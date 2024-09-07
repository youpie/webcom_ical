use time::macros::*;

use dotenvy::dotenv_override;
use dotenvy::var;
use std::path::Path;
use std::str::Split;
use std::time::Duration;
use thirtyfour::prelude::*;

use time::Date;
use time::Time;

#[derive(Debug, Clone)]
pub struct Shift {
    date: Option<Date>,
    start: Time,
    end: Time,
    duration: Duration,
    number: String,
    kind: String,
    location: String,
    description: String,
}

impl Shift {
    fn new(text: String) -> Self {
        let parts = text.split("\u{a0}• \u{a0}• ");
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

        let time_format = format_description!("[hour]:[minute]");
        let start_time_str = time.split_whitespace().nth(0).unwrap();
        let mut end_time_str = time.split_whitespace().nth(2).unwrap();
        if end_time_str == "24" {
            end_time_str = "0"
        }
        println!("time {}", end_time_str);
        let start: Time = Time::parse(start_time_str, time_format).unwrap();
        let end: Time = Time::parse(end_time_str, time_format).unwrap();

        let duration_split = shift_duration.split_whitespace().nth(0).unwrap().split(":");
        println!("split {:?}", duration_split.clone().nth(1).unwrap());
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
            date: None,
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

async fn get_elements(elements: Vec<WebElement>) -> WebDriverResult<()> {
    let mut temp_emlements: Vec<Shift> = vec![];
    for element in elements {
        let text = element.attr("data-original-title").await?.unwrap();
        if !text.is_empty() && text != "Op deze dag bent u afwezig." {
            println!("original string {:?}", &text);
            let new_shift = Shift::new(text);
            temp_emlements.push(new_shift.clone());
            println!(" created shift {:?}", &new_shift);
        }
    }
    Ok(())
}

fn get_month(text: String) -> usize {
    let month = [
        "Januari",
        "Februari",
        "Maart",
        "April",
        "Mei",
        "Juni",
        "Juli",
        "Augustus",
        "September",
        "October",
        "November",
        "December",
    ];
    let month_name = text.split_whitespace().nth(1).unwrap();
    let month_index = month.iter().position(|month| month == &month_name).unwrap() + 1;
    month_index
}

#[tokio::main]
async fn main() -> WebDriverResult<()> {
    dotenv_override().ok();
    let caps = DesiredCapabilities::firefox();
    let driver = WebDriver::new("http://0.0.0.0:4444", caps).await?;
    let username = var("USERNAME").unwrap();
    let password = var("PASSWORD").unwrap();
    load_calendar(&driver, &username, &password).await?;
    let maand = driver
        .find(By::PartialLinkText("Rooster"))
        .await?
        .text()
        .await?;
    println!("{}", get_month(maand));
    let elements = driver
        .query(By::ClassName("calDay"))
        .all_from_selector()
        .await?;
    get_elements(elements).await?;
    driver.screenshot(Path::new("./webpage.png")).await?;
    driver.quit().await?;
    Ok(())
}
