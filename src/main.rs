const BASE_DIRECTORY: &str = "kuma/";
const MAIN_URL: &str = "webcom.connexxion.nl";
// the ;x should be equal to the ammount of fallback URLs
const FALLBACK_URL: [&str; 2] = [
    "https://dmz-wbc-web01.connexxion.nl/WebComm/default.aspx",
    "https://dmz-wbc-web02.connexxion.nl/WebComm/default.aspx",
];

extern crate pretty_env_logger;
#[macro_use]
extern crate log;

use clap::command;
use clap::Parser;
use dotenvy::dotenv_override;
use dotenvy::var;
use email::send_errors;
use email::send_welcome_mail;
use tokio::spawn;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Receiver;
use std::fs;
use std::fs::File;
use std::fs::write;
use std::io::Write;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::RwLock;
use thirtyfour::prelude::*;
use time::macros::format_description;

use crate::errors::sign_in_failed_check;
use crate::errors::update_signin_failure;
use crate::errors::FailureType;
use crate::errors::OptionResult;
use crate::errors::ResultLog;
use crate::errors::SignInFailure;
use crate::execution::execution_manager;
use crate::health::send_heartbeat;
use crate::health::update_calendar_exit_code;
use crate::health::ApplicationLogbook;
use crate::ical::*;
use crate::parsing::*;
use crate::shift::*;

pub mod email;
pub mod gebroken_shifts;
mod health;
mod ical;
pub mod kuma;
mod parsing;
pub mod shift;
mod execution;
pub mod errors;

type GenResult<T> = Result<T, Box<dyn std::error::Error>>;

static NAME: LazyLock<RwLock<Option<String>>> = LazyLock::new(|| RwLock::new(None));

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    instant_run: bool,
    #[arg(short, long)]
    single_run: bool
}

fn create_shift_link(shift: &Shift, include_domain: bool) -> GenResult<String> {
    let date_format = format_description!("[day]-[month]-[year]");
    let formatted_date = shift.date.format(date_format)?;
    let domain = match include_domain {
        true => var("PDF_SHIFT_DOMAIN").unwrap_or("https://emphisia.nl/shift/".to_string()),
        false => "".to_owned(),
    };
    if domain.is_empty() && include_domain == true {
        return Ok(format!(
            "https://dmz-wbc-web01.connexxion.nl/WebComm/shiprint.aspx?{}",
            &formatted_date
        ));
    }
    let shift_number_bare = match shift.number.split("-").next() {
        Some(shift_number) => shift_number,
        None => return Err("Could not get shift number".into()),
    };
    Ok(format!(
        "{domain}{shift_number_bare}?date={}",
        &formatted_date
    ))
}

fn create_ical_filename() -> GenResult<String> {
    let username = var("USERNAME")?;
    match var("RANDOM_FILENAME").ok() {
        Some(value) if value == "false".to_owned() => Ok(format!("{}.ics", username)),
        None => Ok(format!("{}.ics", username)),
        _ => Ok(format!("{}.ics", var("RANDOM_FILENAME")?)),
    }
}

