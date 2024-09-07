use dotenvy::dotenv_override;
use dotenvy::var;
use std::path::Path;
use thirtyfour::prelude::*;

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
    driver
        .find(By::Id("ctl00_cntMainBody_lgnView_lnk_1"))
        .await?
        .click()
        .await?;
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
    println!("{}e", month_name);
    let month_index = month.iter().position(|month| month == &month_name).unwrap() + 1;
    println!("{}", month_index);
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
    for element in elements {
        let text = element.attr("data-original-title").await?;
        println!("{:#?}", text);
    }

    driver.screenshot(Path::new("./webpage.png")).await?;
    driver.quit().await?;
    Ok(())
}
