use async_recursion::async_recursion;
use thirtyfour::error::{WebDriverError, WebDriverResult};
use thirtyfour::{By, WebDriver};
use time::{Date, Month};
use thirtyfour::prelude::ElementQueryable;
use crate::gebroken_shifts::{navigate_to_subdirectory, wait_for_response};
use crate::{check_if_webcom_unavailable, check_sign_in_error, set_get_name, wait_until_loaded, FailureType, GenResult, Shift};

/*
Checks all supplied WebElements, it checks if the day contains the text "Dienstuur"  and if so, adds it to a Vec of valid shifts in the calendar
Does not search itself for elements
*/
async fn get_elements(
    driver: &WebDriver,
    month: Month,
    year: i32,
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
            debug!("Loading shift: {:?}", &text);
            let dag_text = element.find(By::Tag("strong")).await?.text().await?;
            let dag_text_split = dag_text.split_whitespace().next().unwrap();

            debug!("dag {}", &dag_text);
            let dag: u8 = dag_text_split.parse().unwrap();
            let date = Date::from_calendar_date(year, month, dag).unwrap();
            let new_shift = Shift::new(text, date);
            temp_emlements.push(new_shift.clone());
            debug!("Found Shift {}", &new_shift.number);
        }
    }
    Ok(temp_emlements)
}

/*
Just presses the previous button in webcom to load the previous month
*/
#[async_recursion]
pub async fn load_previous_month_shifts(
    driver: &WebDriver,
    extra_months_back: usize
) -> GenResult<Vec<Shift>> {
    debug!("Loading Previous Month..");
    let now = time::OffsetDateTime::now_utc();
    let today = now.date();
    let mut new_month = today.month().previous();
    let mut new_year = if new_month == Month::December {
            today.year() - 1
        } else {
            today.year()
        };;
    for _ in 0..extra_months_back {
        info!("Going way back");
        new_month = new_month.previous();

        new_year = if new_month == Month::December {
            new_year - 1
        } else {
            new_year
        };
    }
    let mut shifts = vec![]; 
    if extra_months_back > 0 {
        shifts.append(&mut load_previous_month_shifts(driver, extra_months_back - 1).await?)
    }
    navigate_to_subdirectory(
        &driver,
        &format!("roster.aspx?{}-{}-01", new_year, new_month as u8),
    )
        .await?;
    wait_until_loaded(&driver).await.unwrap();
    shifts.append(&mut get_elements(&driver, new_month, new_year).await?);
    Ok(shifts)
}

/*
Just presses the next button in webcom twice to load the next month.
Only works correctly if the previous month function has been ran before
*/
pub async fn load_next_month_shifts(driver: &WebDriver) -> WebDriverResult<Vec<Shift>> {
    debug!("Loading Next Month..");
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
    Ok(get_elements(&driver, new_month, new_year).await?)
}

pub async fn load_current_month_shifts(driver: &WebDriver) -> GenResult<Vec<Shift>> {
    let now = time::OffsetDateTime::now_utc();
    let today = now.date();
    Ok(get_elements(&driver, today.month(), today.year()).await?)
}

/*
Logs into webcom, has no logic for when the login fails.
It will also find and return the first name of the user, this will fail if the login is unsuccesful
*/
pub async fn load_calendar(driver: &WebDriver, user: &str, pass: &str) -> GenResult<()> {
    info!("Logging in..");
    sign_in_webcom(driver, user, pass).await?;
    //wait_until_loaded(&driver).await?;

    //println!("{}", name_text);
    // let rooster_knop = driver.query(By::LinkText("Rooster")).first().await?;
    // rooster_knop.wait_until().displayed().await?;
    // rooster_knop.click().await?;
    info!("Loading rooster..");
    navigate_to_subdirectory(driver, "roster.aspx").await?;
    Ok(())
}



async fn sign_in_webcom(driver: &WebDriver, user: &str, pass: &str) -> GenResult<()> {
    let possible_error = match driver.find(By::Id("h3")).await{
        Ok(element) => Some(element.text().await.unwrap_or("GEEN TEKST".to_owned())),
        Err(_) => None
    };
    let username_field = driver
        .find(By::Id("ctl00_cntMainBody_lgnView_lgnLogin_UserName"))
        .await.map_err(|error| {
            match check_if_webcom_unavailable(possible_error){
                true => Box::new(FailureType::SignInFailed(crate::SignInFailure::WebcomDown)),
                false => Box::new(FailureType::Other(error.to_string()))
            }
        })?;
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
    debug!("waiting until login page is loaded");
    //wait_until_loaded(&driver).await?;
    let _ = wait_for_response(&driver, By::Tag("h3"), false).await;
    debug!("login page is loaded");
    let name_text = match driver.find(By::Tag("h3")).await {
        Ok(element) => element.text().await?,
        Err(_) => {
            return Err(Box::new(check_sign_in_error(driver).await?));
        }
    };
    let name = name_text
        .split(",")
        .last()
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();
    set_get_name(Some(name));
    Ok(())
}