use chrono::NaiveDate;
use chrono::NaiveDateTime;
use chrono::NaiveTime;
use dotenvy::dotenv_override;
use dotenvy::var;
use email::send_errors;
use email::send_welcome_mail;
use gebroken_shifts::navigate_to_subdirectory;
use gebroken_shifts::wait_for_response;
use icalendar::Calendar;
use icalendar::CalendarDateTime;
use icalendar::Component;
use icalendar::Event;
use icalendar::EventLike;
use reqwest;
use serde::{Deserialize, Serialize};
use tokio::io;
use std::fs::File;
use std::hash::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::str::Split;
use thirtyfour::prelude::*;
use time::macros::format_description;
use time::Duration;
use time::Month;
use thiserror::Error;

use time::Date;
use time::Time;

pub mod email;
pub mod gebroken_shifts;

type GenResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct IncorrectCredentialsCount {
    retry_count: usize,
    error: Option<SignInFailure>,
}

impl IncorrectCredentialsCount{
    fn new() -> Self{
        Self{retry_count: 0, error: None}
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
enum SignInFailure {
    TooManyTries,
    IncorrectCredentials,
    Other(String)
}

#[derive(Debug, Error)]
enum FailureType {
    TriesExceeded,
    GeckoEngine,
    SignInFailed(SignInFailure),
    OK,
}

impl std::fmt::Display for FailureType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,"{:?}",self)
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
    name: String,
}

