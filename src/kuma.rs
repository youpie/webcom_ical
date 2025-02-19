use std::path::PathBuf;
use std::str::FromStr;
use kuma_client::{monitor, tag, Client};
use serde::{Deserialize, Serialize};
use url::Url;
use std::fs::File;
use std::io::Write;
use dotenvy::var;

type GenResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug,Serialize,Deserialize)]
struct KumaData{
    push_url: Url
}

impl KumaData{
    fn save(&self ,path: PathBuf) -> GenResult<()>{
        let failure_counter_serialised = toml::to_string(self)?;
        let mut output = File::create(path).unwrap();
        write!(output, "{}", failure_counter_serialised)?;
        Ok(())
    }

    fn load(&self, path: PathBuf) -> GenResult<Self> {
        let self_toml = std::fs::read_to_string(path)?;
        let self_struct: Self = toml::from_str(&self_toml)?;
        Ok(self_struct)
    }
}

pub async fn first_run(path: PathBuf, url: Url, personeelsnummer: &str) -> GenResult<()>{
    // If kuma preferences already exists, skip
    if path.exists() {
        return Ok(());
    }

    let username = var("KUMA_USERNAME")?;
    let password = var("KUMA_PASSWORD")?;
    let kuma_client = connect_to_kuma(url, username, password).await?;
    create_monitor(kuma_client, personeelsnummer).await?;
    Ok(())
}

async fn connect_to_kuma(url: Url, username: String, password: String) -> GenResult<Client> {
    Ok(Client::connect(kuma_client::Config { url: url, username: Some(username), password: Some(password),..Default::default()}).await?)
}

async fn create_monitor(kuma_client: Client, personeelsnummer: &str) -> GenResult<Url> {
    let heartbeat_interval: i32 = var("KUMA_HEARTBEAT_INTERVAL")?.parse()?;
    let heartbeat_retry: i32 = var("KUMA_HEARTBEAT_RETRY")?.parse()?;
    let tag = tag::Tag{
        name: Some("Webcom Ical".to_string()),
        ..Default::default()
    };
    let monitor = monitor::MonitorPush{
        name: Some(personeelsnummer.to_string()),
        interval: Some(heartbeat_interval),
        max_retries: Some(heartbeat_retry),
        tags: vec![tag],
        push_token: Some(personeelsnummer.to_string()),
        ..Default::default()
    };
    let monitor_response = kuma_client.add_monitor(monitor).await?;
    println!("Monitor response: {:?}",monitor_response);
    Ok(Url::from_str("https://google.com").unwrap())
}