pub async fn wait_until_loaded(driver: &WebDriver) -> GenResult<()> {
    let mut started_loading = false;
    let timeout_duration = std::time::Duration::from_secs(30);
    let _ = tokio::time::timeout(timeout_duration, async {
        loop {
            let ready_state: ScriptRet = driver
                .execute("return document.readyState", vec![])
                .await?;
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

pub fn create_path(filename: &str) -> PathBuf {
    let mut path = PathBuf::from(BASE_DIRECTORY);
    path.push(filename);
    path
}

fn set_get_name(new_name_option: Option<String>) -> String {
    let path = create_path("name");
    // Just return constant name if already set
    if let Ok(const_name_option) = NAME.read() {
        if let Some(const_name) = const_name_option.clone() {
            if new_name_option.is_none() {
                return const_name;
            }
        }
    }
    let mut name = std::fs::read_to_string(&path)
        .ok()
        .unwrap_or("FOUT BIJ LADEN VAN NAAM".to_owned());

    // Write new name if previous name is different (deadname protection lmao)
    if let Some(new_name) = new_name_option {
        if new_name != name {
            if let Err(error) = write(&path, &new_name) {
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

// Main program logic that has to run, if it fails it will all be reran.
async fn main_program(
    driver: &WebDriver,
    username: &str,
    password: &str,
    retry_count: usize,
    logbook: &mut ApplicationLogbook,
) -> GenResult<FailureType> {
    driver.delete_all_cookies().await?;
    info!("Loading site: {}..", MAIN_URL);
    match driver.goto(MAIN_URL).await {
        Ok(_) => wait_untill_redirect(&driver).await?,
        Err(_) => {
            error!(
                "Failed waiting for redirect. Going to fallback {}",
                FALLBACK_URL[retry_count % FALLBACK_URL.len()]
            );
            driver
                .goto(FALLBACK_URL[retry_count % FALLBACK_URL.len()])
                .await
                .map_err(|_| Box::new(FailureType::ConnectError))?
        }
    };
    // If getting previous shift information failed, just create an empty one. Because it will cause a new calendar to be created
    let previous_shifts_information = match get_previous_shifts()? {
        Some(previous_shifts) => previous_shifts,
        None => PreviousShiftInformation::new(),
    };
    load_calendar(&driver, &username, &password).await?;
    wait_until_loaded(&driver).await?;
    let mut new_shifts = load_current_month_shifts(&driver, logbook).await?;
    let ical_path = get_ical_path()?;
    if !ical_path.exists() {
        info!(
            "An existing calendar file has not been found, adding two extra months of shifts, also removing partial calendars"
        );
        || -> GenResult<()> {
            fs::remove_file(PathBuf::from(NON_RELEVANT_EVENTS_PATH))?;
            Ok(fs::remove_file(PathBuf::from(RELEVANT_EVENTS_PATH))?)
        }().warn("Removing partial shifts");

        new_shifts.append(&mut load_previous_month_shifts(&driver, 2).await?);
    } else {
        debug!("Existing calendar file found");
        new_shifts.append(&mut load_previous_month_shifts(&driver, 0).await?);
    }
    new_shifts.append(&mut load_next_month_shifts(&driver, logbook).await?);
    info!("Found {} shifts", new_shifts.len());
    let non_relevant_shifts = previous_shifts_information.previous_non_relevant_shifts;
    let mut previous_shifts = previous_shifts_information.previous_relevant_shifts;
    // The main send email function will return the broken shifts that are new or have changed.
    // This is because the send email functions uses the previous shifts and scanns for new shifts
    // write("./shifts.json",serde_json::to_string_pretty(&new_shifts).unwrap());
    let shifts = match email::send_emails(&mut new_shifts, &mut previous_shifts) {
        Ok(shifts) => shifts,
        Err(err) => return Err(err),
    };
    let shifts = gebroken_shifts::load_broken_shift_information(&driver, &shifts).await?; // Replace the shifts with the newly created list of broken shifts
    ical::save_relevant_shifts(&shifts)?;
    let broken_split_shifts = gebroken_shifts::split_broken_shifts(shifts.clone())?;
    let midnight_stopped_shifts = gebroken_shifts::stop_shift_at_midnight(&broken_split_shifts);
    let mut night_split_shifts = gebroken_shifts::split_night_shift(&midnight_stopped_shifts);
    night_split_shifts.sort_by_key(|shift| shift.magic_number);
    night_split_shifts.dedup();
    let mut all_shifts = night_split_shifts.clone();
    all_shifts.append(&mut non_relevant_shifts.clone());
    let calendar = create_ical(
        &night_split_shifts,
        shifts,
        &previous_shifts_information.previous_exit_code,
    );
    send_welcome_mail(&ical_path)?;
    let mut output = File::create(&ical_path)?;
    info!("Writing to: {:?}", &ical_path);
    write!(output, "{}", calendar)?;
    logbook.generate_shift_statistics(&all_shifts);
    Ok(previous_shifts_information.previous_exit_code)
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
async fn main_loop(
    driver: &WebDriver,
    receiver: &mut Receiver<bool>,
    kuma_url: Option<&str>,
) -> GenResult<()> {
    loop {
        info!("Waiting for notification");
        let continue_execution = receiver.recv().await.result()?;
        let name = set_get_name(None);
        let mut logbook = ApplicationLogbook::load();

        let username = var("USERNAME").expect("Error in username variable loop");
        let password = var("PASSWORD").expect("Error in password variable loop");

        let mut current_exit_code = FailureType::default();
        let mut previous_exit_code = FailureType::default();

        let mut running_errors: Vec<Box<dyn std::error::Error>> = vec![];

        let mut retry_count: usize = 0;
        let max_retry_count: usize = var("RETRY_COUNT")
            .unwrap_or("3".to_string())
            .parse()
            .unwrap_or(3);

        // Check if the program is allowed to run, or not due to failed sign-in
        let sign_in_check: Option<SignInFailure> = sign_in_failed_check().unwrap_or(None);
        if let Some(failure) = sign_in_check {
            retry_count = max_retry_count;
            current_exit_code = FailureType::SignInFailed(failure);
        }

        while retry_count < max_retry_count {
            match main_program(&driver, &username, &password, retry_count, &mut logbook).await {
                Ok(last_exit_code) => {
                    previous_exit_code = last_exit_code;
                    update_signin_failure(false, None).warn("Updating signin failure");
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
                                _ = update_signin_failure(true, Some(y.clone()));
                                current_exit_code = FailureType::SignInFailed(y.to_owned());
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
            current_exit_code = FailureType::TriesExceeded;
            send_errors(&running_errors, &name).warn("Sending errors in loop");
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
                    current_exit_code = FailureType::ConnectError;
                }
            }
        }
        if current_exit_code != FailureType::TriesExceeded {
            send_heartbeat(&current_exit_code, kuma_url, &username).await.warn("Sending Heartbeat in loop");
        }
        logbook.save(&current_exit_code).warn("Saving logbook in loop");
        // Update the exit code in the calendar if it is not equal to the previous value
        if previous_exit_code != current_exit_code {
            warn!("Previous exit code was different than current, need to update");
            update_calendar_exit_code(&previous_exit_code, &current_exit_code).warn("Updating calendar exit code");
        }
        if !continue_execution {
            break;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> GenResult<()> {
    dotenv_override().ok();
    pretty_env_logger::init();
    warn!("Starting Webcom Ical");

    let args = Args::parse();

    let username = var("USERNAME").expect("Error in username variable");
    let kuma_url = var("KUMA_URL").ok();

    let mut logbook = ApplicationLogbook::load();
    let driver: WebDriver = match initiate_webdriver().await {
        Ok(driver) => driver,
        Err(error) => {
            error!("Kon driver niet opstarten: {:?}", &error);
            send_errors(&vec![error], &set_get_name(None)).info("Send errors");
            logbook.save(&FailureType::GeckoEngine).warn("Saving Logbook");
            send_heartbeat(&FailureType::GeckoEngine, kuma_url.as_deref(), &username).await.warn("Sending heartbeat");
            return Err("driver fout".into());
        }
    };

    if let Some(url_unwrap) = kuma_url.clone() {
        if !url_unwrap.is_empty() {
            debug!("Checking if kuma needs to be created");
            kuma::first_run(&url_unwrap, &username).await.warn("Kuma Run");
        }
    }

    let (tx, mut rx) = channel(1);
    let instant_run = args.instant_run;
    // If the single run argument is set, just send a single message so the main loop instantly runs. 
    // Otherwise start the execution manager
    match args.single_run {
        false => {spawn(async move {execution_manager(tx, instant_run).await});},
        true => {tx.send(false).await?;}
    };
    
    main_loop(&driver, &mut rx, kuma_url.as_deref()).await?;
    driver.quit().await?;
    Ok(())
}