impl Shift {
    /*
    Creates a new Shift struct from a simple string straight from webcom
    Also hashes the string to see if it has been updated
    Looks intimidating, bus is mostly boilerplate + a bit of logic for correctly parsing the duration
    */
    fn new(text: String, date: Date, name: &str) -> Self {
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
        if parts_list[7].nth(0).unwrap_or("") == "Startplaats"{
            location_modifier = 0;
            location = parts_list[7].nth(1).unwrap_or("").to_string();
        }
        let description: String = parts_list[8-location_modifier].nth(1).unwrap_or("").to_string();
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
            end_date = date + time::Duration::days(1);
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
            name: name.to_string(),
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
    let domain = var("PDF_SHIFT_DOMAIN").unwrap_or("https://emphisia.nl/shift/".to_string());
    if domain.is_empty() {
        let date_format = format_description!("[year]-[month]-[day]");
        return Ok(format!(
            "https://dmz-wbc-web01.connexxion.nl/WebComm/shiprint.aspx?{}",
            shift.date.format(date_format)?
        ));
    }
    let shift_number_bare= shift.number.split("-").next().unwrap();
    Ok(format!("{domain}{shift_number_bare}"))
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

async fn sign_in_webcom(driver: &WebDriver, user: &str, pass: &str) -> GenResult<String> {
    let username_field = driver
        .find(By::Id("ctl00_cntMainBody_lgnView_lgnLogin_UserName"))
        .await?;
    username_field.send_keys(user).await?;
    let password_field = driver
        .find(By::Id("ctl00_cntMainBody_lgnView_lgnLogin_Password"))
        .await?;
    password_field.send_keys(pass).await?;
    driver
        .find(By::Id("ctl00_cntMainBody_lgnView_lgnLogin_LoginButton"))
        .await?
        .click()
        .await?;
    println!("waiting until loaded");
    //wait_until_loaded(&driver).await?;
    let _ = wait_for_response(&driver, By::Tag("h3"), false).await;
    println!("loaded");
    let name_text = match driver.find(By::Tag("h3")).await{
        Ok(element) => element.text().await?,
        Err(_) => {return Err(Box::new(check_sign_in_error(driver).await?));},
    };
    let name = name_text
        .split(",")
        .last()
        .unwrap().split_whitespace().next().unwrap()
        .to_string();
    Ok(name)
}

async fn check_sign_in_error(driver: &WebDriver) -> GenResult<FailureType>{
    println!("Sign in failed");
    match driver.find(By::Id("ctl00_lblMessage")).await {
        Ok(element) => {let element_text = element.text().await?;
            let sign_in_error_type = get_sign_in_error_type(&element_text);
            println!("Found error banner: {:?}",&sign_in_error_type);
            return Ok(FailureType::SignInFailed(sign_in_error_type));}
        Err(_) => {println!("Geen fount banner gevonden")}
    };
    Ok(FailureType::SignInFailed(SignInFailure::Other("Geen idee waarom er niet ingelogd kon worden".to_string())))
}

fn get_sign_in_error_type(text: &str) -> SignInFailure {
    match text {
        "Uw aanmelding was niet succesvol. Voer a.u.b. het personeelsnummer of 'naam, voornaam' in" => SignInFailure::IncorrectCredentials,
        "Te veel verkeerde aanmeldpogingen" => SignInFailure::TooManyTries,
        _ => SignInFailure::Other(text.to_string())
    }

}

/*
Logs into webcom, has no logic for when the login fails.
It will also find and return the first name of the user, this will fail if the login is unsuccesful
*/
async fn load_calendar(driver: &WebDriver, user: &str, pass: &str) -> GenResult<String> {
    println!("Logging in..");
    let name = sign_in_webcom(driver, user, pass).await?;
    //wait_until_loaded(&driver).await?;
    
    
    //println!("{}", name_text);
    // let rooster_knop = driver.query(By::LinkText("Rooster")).first().await?;
    // rooster_knop.wait_until().displayed().await?;
    // rooster_knop.click().await?;
    println!("Loading rooster..");
    navigate_to_subdirectory(driver, "roster.aspx").await?;
    Ok(name)
}

/*
Checks all supplied WebElements, it checks if the day contains the text "Dienstuur"  and if so, adds it to a Vec of valid shifts in the calendar
Does not search itself for elements
*/
async fn get_elements(
    driver: &WebDriver,
    month: Month,
    year: i32,
    name: String,
) -> WebDriverResult<Vec<Shift>> {
    let mut temp_emlements: Vec<Shift> = vec![];
    let elements = driver
        .query(By::ClassName("calDay"))
        .all_from_selector()
        .await?;
    for element in elements {
        let text = match element.attr("data-original-title").await? {
            Some(x) => x,
            None => {
                return Err(WebDriverError::FatalError(
                    "no elements in rooster".to_string(),
                ));
            }
        };
        if !text.is_empty() && text.contains("Dienstduur") {
            // println!("Loading shift: {:?}", &text);
            let dag_text = element.find(By::Tag("strong")).await?.text().await?;
            let dag_text_split = dag_text.split_whitespace().next().unwrap();

            // println!("dag {}", &dag_text);
            let dag: u8 = dag_text_split.parse().unwrap();
            let date = Date::from_calendar_date(year as i32, month, dag).unwrap();
            let new_shift = Shift::new(text, date, &name);
            temp_emlements.push(new_shift.clone());
            println!("Found Shift {}", &new_shift.number);
        }
    }
    Ok(temp_emlements)
}

/*
Creates the ICAL file to add to the calendar
*/
fn create_ical(shifts: &Vec<Shift>) -> String {
    println!("Creating calendar file...");
    let mut calendar = Calendar::new()
        .name("Hermes rooster")
        .append_property(("METHOD","PUBLISH")).timezone("Europe/Amsterdam")
        .done();
    for shift in shifts {
        let shift_link = create_shift_link(shift).unwrap();
        calendar.push(
            Event::new()
                .summary(&format!("Shift - {}", shift.number))
                .description(&format!(
                    "Dienstsoort • {}
Duur • {} uur {} minuten
Omschrijving • {}
Shift sheet • {}",
                    shift.kind,
                    shift.duration.whole_hours(),
                    shift.duration.whole_minutes() % 60,
                    shift.description,
                    shift_link
                ))
                .location(&shift.location)
                .starts(create_dateperhapstime(shift.date, shift.start))
                .ends(create_dateperhapstime(shift.end_date, shift.end))
                .done(),
        );
    }
    //println!("{}", calendar);
    String::from(calendar.to_string())
}

/*
I use the create Time to keep track of dates and time. But the crate used for creating the ICAL file uses chrono to keep time.
*/
fn create_dateperhapstime(date: Date, time: Time) -> CalendarDateTime {
    let date_day = date.day();
    let date_month = date.month() as u8;
    let date_year = date.year();
    let time_hrs = time.hour();
    let time_min = time.minute();
    let naive_time = NaiveTime::from_hms_opt(time_hrs as u32, time_min as u32, 0).unwrap();
    let naive_date =
        NaiveDate::from_ymd_opt(date_year as i32, date_month as u32, date_day as u32).unwrap();
    let naive_date_time = NaiveDateTime::new(naive_date, naive_time);
    CalendarDateTime::WithTimezone {
        date_time: naive_date_time,
        tzid: "Europe/Amsterdam".to_string(),
    }
}

/*
Just presses the previous button in webcom to load the previous month
*/
async fn load_previous_month_shifts(
    driver: &WebDriver,
    name: String,
) -> WebDriverResult<Vec<Shift>> {
    println!("Loading Previous Month..");
    let now = time::OffsetDateTime::now_utc();
    let today = now.date();
    let new_month = today.month().previous();
    let new_year = if new_month == Month::December {
        today.year() - 1
    } else {
        today.year()
    };
    navigate_to_subdirectory(
        &driver,
        &format!("roster.aspx?{}-{}-01", new_year, new_month as u8),
    )
    .await?;
    wait_until_loaded(&driver).await.unwrap();
    Ok(get_elements(&driver, new_month, new_year, name).await?)
}

/*
Just presses the next button in webcom twice to load the next month.
Only works correctly if the previous month function has been ran before
*/
async fn load_next_month_shifts(driver: &WebDriver, name: String) -> WebDriverResult<Vec<Shift>> {
    println!("Loading Next Month..");
    let now = time::OffsetDateTime::now_utc();
    let today = now.date();
    let new_month = today.month().next();
    let new_year = if new_month == Month::January {
        today.year() + 1
    } else {
        today.year()
    };
    navigate_to_subdirectory(
        &driver,
        &format!("roster.aspx?{}-{}-01", new_year, new_month as u8),
    )
    .await?;
    wait_until_loaded(&driver).await.unwrap();
    Ok(get_elements(&driver, new_month, new_year, name).await?)
}

async fn load_current_month_shifts(driver: &WebDriver, name: String) -> GenResult<Vec<Shift>> {
    let now = time::OffsetDateTime::now_utc();
    let today = now.date();
    Ok(get_elements(&driver, today.month(), today.year(), name).await?)
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
    }).await?;

