use reqwest;
use std::error::Error;

async fn send_request() -> Result<String, Box<dyn Error>> {
    Ok(
        reqwest::get("https://webcom.connexxion.nl/WebCommWebBalancer")
            .await?
            .text()
            .await?,
    )
}

#[tokio::main]
async fn main() {
    let response = send_request().await;
    println!("{}", scraper::Html::parse_document(&response.unwrap()));
}
