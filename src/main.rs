extern crate pretty_env_logger;
#[macro_use] extern crate log;

use dotenvy::dotenv_override;
use dotenvy::var;
use email::send_errors;
use email::send_welcome_mail;
use reqwest;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::fs::write;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::RwLock;
use thirtyfour::prelude::*;
use thiserror::Error;
use time::macros::format_description;
use url::Url;

use crate::ical::*;
use crate::parsing::*;
use crate::shift::*;

use time::Time;

pub mod email;
pub mod gebroken_shifts;
pub mod shift;
mod ical;
pub mod kuma;
mod parsing;

type GenResult<T> = Result<T, Box<dyn std::error::Error>>;

const BASE_DIRECTORY: &str = "kuma/";
const FALLBACK_URL: &str = "https://dmz-wbc-web02.connexxion.nl/WebComm/default.aspx";
static NAME: LazyLock<RwLock<Option<String>>> = LazyLock::new(|| RwLock::new(None));

// This should also store the hash of the password. So it can know if the password has changed in the meantime
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IncorrectCredentialsCount {
    retry_count: usize,
    error: Option<SignInFailure>,
}

impl IncorrectCredentialsCount {
    fn new() -> Self {
        Self {
            retry_count: 0,
            error: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Error)]
enum SignInFailure {
    #[error("Er zijn te veel incorrecte inlogpogingen in een korte periode gedaan")]
    TooManyTries,
    #[error("Inloggegevens kloppen niet")]
    IncorrectCredentials,
    #[error("Webcomm heeft een storing")]
    WebcomDown,
    #[error("Onbekende fout: {0}")]
    Other(String),
}

#[derive(Debug, Error, PartialEq, Clone)]
enum FailureType {
    #[error("Webcom ical was niet in staat na meerdere pogingen diensten correct in te laden")]
    TriesExceeded,
    #[error("Webcom ical kan geen verbinding maken met de interne browser")]
    GeckoEngine,
    #[error("Webcom ical kon niet inloggen. Fout: {}",0.to_string())]
    SignInFailed(SignInFailure),
    #[error("Webcom ical kon geen verbinding maken met de Webcomm site")]
    ConnectError,
    #[error("Een niet-specifieke fout is opgetreden: {0}")]
    Other(String),
    #[error("Ok")]
    OK,
}

// impl std::fmt::Display for FailureType {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{:?}", self)
//     }
// }

/*
An absolutely useless struct that is only needed  becasue a Vec<> cannot be serialised
*/
#[derive(Serialize, Deserialize)]
pub struct Shifts {
    shifts: Vec<Shift>,
}

impl Shifts {
    fn new(shifts: Vec<Shift>) -> Self {
        Self { shifts }
    }
}

fn create_shift_link(shift: &Shift) -> GenResult<String> {
    let date_format = format_description!("[day]-[month]-[year]");
    let formatted_date = shift.date.format(date_format)?;
    let domain = var("PDF_SHIFT_DOMAIN").unwrap_or("https://emphisia.nl/shift/".to_string());
    if domain.is_empty() {
        return Ok(format!(
            "https://dmz-wbc-web01.connexxion.nl/WebComm/shiprint.aspx?{}",
            &formatted_date
        ));
    }
    let shift_number_bare = match shift.number.split("-").next(){
        Some(shift_number) => shift_number,
        None => return Err("Could not get shift number".into())
    };
    Ok(format!(
        "{domain}{shift_number_bare}?date={}",
        &formatted_date
    ))
}

// Creates and returns a Time::time from a given string of time eg: 12:34
// Uses A LOT of unwraps, so can easilly fail. :)
fn get_time(str_time: &str) -> Time {
    let mut time_split = str_time.split(":");
    let mut hour: u8 = time_split.clone().next().unwrap().parse().unwrap();
    let min: u8 = time_split.nth(1).unwrap().parse().unwrap();
    if hour >= 24 {
        hour = hour - 24;
    }
    Time::from_hms(hour, min, 0).unwrap()
}

async fn check_sign_in_error(driver: &WebDriver) -> GenResult<FailureType> {
    error!("Sign in failed");
    match driver.find(By::Id("ctl00_lblMessage")).await {
        Ok(element) => {
            let element_text = element.text().await?;
            let sign_in_error_type = get_sign_in_error_type(&element_text);
            info!("Found error banner: {:?}", &sign_in_error_type);
            return Ok(FailureType::SignInFailed(sign_in_error_type));
        }
        Err(_) => {
            info!("Geen fount banner gevonden");
        }
    };
    Ok(FailureType::SignInFailed(SignInFailure::Other(
        "Geen idee waarom er niet ingelogd kon worden".to_string(),
    )))
}

// See if there is a text which indicated webcom is offline
fn check_if_webcom_unavailable(h3_text: Option<String>) -> bool {
    match h3_text {
        Some(text) => {
            if text == "De servertoepassing is niet beschikbaar.".to_owned() {
                return true;
            }
        }
        None => (),
    };
    false
}

fn get_sign_in_error_type(text: &str) -> SignInFailure {
    match text {
        "Uw aanmelding was niet succesvol. Voer a.u.b. het personeelsnummer of 'naam, voornaam' in" => {
            SignInFailure::IncorrectCredentials
        }
        "Te veel verkeerde aanmeldpogingen" => SignInFailure::TooManyTries,
        _ => SignInFailure::Other(text.to_string()),
    }
}

fn create_ical_filename() -> GenResult<String> {
    let username = var("USERNAME")?;
    match var("RANDOM_FILENAME").ok() {
        Some(value) if value == "false".to_owned() => Ok(format!("{}.ics", username)),
        None => Ok(format!("{}.ics", username)),
        _ => Ok(format!("{}.ics", var("RANDOM_FILENAME")?)),
    }
}

/*
Serialise the shifts to be saved to disk.
This is needed to send a mail when a new shift is found
Needs to create a struct with a Vec<Shift> because otherwise it wouldn't serialise correctly
*/
fn save_shifts_on_disk(shifts: &Vec<Shift>, path: &Path) -> GenResult<()> {
    let shifts_struct = Shifts::new(shifts.clone());
    let shifts_serialised = toml::to_string(&shifts_struct)?;
    let mut output = File::create(path)?;
    write!(output, "{}", shifts_serialised)?;
    Ok(())
}

pub async fn wait_until_loaded(driver: &WebDriver) -> GenResult<()> {
    let mut started_loading = false;
    let timeout_duration = std::time::Duration::from_secs(30);
    let _ = tokio::time::timeout(timeout_duration, async {
        loop {
            let ready_state: ScriptRet = driver
                .execute("return document.readyState", vec![])
                .await
                .unwrap();
            let current_state = format!("{:?}", ready_state.json());
            if current_state == "String(\"complete\")" && started_loading {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                return Ok::<(), WebDriverError>(());
            }
            if current_state == "String(\"loading\")" {
                started_loading = true;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    Ok(())
}

async fn wait_untill_redirect(driver: &WebDriver) -> GenResult<()> {
    let initial_url = driver.current_url().await?;
    let mut current_url = driver.current_url().await?;
    let timeout = std::time::Duration::from_secs(30); // Maximum wait time.

    tokio::time::timeout(timeout, async {
        loop {
            let new_url = driver.current_url().await.unwrap();
            if new_url != current_url {
                current_url = new_url;
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;

    if current_url == initial_url {
        warn!("Timeout waiting for redirect.");
        return Err(Box::new(WebDriverError::Timeout(
            "Redirect did not occur".into(),
        )));
    }

    debug!("Redirected to: {}", current_url);
    wait_until_loaded(driver).await?;
    Ok(())
}

async fn heartbeat(
    reason: FailureType,
    url: Option<String>,
    personeelsnummer: &str,
) -> GenResult<()> {
    if url.is_none() || reason == FailureType::TriesExceeded {
        info!("no heartbeat URL");
        return Ok(());
    }
    let mut request_url: Url = url.clone().unwrap().parse().unwrap();
    request_url.set_path(&format!("/api/push/{personeelsnummer}"));
    request_url.set_query(Some(&format!(
        "status={}&msg={}&ping=",
        match reason.clone() {
            FailureType::GeckoEngine => "down",
            FailureType::SignInFailed(failure) if matches!(failure, SignInFailure::WebcomDown | SignInFailure::TooManyTries | SignInFailure::Other(_)) => "down",
            _ => "up",
        },
        reason.to_string()
    )));
    reqwest::get(request_url).await?;
    Ok(())
}

// Loads the sign in failure counter. Creates it if it does not exist
fn load_sign_in_failure_count(path: &Path) -> GenResult<IncorrectCredentialsCount> {
    let failure_count_toml = std::fs::read_to_string(path)?;
    let failure_counter: IncorrectCredentialsCount = toml::from_str(&failure_count_toml)?;
    Ok(failure_counter)
}

// Save the sign in faulure count file
fn save_sign_in_failure_count(path: &Path, counter: &IncorrectCredentialsCount) -> GenResult<()> {
    let failure_counter_serialised = toml::to_string(counter)?;
    let mut output = File::create(path).unwrap();
    write!(output, "{}", failure_counter_serialised)?;
    Ok(())
}

// This is a pretty useless function. It checks if the DOMAIN env variable was changed since last time the program was run
// It is just to send a new welcome mail
fn check_domain_update(ical_path: &PathBuf) {
    let previous_domain;
    let path = &format!("./{BASE_DIRECTORY}previous_domain");
    match std::fs::read_to_string(path) {
        Ok(x) => {
            // println!("{}", &x);
            previous_domain = Some(x)
        }
        Err(_) => previous_domain = None,
    }
    let current_domain = var("DOMAIN").unwrap_or("".to_string());
    if let Some(previous_domain_unwrap) = previous_domain {
        if previous_domain_unwrap != current_domain {
            let _ = send_welcome_mail(ical_path, true);
        }
    }
    match File::create(path) {
        Ok(mut file) => {
            let _ = write!(file, "{}", current_domain);
        }
        Err(_) => (),
    }
}

fn set_get_name(new_name_option: Option<String>) -> String {
    let path = "./kuma/name";
    // Just return constant name if already set
    if let Ok(const_name) = NAME.read() {
        if new_name_option.is_none() && const_name.is_some() {
            return const_name.clone().unwrap();
        }
    }
    let mut name = std::fs::read_to_string(path)
        .ok()
        .unwrap_or("FOUT BIJ LADEN VAN NAAM".to_owned());

    // Write new name if previous name is different (deadname protection lmao)
    if let Some(new_name) = new_name_option {
        if new_name != name {
            if let Err(error) = write(path, &new_name) {
                error!("Fout tijdens opslaan van naam: {}", error.to_string());
            }
            name = new_name;
        }
    }
    if let Ok(mut const_name) = NAME.write() {
        *const_name = Some(name.clone());
    }
    name
}

// If returning true, continue execution
fn sign_in_failed_check(username: &str) -> GenResult<Option<SignInFailure>> {
    let resend_error_mail_count: usize = var("SIGNIN_FAIL_MAIL_REPEAT")
        .unwrap_or("24".to_string())
        .parse()
        .unwrap_or(2);
    let sign_in_attempt_reduce: usize = var("SIGNIN_FAILED_REDUCE")
        .unwrap_or("2".to_string())
        .parse()
        .unwrap_or(1);
    let path = Path::new("./sign_in_failure_count.toml");
    // Load the existing failure counter, create a new one if one doesn't exist yet
    let mut failure_counter = match load_sign_in_failure_count(path) {
        Ok(value) => value,
        Err(_) => {
            let new = IncorrectCredentialsCount::new();
            save_sign_in_failure_count(path, &new)?;
            new
        }
    };
    let return_value: Option<SignInFailure>;
    // else check if retry counter == reduce_ammount, if not, stop running
    if failure_counter.retry_count == 0 {
        return_value = None;
    } else if failure_counter.retry_count % sign_in_attempt_reduce == 0 {
        warn!(
            "Continuing execution with sign in error, reduce val: {sign_in_attempt_reduce}, current count {}",
            failure_counter.retry_count
        );
        failure_counter.retry_count += 1;
        return_value = None;
    } else {
        warn!("Skipped execution due to previous sign in error");
        failure_counter.retry_count += 1;
        return_value = Some(failure_counter.error.clone().unwrap());
    }
    if failure_counter.retry_count % resend_error_mail_count == 0 && failure_counter.error.is_some()
    {
        email::send_failed_signin_mail(username, &failure_counter, false)?;
    }
    save_sign_in_failure_count(path, &failure_counter)?;
    Ok(return_value)
}



fn sign_in_failed_update(
    username: &str,
    failed: bool,
    failure_type: Option<SignInFailure>,
) -> GenResult<()> {
    let path = Path::new("./sign_in_failure_count.toml");
    let mut failure_counter = load_sign_in_failure_count(path)?;
    // if failed == true, set increment counter and set error
    if failed == true {
        failure_counter.error = failure_type;
        if failure_counter.retry_count == 0 {
            failure_counter.retry_count += 1;
            email::send_failed_signin_mail(username, &failure_counter, true)?;
        }
    }
    // if failed == false, reset counter
    else if failed == false {
        if failure_counter.error.is_some() {
            info!("Sign in succesful again!");
            email::send_sign_in_succesful(username)?;
        }
        failure_counter.retry_count = 0;
        failure_counter.error = None;
    }
    save_sign_in_failure_count(path, &failure_counter)?;
    Ok(())
}

// Main program logic that has to run, if it fails it will all be reran.
async fn main_program(driver: &WebDriver, username: &str, password: &str) -> GenResult<()> {
    driver.delete_all_cookies().await?;
    let main_url = "webcom.connexxion.nl";
    info!("Loading site: {}..", main_url);
    match driver.goto(main_url).await {
        Ok(_) => wait_untill_redirect(&driver).await?,
        Err(_) => {error!("Failed waiting for redirect. Going to fallback {FALLBACK_URL}");
        driver.goto(FALLBACK_URL).await.map_err(|_| {Box::new(FailureType::ConnectError)})? }
    };
    load_calendar(&driver, &username, &password).await?;
    wait_until_loaded(&driver).await?;
    let mut new_shifts = load_current_month_shifts(&driver).await?;
    new_shifts.append(&mut load_previous_month_shifts(&driver,).await?);
    new_shifts.append(&mut load_next_month_shifts(&driver).await?);
    info!("Found {} shifts", new_shifts.len());
    
    // If getting previous shift information failed, just create an empty one. Because it will cause a new calendar to be created
    let previous_shifts_information = match get_previous_shifts() {
        Some(previous_shifts) => previous_shifts,
        None => PreviousShiftInformation::new()
    };
    let non_relevant_shifts = previous_shifts_information.previous_non_relevant_shifts;
    let mut previous_shifts = previous_shifts_information.previous_relevant_shifts;
    // The main send email function will return the broken shifts that are new or have changed.
    // This is because the send email functions uses the previous shifts and scanns for new shifts
    let current_shifts_map = match email::send_emails(&mut new_shifts, &mut previous_shifts) {
        Ok(shifts) => shifts,
        Err(err) => return Err(err),
    };
    let mut current_shifts: Vec<Shift> = current_shifts_map.values().cloned().collect();
    gebroken_shifts::gebroken_diensten_laden(&driver, &mut current_shifts).await?; // Replace the shifts with the newly created list of broken shifts
    debug!("Shift information:\n{current_shifts:#?}");
    ical::save_relevant_shifts(&current_shifts)?;
    let current_shifts = gebroken_shifts::split_broken_shifts(current_shifts)?;
    let current_shifts = gebroken_shifts::split_night_shift(&current_shifts);
    let calendar = create_ical(&current_shifts, non_relevant_shifts);
    let ical_path = PathBuf::from(&format!(
        "{}{}",
        var("SAVE_TARGET")?,
        create_ical_filename()?
    ));
    send_welcome_mail(&ical_path, false)?;
    check_domain_update(&ical_path);
    let mut output = File::create(&ical_path)?;
    info!("Writing to: {:?}", &ical_path);
    write!(output, "{}", calendar)?;
    Ok(())
}

async fn initiate_webdriver() -> GenResult<WebDriver> {
    let gecko_ip = var("GECKO_IP")?;
    let caps = DesiredCapabilities::firefox();
    let driver = WebDriver::new(format!("http://{}", gecko_ip), caps).await?;
    Ok(driver)
}

/*
This starts the WebDriver session
Loads the main logic, and retries if it fails
*/
#[tokio::main]
async fn main() -> WebDriverResult<()> {
    dotenv_override().ok();
    pretty_env_logger::init();
    let version = var("CARGO_PKG_VERSION").unwrap_or("onbekend".to_string());
    warn!("Starting Webcom Ical version {version}");
    let mut error_reason = FailureType::OK;
    let name = set_get_name(None);
    let kuma_url = var("KUMA_URL").ok();
    let username = var("USERNAME").unwrap();
    let password = var("PASSWORD").unwrap();
    let driver = match initiate_webdriver().await {
        Ok(driver) => driver,
        Err(error) => {
            error!("Kon driver niet opstarten: {:?}", &error);
            send_errors(&vec![error], &name).unwrap();
            error_reason = FailureType::GeckoEngine;
            heartbeat(error_reason, kuma_url, &username).await.unwrap();
            return Err(WebDriverError::FatalError("driver fout".to_string()));
        }
    };
    let mut retry_count: usize = 0;
    let mut running_errors: Vec<Box<dyn std::error::Error>> = vec![];
    let max_retry_count: usize = var("RETRY_COUNT")
        .unwrap_or("3".to_string())
        .parse()
        .unwrap_or(3);

    if let Some(url_unwrap) = kuma_url.clone() {
        if !url_unwrap.is_empty() {
            debug!("Checking if kuma needs to be created");
            kuma::first_run(&url_unwrap, &username).await.unwrap();
        }
    }
    // Check if the program is allowed to run, or not due to failed sign-in
    let sign_in_check: Option<SignInFailure> = sign_in_failed_check(&name).unwrap();
    if let Some(failure) = sign_in_check {
        retry_count = max_retry_count;
        error_reason = FailureType::SignInFailed(failure);
    }
    while retry_count <= max_retry_count - 1 {
        match main_program(&driver, &username, &password).await {
            Ok(_) => {
                sign_in_failed_update(&name, false, None).unwrap();
                retry_count = max_retry_count;
            }
            Err(x) => {
                match x.downcast_ref::<FailureType>() {
                    Some(FailureType::SignInFailed(y)) => {
                        // Do not stop webcom if the sign in failure reason is unknown
                        if let SignInFailure::Other(x) = y {
                            warn!(
                                "Kon niet inloggen, maar een onbekende fout: {}. Probeert opnieuw",
                                x
                            )
                        } else {
                            retry_count = max_retry_count;
                            sign_in_failed_update(&name, true, Some(y.clone())).unwrap();
                            error_reason = FailureType::SignInFailed(y.to_owned());
                            error!("Inloggen niet succesvol, fout: {:?}", y)
                        }
                    }
                    _ => {
                        warn!(
                            "Fout tijdens shift laden, opnieuw proberen, poging: {}. Fout: {}",
                            retry_count + 1,
                            &x.to_string()
                        );
                        running_errors.push(x);
                    }
                }
            }
        };
        retry_count += 1;
    }
    if running_errors.is_empty() {
        info!("Alles is in een keer goed gegaan, jippie!");
    } else if running_errors.len() < max_retry_count {
        warn!("Errors have occured, but succeded in the end");
    } else {
        error_reason = FailureType::TriesExceeded;
        match send_errors(&running_errors, &name) {
            Ok(_) => (),
            Err(x) => error!("failed to send error email, ironic: {:?}", x),
        }
    }
    // TODO the logic for selecting the error reason is quite confusing. This should be cleaned up someday
    // Could be done by always returning a failuretype. Wrapping it around all error types. Possible based on the type of error
    // Then having one big match on what to do based on the failure type?
    // But that is for later
    // This currently informs the user if webcom is down
    if let Some(last_error) = running_errors.last() {
            if let Some(failure_type) = last_error.downcast_ref::<FailureType>() {
                if failure_type == &FailureType::ConnectError {
                    info!("Failure reason is because webcom is down");
                    error_reason = FailureType::ConnectError;
                } 
            }
        } 
    if error_reason != FailureType::TriesExceeded {
        heartbeat(error_reason, kuma_url, &username).await.unwrap();
    }
    driver.quit().await?;
    Ok(())
}