    if current_url == initial_url {
        println!("Timeout waiting for redirect.");
        return Err(Box::new(WebDriverError::Timeout("Redirect did not occur".into())));
    }

    println!("Redirected to: {}", current_url);
    wait_until_loaded(driver).await?;
    Ok(())
}

async fn heartbeat(reason: FailureType, url: Option<String>) -> GenResult<()> {
    if url.is_none(){println!("no heartbeat URL");return Ok(());}
    reqwest::get(format!(
        "{}?status={}&msg={:?}&ping=",url.unwrap(),
        match reason {
            FailureType::GeckoEngine => "down",
            FailureType::SignInFailed(_) => "down",
            _ => "up",
        },
        reason
    ))
    .await?;
    Ok(())
}

// Loads the sign in failure counter. Creates it if it does not exist
fn load_sign_in_failure_count(path: &Path) -> GenResult<IncorrectCredentialsCount> {
    let failure_count_toml = match std::fs::read_to_string(path) {
        Ok(file_string) => file_string,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            println!("Failure counter not found, creating file..");
            let mut newfile = File::create(path).unwrap();
            let new_counter = toml::to_string(&IncorrectCredentialsCount::new())?;
            write!(&mut newfile,"{}",&new_counter).unwrap();
            new_counter
        }
        Err(_error) => return Err(Box::new(WebDriverError::FatalError("kon bestand niet laden".to_string())))
    };
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
fn check_domain_update(ical_path: &PathBuf, shift: &Shift, name: &str) {
    let previous_domain;
    let path = "./previous_domain";
    match std::fs::read_to_string(path){
        Ok(x) => {println!("{}",&x);
            previous_domain=Some(x)}, 
        Err(_) => previous_domain = None,
    }
    let current_domain = var("DOMAIN").unwrap_or("".to_string());
    if let Some(previous_domain_unwrap) = previous_domain {
        if previous_domain_unwrap != current_domain {
            let _ = send_welcome_mail(ical_path, &name, &shift.name, true);
        }
    }
    match File::create(path){
        Ok(mut file) => {let _ = write!(file, "{}",current_domain);},
        Err(_) => ()
    }
    
}

