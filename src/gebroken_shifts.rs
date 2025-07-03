use crate::{shift::ShiftState, GenResult, Shift};
use async_recursion::async_recursion;
use dotenvy::var;
use thirtyfour::{
    error::{WebDriverError, WebDriverResult},
    prelude::*,
    WebDriver, WebElement,
};
use time::{macros::format_description, Time};

/*
Main function for loading broken shifts
First visits the web page
Finds the op- and afstaptijden
Creates two new shifts and adds them to the list
Returns the new list
Does not return most errors as there are a few valid reason this function fails
*/
pub async fn gebroken_diensten_laden(
    driver: &WebDriver,
    all_shifts: &mut Vec<Shift>,
) -> WebDriverResult<()> {
    for shift in all_shifts {
        if shift.is_broken && (shift.state == ShiftState::Changed || shift.state == ShiftState::New) {
            info!("Creating broken shift: {}", shift.number);

            match get_broken_shift_time(driver, shift).await {
                Ok(_) => {
                    info!("Added broken shift time to shift {}",shift.number);
                }
                Err(x) => {
                    warn!(
                        "An error occured creating a broken shift: {:?}",
                        x
                    );
                }
            };
            navigate_to_subdirectory(driver, "/WebComm/roster.aspx").await?; //Ga terug naar de rooster pagina, anders laden de gebroken shifts niet goed
            wait_for_response(driver, By::ClassName("calDay"), false).await?
        } else if shift.is_broken && shift.state == ShiftState::Unchanged {
            info!("Shift {} is broken, but unchanged from last check", shift.number);
        }
    }
    info!("Done generating broken shifts");
    Ok(())
}

/*
A small function to combine the three functions needed for creating a broken shift into one match statement
*/
async fn get_broken_shift_time(driver: &WebDriver, shift: &mut Shift) -> GenResult<()> {
    let broken_diensten = load_broken_dienst_page(driver, &shift).await?;
    let between_times = find_broken_start_stop_time(broken_diensten).await?;
    shift.broken_period = Some(between_times);
    Ok(())
}

/*
A function created to overcome a limitation of gnome calendar
https://gitlab.gnome.org/GNOME/gnome-calendar/-/issues/944
Not needed for most people
*/
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

/*
Creates the URL needed for the broken shift
Waits untill the page is fully loaded and then gets and returns all rows of the shift sheet
*/
pub async fn load_broken_dienst_page(
    driver: &WebDriver,
    shift: &Shift,
) -> GenResult<Vec<WebElement>> {
    let date = shift.date;
    let date_format = format_description!("[year]-[month]-[day]");
    let formatted_date = date.format(date_format).unwrap();
    navigate_to_subdirectory(driver, &format!("/WebComm/shift.aspx?{}", formatted_date)).await?;
    //wait_until_loaded(&driver).await?;
    wait_for_response(driver, By::PartialLinkText("Werk en afwezigheden"), true).await?;
    let trip_body = driver.find(By::Tag("tbody")).await?;
    let trip_rows = trip_body.query(By::Tag("tr")).all_from_selector().await?;
    Ok(trip_rows)
}

/*
Finds the the first afstaptijd and second opstaptijd in the shift sheet, converts these to time:time and returns them
returns afstaptijd, opstaptijd in that order
Returns error if only 1 opstap/afstaptijd is found, this is the case when you get assigned half of a broken shfit
*/
pub async fn find_broken_start_stop_time(
    shift_rows: Vec<WebElement>,
) -> WebDriverResult<(Time, Time)> {
    let mut afstaptijden: Vec<String> = vec![];
    let mut opstaptijden: Vec<String> = vec![];
    for row in shift_rows {
        let shift_columns = row.query(By::Tag("td")).all_from_selector().await?;
        if shift_columns.last().unwrap().text().await? == "Afstaptijd" {
            debug!("afstaptijd {}", shift_columns[3].text().await?);
            afstaptijden.push(shift_columns[1].text().await?);
        }
        if shift_columns.last().unwrap().text().await? == "Opstaptijd" {
            debug!("opstaptijd {}", shift_columns[1].text().await?);
            opstaptijden.push(shift_columns[1].text().await?);
        }
    }
    let tijd_formaat = format_description!("[hour]:[minute]");
    match afstaptijden.len() {
        1 => {
            return Err(WebDriverError::FatalError(
                "Not complete broken shift".to_string(),
            ));
        }
        _ => (),
    };
    let afstaptijd = Time::parse(afstaptijden.first().unwrap(), tijd_formaat).unwrap();
    let opstaptijd = Time::parse(opstaptijden.last().unwrap(), tijd_formaat).unwrap();
    Ok((afstaptijd, opstaptijd))
}

/*
A function to navigate to a subdirectory of the current URL
Needed because if the while url is entered, the cookies will be lost and you will have to log in again
*/
pub async fn navigate_to_subdirectory(driver: &WebDriver, subdirectory: &str) -> WebDriverResult<()> {
    let script = format!("window.location.href = '{}';", subdirectory);
    driver.execute(&script, vec![]).await?;
    Ok(())
}

// This function clones a vec of shifts and splits broken shifts, if that value is set
pub fn split_broken_shifts(shifts: Vec<Shift>) -> GenResult<Vec<Shift>> {
    let mut shifts_clone = shifts.clone();
    let mut shifts_to_append = vec![];
    let vec_len = shifts_clone.len() -1;
    for shift in shifts.iter().rev().enumerate() {
        let position = vec_len - shift.0;
        if shift.1.broken_period.is_some() {
            if let Some(mut shifts_split) = shift.1.split_broken() {
                shifts_clone.remove(position);
                shifts_to_append.append(&mut shifts_split);
            }
        }
    }
    shifts_clone.append(&mut shifts_to_append);
    Ok(shifts_clone)
}

/*
A simple function to wait until a page is truly fully loaded
You need to provide a element on the page to wait for
If clickable is false it will only check if it is displayed, not clickable
*/
#[async_recursion]
pub async fn wait_for_response(
    driver: &WebDriver,
    element: By,
    clickable: bool,
) -> WebDriverResult<()> {
    let query = driver.query(element.clone()).first().await?;
    let test = match clickable {
        true => query.wait_until().clickable().await,
        false => query.wait_until().displayed().await,
    };
    match test {
        Err(WebDriverError::ElementClickIntercepted(_)) => {
            wait_for_response(driver, element, clickable).await?;
        }
        x => return x,
    };
    Ok(())
}
