use dotenvy::dotenv_override;
use dotenvy::var;
use icalendar::Calendar;
use icalendar::CalendarDateTime;
use icalendar::Component;
use icalendar::DatePerhapsTime;
use icalendar::Event;
use icalendar::EventLike;
use std::collections::HashMap;
use std::path::Path;
use std::str::Split;
use std::time::Duration;
use thirtyfour::prelude::*;
use time::Month;
use time::OffsetDateTime;
use time::PrimitiveDateTime;
use time::UtcOffset;

use time::Date;
use time::Time;

#[derive(Debug, Clone)]
pub struct Shift {
    start: chrono::DateTime<chrono_tz::Tz>,
    end: chrono::DateTime<chrono_tz::Tz>,
    duration: Duration,
    number: String,
    kind: String,
    location: String,
    description: String,
}

impl Shift {
    fn new(text: String, date: Date) -> Self {
        let parts = text.split("\u{a0}• \u{a0}• ");
        let parts_clean: Vec<String> = parts.map(|x| x.replace("\u{a0}• ", "")).collect();
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

        let duration_split = shift_duration.split_whitespace().nth(0).unwrap().split(":");
        let duration_minutes: u64 = duration_split
            .clone()
            .nth(1)
            .unwrap()
            .parse::<u64>()
            .unwrap()
            * 60;
        let duration_hours: u64 = duration_split
            .clone()
            .nth(0)
            .unwrap()
            .parse::<u64>()
            .unwrap()
            * 60
            * 60;
        let duration = Duration::from_secs(duration_hours + duration_minutes);

        Self {
            date,
            number,
            start,
            end,
            duration,
            kind,
            location,
            description,
        }
    }
}

fn get_time(str_time: &str) -> Time {
    let mut time_split = str_time.split(":");
    let mut hour: u8 = time_split.clone().nth(0).unwrap().parse().unwrap();
    let min: u8 = time_split.nth(1).unwrap().parse().unwrap();
    if hour >= 24 {
        hour = hour - 24;
    }
    Time::from_hms(hour, min, 0).unwrap()
}

async fn load_calendar(driver: &WebDriver, user: &str, pass: &str) -> WebDriverResult<()> {
    driver
        .goto("https://dmz-wbc-web01.connexxion.nl/WebComm/default.aspx")
        .await?;
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
    driver.find(By::LinkText("Rooster")).await?.click().await?;
    Ok(())
}

async fn get_elements(
    elements: Vec<WebElement>,
    month: Month,
    year: u32,
) -> WebDriverResult<Vec<Shift>> {
    let mut temp_emlements: Vec<Shift> = vec![];
    for element in elements {
        let text = element.attr("data-original-title").await?.unwrap();
        if !text.is_empty() && text.contains("Dienstduur") {
            let dag: u8 = element
                .find(By::Tag("strong"))
                .await?
                .text()
                .await?
                .parse()
                .unwrap();
            let date = Date::from_calendar_date(year as i32, month, dag).unwrap();
            let new_shift = Shift::new(text, date);
            temp_emlements.push(new_shift.clone());
            println!("Created shift {:?}", &new_shift);
        }
    }

    Ok(temp_emlements)
}

fn get_month_year(text: &str) -> (Month, u32) {
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
        ("October", Month::October),
        ("November", Month::November),
        ("December", Month::December),
    ]);
    let month_name = text.split_whitespace().nth(1).unwrap();
    let year: u32 = text.split_whitespace().nth(2).unwrap().parse().unwrap();
    let month = month_dict.get(month_name).unwrap();
    (*month, year)
}

fn create_ical(shifts: Vec<Shift>) {
    let mut calendar = Calendar::new().name("Hermes rooster").done();
    for shift in shifts {
        calendar.push(
            Event::new()
                .summary(&format!("Shift - {}", shift.number))
                .description(&format!(
                    "Dienstsoort: {} \n Duur: {} \n Omschrijving: {}",
                    shift.kind,
                    (shift.duration.as_secs() / 60 / 24),
                    shift.description
                ))
                .location(&shift.location)
                .starts()
                .done(),
        );
    }
}

fn create_dateperhapstime(date: Date, time: Time) -> DatePerhapsTime {
    CalendarDateTime::from_ym
}

#[tokio::main]
async fn main() -> WebDriverResult<()> {
    dotenv_override().ok();
    let caps = DesiredCapabilities::firefox();
    let driver = WebDriver::new("http://0.0.0.0:4444", caps).await?;
    let username = var("USERNAME").unwrap();
    let password = var("PASSWORD").unwrap();
    load_calendar(&driver, &username, &password).await?;
    let month_year = driver
        .find(By::PartialLinkText("Rooster"))
        .await?
        .text()
        .await?;
    let (month, year) = get_month_year(&month_year);
    let elements = driver
        .query(By::ClassName("calDay"))
        .all_from_selector()
        .await?;
    get_elements(elements, month, year).await?;
    driver.screenshot(Path::new("./webpage.png")).await?;
    driver.quit().await?;
    Ok(())
}