// If returning true, continue execution
fn sign_in_failed_check(username: &str) -> GenResult<Option<SignInFailure>>{
    let resend_error_mail_count: usize = var("SIGNIN_FAIL_MAIL_REPEAT").unwrap_or("24".to_string()).parse().unwrap_or(2);
    let sign_in_attempt_reduce: usize = var("SIGNIN_FAILED_REDUCE").unwrap_or("2".to_string()).parse().unwrap_or(1);
    let path = Path::new("./sign_in_failure_count.toml");
    // Load the existing counter, create a new one if one doesnt exist yet
    let mut failure_counter = match load_sign_in_failure_count(path){
        Ok(value) => value,
        Err(_) => {
            let new = IncorrectCredentialsCount::new();
            save_sign_in_failure_count(path, &new)?;
            new}
    };
    let return_value: Option<SignInFailure>;
    // else check if retry counter == reduce_ammount, if not, stop running
    if failure_counter.retry_count == 0 {
        return_value = None;
    }
    else if failure_counter.retry_count % sign_in_attempt_reduce == 0 {
        println!("Continuing execution with sign in error, reduce val: {sign_in_attempt_reduce}, current count {}",failure_counter.retry_count);
        failure_counter.retry_count += 1;
        return_value = None;           
    }
    else{
        println!("Skipped execution due to previous sign in error");
        failure_counter.retry_count += 1;
        return_value = Some(failure_counter.error.clone().unwrap());
    }
    if failure_counter.retry_count % resend_error_mail_count == 0 && failure_counter.error.is_some(){
        email::send_failed_signin_mail(username, &failure_counter,false)?;
    }
    save_sign_in_failure_count(path, &failure_counter)?;
    Ok(return_value)
}

fn sign_in_failed_update(username: &str, failed: bool, failure_type: Option<SignInFailure>) -> GenResult<()>{
    let path = Path::new("./sign_in_failure_count.toml");
    let mut failure_counter = load_sign_in_failure_count(path)?;
    // if failed == true, set increment counter and set error
    if failed == true{
        failure_counter.error = failure_type;
        if failure_counter.retry_count == 0{
            failure_counter.retry_count += 1;
            email::send_failed_signin_mail(username, &failure_counter, true)?;
        }
    }
    // if failed == false, reset counter
    else if failed == false{
        if failure_counter.error.is_some() {
            println!("Sign in succesful again!");
            email::send_sign_in_succesful(username)?;
        }
        failure_counter.retry_count = 0;
        failure_counter.error = None;
    }
    save_sign_in_failure_count(path, &failure_counter)?;
    Ok(())
}

