use async_recursion::async_recursion;
use dotenvy::var;
use std::path::Path;
use thirtyfour::{
    error::{WebDriverError, WebDriverResult},
    prelude::*,
    WebDriver, WebElement,
};
use time::{macros::format_description, Time};

use crate::Shift;

pub async fn gebroken_diensten_laden(driver: &WebDriver, shifts: &Vec<Shift>) -> Vec<Shift> {
    let mut new_shifts: Vec<Shift> = vec![];
    for shift in shifts {
        if shift.is_broken {
            println!("Creating broken shift: {}", shift.number);
            let shift_rows = load_broken_dienst_page(driver, &shift).await.unwrap();
            let between_times = find_broken_start_stop_time(shift_rows).await.unwrap();
            let broken_shifts = Shift::new_from_existing(between_times, shift, false);
            new_shifts.extend(broken_shifts);
        } else {
            new_shifts.push(shift.clone());
        }
    }
    new_shifts
}

pub fn split_night_shift(shifts: &Vec<Shift>) -> Vec<Shift> {
    let split_option = var("BREAK_UP_NIGHT_SHIFT").unwrap();
    let mut temp_shift: Vec<Shift> = vec![];
    if split_option == "true" {
        for shift in shifts {
            if shift.end_date != shift.date {
                let split_shift = Shift::new_from_existing(
                    (
                        Time::from_hms(0, 0, 0).unwrap(),
                        Time::from_hms(0, 0, 0).unwrap(),
                    ),
                    shift,
                    true,
                );
                temp_shift.extend(split_shift);
            } else {
                temp_shift.push(shift.clone());
            }
        }
    } else {
        temp_shift = shifts.clone();
    }
    temp_shift
}

pub async fn load_broken_dienst_page(
    driver: &WebDriver,
    shift: &Shift,
) -> WebDriverResult<Vec<WebElement>> {
    let date = shift.date;
    let date_format = format_description!("[year]-[month]-[day]");
    let formatted_date = date.format(date_format).unwrap();
    navigate_to_subdirectory(driver, &format!("/WebComm/shift.aspx?{}", formatted_date)).await?;
    wait_for_response(driver).await?;
    let trip_body = driver.find(By::Tag("tbody")).await?;
    let trip_rows = trip_body.query(By::Tag("tr")).all_from_selector().await?;
    Ok(trip_rows)
}

pub async fn find_broken_start_stop_time(
    shift_rows: Vec<WebElement>,
) -> WebDriverResult<(Time, Time)> {
    let mut afstaptijden: Vec<String> = vec![];
    let mut opstaptijden: Vec<String> = vec![];
    for row in shift_rows {
        let shift_columns = row.query(By::Tag("td")).all_from_selector().await?;
        if shift_columns.last().unwrap().text().await? == "Afstaptijd" {
            //println!("afstaptijd {}", shift_columns[3].text().await?);
            afstaptijden.push(shift_columns[1].text().await?);
        }
        if shift_columns.last().unwrap().text().await? == "Opstaptijd" {
            //println!("opstaptijd {}", shift_columns[1].text().await?);
            opstaptijden.push(shift_columns[1].text().await?);
        }
    }
    let tijd_formaat = format_description!("[hour]:[minute]");
    let afstaptijd = Time::parse(afstaptijden.first().unwrap(), tijd_formaat).unwrap();
    let opstaptijd = Time::parse(opstaptijden.last().unwrap(), tijd_formaat).unwrap();
    Ok((afstaptijd, opstaptijd))
}

async fn navigate_to_subdirectory(driver: &WebDriver, subdirectory: &str) -> WebDriverResult<()> {
    let script = format!("window.location.href = '{}';", subdirectory);
    driver.execute(&script, vec![]).await?;
    Ok(())
}

#[async_recursion]
async fn wait_for_response(driver: &WebDriver) -> WebDriverResult<()> {
    let query = driver
        .query(By::PartialLinkText("Werk en afwezigheden"))
        .first()
        .await?;
    let test = query.wait_until().clickable().await;
    match test {
        Err(WebDriverError::ElementClickIntercepted(_)) => {
            wait_for_response(driver).await?;
        }
        x => return x,
    };
    Ok(())
}
