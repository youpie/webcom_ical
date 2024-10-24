use chrono::NaiveDate;
use chrono::NaiveDateTime;
use chrono::NaiveTime;
use dotenvy::dotenv_override;
use dotenvy::var;
use email::send_errors;
use icalendar::Calendar;
use icalendar::CalendarDateTime;
use icalendar::Component;
use icalendar::Event;
use icalendar::EventLike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::hash::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::io::Write;
use std::path::Path;
use std::str::Split;
use thirtyfour::prelude::*;
use time::macros::format_description;
use time::Duration;
use time::Month;

use time::Date;
use time::Time;

pub mod email;
pub mod gebroken_shifts;

type GenResult<T> = Result<T, Box<dyn std::error::Error>>;

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
        let location: String = parts_list[7].nth(1).unwrap_or("").to_string();
        let description: String = parts_list[8].nth(1).unwrap_or("").to_string();
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

// Creates and returns a Time::time from a given string of time eg: 12:34
// Uses A LOT of unwraps, so can easilly fail. :)
fn get_time(str_time: &str) -> Time {
    let mut time_split = str_time.split(":");
    let mut hour: u8 = time_split.clone().nth(0).unwrap().parse().unwrap();
    let min: u8 = time_split.nth(1).unwrap().parse().unwrap();
    if hour >= 24 {
        hour = hour - 24;
    }
    Time::from_hms(hour, min, 0).unwrap()
}

/*
Logs into webcom, has no logic for when the login fails.
It will also find and return the first name of the user, this will fail if the login is unsuccesful
*/
async fn load_calendar(driver: &WebDriver, user: &str, pass: &str) -> WebDriverResult<String> {
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
    gebroken_shifts::wait_for_response(driver, By::Tag("h3"), false).await?;
    let name_text = driver.find(By::Tag("h3")).await?.text().await?;
    let name = name_text
        .split_whitespace()
        .nth(0)
        .unwrap()
        .split(",")
        .last()
        .unwrap()
        .to_string();
    println!("{}", name_text);
    let rooster_knop = driver.query(By::LinkText("Rooster")).first().await?;
    rooster_knop.wait_until().displayed().await?;
    rooster_knop.click().await?;
    Ok(name)
}

/*
Checks all supplied WebElements, it checks if the day contains the text "Dienstuur"  and if so, adds it to a Vec of valid shifts in the calendar
Does not search itself for elements
*/
async fn get_elements(
    elements: Vec<WebElement>,
    month: Month,
    year: u32,
    name: String,
) -> WebDriverResult<Vec<Shift>> {
    let mut temp_emlements: Vec<Shift> = vec![];
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
            let dag_text_split = dag_text.split_whitespace().nth(0).unwrap();

            // println!("dag {}", &dag_text);
            let dag: u8 = dag_text_split.parse().unwrap();
            let date = Date::from_calendar_date(year as i32, month, dag).unwrap();
            let new_shift = Shift::new(text, date, &name);
            temp_emlements.push(new_shift.clone());
            // println!("Created shift {:?}", &new_shift);
        }
    }

    Ok(temp_emlements)
}

/*
The webdriver needs to be at a calendar view. It loads the current month of the calendar
Uses a really simple way (a dictionary) to convert a string of a month to time::Month
*/
async fn get_month_year(driver: &WebDriver) -> WebDriverResult<(Month, u32)> {
    let month_dict = HashMap::from([
        ("Januari", Month::January),
        ("Februari", Month::February),
        ("Maart", Month::March),
        ("April", Month::April),
        ("Mei", Month::May),
        ("Juni", Month::June),
        ("Juli", Month::July),
        ("Augustus", Month::August),
        ("September", Month::September),
        ("Oktober", Month::October),
        ("November", Month::November),
        ("December", Month::December),
    ]);
    let text = driver
        .find(By::PartialLinkText("Rooster"))
        .await?
        .text()
        .await?;
    println!("Loading: {}", text);
    let month_name = text.split_whitespace().nth(1).unwrap();
    let year: u32 = text.split_whitespace().nth(2).unwrap().parse().unwrap();
    let month = month_dict.get(month_name).unwrap();
    Ok((*month, year))
}