// Main program logic that has to run, if it fails it will all be reran.
async fn main_program(
    driver: &WebDriver,
    username: &str,
    password: &str,
) -> GenResult<()> {
    driver.delete_all_cookies().await?;
    // let main_url = format!(
    //     "https://dmz-wbc-web0{}.connexxion.nl/WebComm/default.aspx",
    //     (retry_count % 2) + 1
    // );
    let main_url = format!("webcom.connexxion.nl");
    println!("Loading site: {}..",main_url);
    driver
        .goto(main_url)
        .await?;
    wait_untill_redirect(&driver).await?;
    let name = load_calendar(&driver, &username, &password).await?;
    wait_until_loaded(&driver).await?;
    let mut shifts = load_current_month_shifts(&driver, name.clone()).await?;
    shifts.append(&mut load_previous_month_shifts(&driver, name.clone()).await?);
    shifts.append(&mut load_next_month_shifts(&driver, name.clone()).await?);
    println!("Found {} shifts", shifts.len());
    email::send_emails(&shifts)?;
    save_shifts_on_disk(&shifts, Path::new("./previous_shifts.toml"))?; // We save the shifts before modifying them further to declutter the list. We only need the start and end times of the total shift.
    let shifts = gebroken_shifts::gebroken_diensten_laden(&driver, &shifts).await?; // Replace the shifts with the newly created list of broken shifts
    let shifts = gebroken_shifts::split_night_shift(&shifts);
    let calendar = create_ical(&shifts);
    let ical_path = PathBuf::from(&format!("{}{}.ics", var("SAVE_TARGET")?, username));
    email::send_welcome_mail(&ical_path,username, &name,false)?;
    check_domain_update(&ical_path, &shifts.last().unwrap(), &name);
    let mut output = File::create(&ical_path)?;
    println!("Writing to: {:?}", &ical_path);
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
    let version= var("CARGO_PKG_VERSION").unwrap_or("onbekend".to_string());
    println!("Starting Webcom Ical version {version}");
    let mut error_reason = FailureType::OK;
    let heartbeat_url = var("HEARTBEAT_URL").ok();
    let driver = match initiate_webdriver().await {
        Ok(driver) => driver,
        Err(error) => {
            println!("Kon driver niet opstarten: {:?}", &error);
            send_errors(vec![error], "flats").unwrap();
            error_reason = FailureType::GeckoEngine;
            heartbeat(error_reason,heartbeat_url).await.unwrap();
            return Err(WebDriverError::FatalError("driver fout".to_string()));
        }
    };
    let username = var("USERNAME").unwrap();
    let password = var("PASSWORD").unwrap();
    let mut retry_count: usize = 0;
    let mut running_errors: Vec<Box<dyn std::error::Error>> = vec![];
    let max_retry_count: usize = var("RETRY_COUNT")
        .unwrap_or("3".to_string())
        .parse()
        .unwrap_or(3);

    let start_main = sign_in_failed_check(&username).unwrap();
    if let Some(failure) = start_main {
        retry_count = max_retry_count;
        error_reason = FailureType::SignInFailed(failure);
    }
    while retry_count <= max_retry_count - 1 {
        match main_program(&driver, &username, &password).await {
            Ok(_) => {sign_in_failed_update(&username,false, None).unwrap();
                retry_count = max_retry_count;
                },
            Err(x) => {
                match x.downcast_ref::<FailureType>(){
                Some(FailureType::SignInFailed(y)) => {
                    // Do not stop webcom if the sign in failure reason is unknown
                    if let SignInFailure::Other(x) = y{
                        println!("Kon niet inloggen, maar een onbekende fout: {}. Probeert opnieuw",x)
                    }
                    else{
                        retry_count = max_retry_count;
                        sign_in_failed_update(&username,true, Some(y.clone())).unwrap();
                        error_reason = FailureType::SignInFailed(y.to_owned());
                        println!("Inloggen niet succesvol, fout: {:?}",y)
                    }
                },
                _ => {println!(
                    "Fout tijdens shift laden, opnieuw proberen, poging: {}. Fout: {}",
                    retry_count + 1,
                    &x.to_string()
                );
                running_errors.push(x);}
            }
            }
        };
        retry_count += 1;
    }
    if running_errors.is_empty() {
        println!("Alles is in een keer goed gegaan, jippie!");
    } else if running_errors.len() < max_retry_count {
        println!("Errors have occured, but succeded in the end");
    } else {
        error_reason = FailureType::TriesExceeded;
        match send_errors(running_errors, &username) {
            Ok(_) => (),
            Err(x) => println!("failed to send error email, ironic: {:?}", x),
        }
    }
    heartbeat(error_reason,heartbeat_url).await.unwrap();
    driver.quit().await?;
    Ok(())
}
