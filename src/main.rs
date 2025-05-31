extern crate pretty_env_logger;
#[macro_use] extern crate log;

use dotenvy::dotenv_override;
use dotenvy::var;
use email::send_errors;
use email::send_welcome_mail;
use email::PreviousShiftsError;
use reqwest;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::fs::write;
use std::hash::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::str::Split;
use std::sync::LazyLock;
use std::sync::RwLock;
use thirtyfour::prelude::*;
use thiserror::Error;
use time::Duration;
use time::macros::format_description;
use url::Url;

use crate::ical::*;
use crate::parsing::*;

use time::Date;
use time::Time;

pub mod email;
pub mod gebroken_shifts;
mod ical;
pub mod kuma;
mod parsing;

type GenResult<T> = Result<T, Box<dyn std::error::Error>>;

const BASE_DIRECTORY: &str = "kuma/";
static NAME: LazyLock<RwLock<Option<String>>> = LazyLock::new(|| RwLock::new(None));

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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
enum SignInFailure {
    TooManyTries,
    IncorrectCredentials,
    WebcomDown,
    Other(String),
}

#[derive(Debug, Error, PartialEq)]
enum FailureType {
    TriesExceeded,
    GeckoEngine,
    SignInFailed(SignInFailure),
    Other(String),
    OK,
}

impl std::fmt::Display for FailureType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shift {
    date: Date,
    start: Time,
    end_date: Date,
    end: Time,
    duration: Duration,
    number: String,
    kind: String,
    location: String,
    description: String,
    is_broken: bool,
    magic_number: i64,
}

impl Shift {
    /*
    Creates a new Shift struct from a simple string straight from webcom
    Also hashes the string to see if it has been updated
    Looks intimidating, bus is mostly boilerplate + a bit of logic for correctly parsing the duration
    */
    fn new(text: String, date: Date) -> Self {
        let text_clone = text.clone();
        let parts = text_clone.split("\u{a0}• \u{a0}• ");
        let mut location_modifier = 1;
        let parts_clean: Vec<String> = parts
            .map(|x| {
                let y = x.replace("\u{a0}• ", "");
                y
            })
            .collect();
        let mut parts_list: Vec<Split<'_, &str>> =
            parts_clean.iter().map(|x| x.split(": ")).collect();
        let number: String = parts_list[0].nth(1).unwrap().to_string();
        let _date: String = parts_list[1].nth(1).unwrap().to_string();
        let time: String = parts_list[2].nth(1).unwrap_or("").to_string();
        let shift_duration: String = parts_list[3].nth(1).unwrap_or("").to_string();
        let _working_hours: String = parts_list[4].nth(1).unwrap_or("").to_string();
        let _day_of_week: String = parts_list[5].nth(1).unwrap_or("").to_string();
        let kind: String = parts_list[6].nth(1).unwrap_or("").to_string();
        let mut location = "Onbekend".to_string();
        if parts_list[7].next().unwrap_or("") == "Startplaats" {
            location_modifier = 0;
            location = parts_list[7].next().unwrap_or("").to_string();
        }
        let description: String = parts_list[8 - location_modifier]
            .nth(1)
            .unwrap_or("")
            .to_string();
        let start_time_str = time.split_whitespace().nth(0).unwrap();
        let end_time_str = time.split_whitespace().nth(2).unwrap();
        let start = get_time(start_time_str);
        let end = get_time(end_time_str);
        let mut is_broken = false;
        let shift_type = number.chars().nth(0).unwrap();
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let magic_number = (hasher.finish() as i128 - i64::MAX as i128) as i64;
        if shift_type == 'g' || shift_type == 'G' {
            is_broken = true;
        }

        let duration_split = shift_duration.split_whitespace().nth(0).unwrap().split(":");
        let duration_minutes = Duration::minutes(
            duration_split
                .clone()
                .nth(1)
                .unwrap()
                .parse::<i64>()
                .unwrap(),
        );
        let duration_hours = Duration::hours(
            duration_split
                .clone()
                .nth(0)
                .unwrap()
                .parse::<i64>()
                .unwrap(),
        );
        let duration = duration_hours + duration_minutes;
        let mut end_date = date;
        if end < start {
            end_date = date + Duration::days(1);
        }
        Self {
            date,
            number,
            start,
            end_date,
            end,
            duration,
            kind,
            location,
            description,
            is_broken,
            magic_number,
        }
    }

    // Create two new shifts from one broken shift.
    // Assumes second shift cannot start after midnight
    fn new_from_existing(
        new_between_times: (Time, Time),
        existing_shift: &Self,
        start_next_day: bool,
    ) -> Vec<Self> {
        let mut part_one = existing_shift.clone();
        part_one.end = new_between_times.0;
        part_one.end_date = match start_next_day {
            true => existing_shift.end_date,
            false => existing_shift.date,
        };
        let mut part_two = existing_shift.clone();
        part_two.start = new_between_times.1;
        part_two.date = match start_next_day {
            true => existing_shift.end_date,
            false => existing_shift.date,
        };
        let shifts: Vec<Self> = vec![part_one, part_two];
        shifts
    }
}

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
    let username = var("USERNAME").unwrap();
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
        "status={}&msg={:?}&ping=",
        match reason {
            FailureType::GeckoEngine => "down",
            FailureType::SignInFailed(_) => "down",
            _ => "up",
        },
        reason
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
    // let main_url = format!(
    //     "https://dmz-wbc-web0{}.connexxion.nl/WebComm/default.aspx",
    //     (retry_count % 2) + 1
    // );
    let main_url = "webcom.connexxion.nl";
    info!("Loading site: {}..", main_url);
    driver.goto(main_url).await?;
    wait_untill_redirect(&driver).await?;
    load_calendar(&driver, &username, &password).await?;
    wait_until_loaded(&driver).await?;
    let mut shifts = load_current_month_shifts(&driver).await?;
    shifts.append(&mut load_previous_month_shifts(&driver,).await?);
    shifts.append(&mut load_next_month_shifts(&driver).await?);
    info!("Found {} shifts", shifts.len());

    // The main send email function will return the broken shifts that are new or have changed.
    // This is because the send email functions uses the previous shifts and scanns for new shifts
    let shifts_to_check_broken = match email::send_emails(&shifts) {
        Ok(changed_shifts) => changed_shifts,
        Err(err) if err.downcast_ref::<PreviousShiftsError>().is_some() => shifts.clone(),
        Err(err) => return Err(err),
    };
    save_shifts_on_disk(&shifts, Path::new(&format!("./{BASE_DIRECTORY}previous_shifts.toml")))?; // We save the shifts before modifying them further to declutter the list. We only need the start and end times of the total shift.
    let shifts = gebroken_shifts::gebroken_diensten_laden(&driver, shifts,&shifts_to_check_broken).await?; // Replace the shifts with the newly created list of broken shifts
    let shifts = gebroken_shifts::split_night_shift(&shifts);
    let calendar = create_ical(&shifts);
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
            send_errors(vec![error], &name).unwrap();
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
        match send_errors(running_errors, &name) {
            Ok(_) => (),
            Err(x) => error!("failed to send error email, ironic: {:?}", x),
        }
    }
    if error_reason != FailureType::TriesExceeded {
        heartbeat(error_reason, kuma_url, &username).await.unwrap();
    }
    driver.quit().await?;
    Ok(())
}