/*
Creates the ICAL file to add to the calendar
*/
fn create_ical(shifts: &Vec<Shift>) -> String {
    println!("Creating calendar file...");
    let mut calendar = Calendar::new()
        .name("Hermes rooster")
        .timezone("Europe/Amsterdam")
        .done();
    for shift in shifts {
        let date_format = format_description!("[year]-[month]-[day]");
        let shift_link = format!(
            "https://dmz-wbc-web01.connexxion.nl/WebComm/shiprint.aspx?{}",
            shift.date.format(date_format).unwrap()
        );
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
This really ugly function converts between the two.
*/
fn create_dateperhapstime(date: Date, time: Time) -> CalendarDateTime {
    let months = [
        Month::January,
        Month::February,
        Month::March,
        Month::April,
        Month::May,
        Month::June,
        Month::July,
        Month::August,
        Month::September,
        Month::October,
        Month::November,
        Month::December,
    ];
    let date_day = date.day();
    let date_month = months
        .iter()
        .position(|month| month == &date.month())
        .unwrap()
        + 1;
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
async fn load_previous_month(driver: &WebDriver, name: String) -> WebDriverResult<Vec<Shift>> {
    driver
        .find(By::Id("ctl00_ctl00_navilink0"))
        .await?
        .click()
        .await?;
    gebroken_shifts::wait_for_response(driver, By::ClassName("calDay"), false).await?;
    let elements = driver
        .query(By::ClassName("calDay"))
        .all_from_selector()
        .await?;
    let (month, year) = get_month_year(driver).await?;
    Ok(get_elements(elements, month, year, name).await?)
}

/*
Just presses the next button in webcom twice to load the next month.
Only works correctly if the previous month function has been ran before
*/
async fn load_next_month(driver: &WebDriver, name: String) -> WebDriverResult<Vec<Shift>> {
    for _i in 0..2 {
        driver
            .find(By::Id("ctl00_ctl00_navilink1"))
            .await?
            .click()
            .await?;
    }
    gebroken_shifts::wait_for_response(driver, By::ClassName("calDay"), false).await?;
    let elements = driver
        .query(By::ClassName("calDay"))
        .all_from_selector()
        .await?;
    let (month, year) = get_month_year(driver).await?;
    Ok(get_elements(elements, month, year, name).await?)
}

/*
Serialise the shifts to be saved to disk.
This is needed to send a mail when a new shift is found
Needs to create a struct with a Vec<Shift> because otherwise it wouldn't serialise correctly
*/
fn save_shifts_on_disk(shifts: &Vec<Shift>, path: &Path) -> GenResult<()> {
    let shifts_struct = Shifts::new(shifts.clone());
    let shifts_serialised = toml::to_string(&shifts_struct).unwrap();
    let mut output = File::create(path).unwrap();
    write!(output, "{}", shifts_serialised).unwrap();
    Ok(())
}

// Main program logic that has to run, if it fails it will all be reran.
async fn main_program(driver: &WebDriver, username: &str, password: &str) -> GenResult<()> {
    driver.delete_all_cookies().await?;
    driver
        .goto("https://dmz-wbc-web01.connexxion.nl/WebComm/default.aspx?TestingCookie=1")
        .await?;
    let name = load_calendar(&driver, &username, &password).await?;
    driver.execute("return document.readyState", vec![]).await?;
    let (month, year) = get_month_year(&driver).await?;
    let elements = driver
        .query(By::ClassName("calDay"))
        .all_from_selector()
        .await?;
    let mut shifts = get_elements(elements, month, year, name.clone()).await?;
    shifts.append(&mut load_previous_month(&driver, name.clone()).await?);
    shifts.append(&mut load_next_month(&driver, name).await?);
    println!("Found {} shifts", shifts.len());
    email::send_emails(&shifts)?;
    save_shifts_on_disk(&shifts, Path::new("./previous_shifts.toml"))?; // We save the shifts before modifying them further to declutter the list. We only need the start and end times of the total shift.
    let shifts = gebroken_shifts::gebroken_diensten_laden(&driver, &shifts).await?; // Replace the shifts with the newly created list of broken shifts
    let shifts = gebroken_shifts::split_night_shift(&shifts);
    let calendar = create_ical(&shifts);
    let ical_path = &format!("{}{}.ics", var("SAVE_TARGET")?, username);
    let mut output = File::create(ical_path)?;
    println!("Writing to: {:?}", output);
    write!(output, "{}", calendar)?;
    Ok(())
}

/*
This starts the WebDriver session
Loads the main logic, and retries if it fails
*/
#[tokio::main]
async fn main() -> WebDriverResult<()> {
    dotenv_override().ok();
    let gecko_ip = var("GECKO_IP").unwrap();
    let caps = DesiredCapabilities::firefox();
    let driver = WebDriver::new(format!("http://{}", gecko_ip), caps).await?;
    let username = var("USERNAME").unwrap();
    let password = var("PASSWORD").unwrap();
    let mut retry_count: usize = 0;
    let mut running_errors: Vec<Box<dyn std::error::Error>> = vec![];
    let max_retry_count: usize = var("RETRY_COUNT")
        .unwrap_or("3".to_string())
        .parse()
        .unwrap();
    while retry_count <= max_retry_count - 1 {
        match main_program(&driver, &username, &password).await {
            Ok(_) => retry_count = max_retry_count,
            Err(x) => {
                println!(
                    "Fout tijdens shift laden, opnieuw proberen, poging: {}. Fout: {:?}",
                    retry_count + 1,
                    &x
                );
                running_errors.push(x);
            }
        };
        retry_count += 1;
    }
    if running_errors.is_empty() {
        println!("Alles is in een keer goed gegaan, jippie!");
    } else if running_errors.len() < 3 {
        println!("Errors have occured, but succeded in the end");
    } else {
        match send_errors(running_errors, &username) {
            Ok(_) => (),
            Err(x) => println!("failed to send error email, ironic: {:?}", x),
        }
    }
    driver.quit().await?;
    Ok(())
}
