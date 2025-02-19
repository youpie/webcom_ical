use dotenvy::var;
use kuma_client::{monitor, Client, notification};
use serde::{Deserialize, Serialize};
use strfmt::strfmt;
use std::collections::HashMap;
use std::fs::{File,read_to_string};
use std::io::Write;
use std::path::PathBuf;
use url::Url;

use crate::email;

type GenResult<T> = Result<T, Box<dyn std::error::Error>>;

const COLOR_RED: &str = "#a51d2d";
const COLOR_GREEN: &str = "#26a269";

#[derive(Debug, Serialize, Deserialize)]
struct KumaData {
    push_url: Url,
}

impl KumaData {
    fn save(&self, path: PathBuf) -> GenResult<()> {
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

pub async fn first_run(path: PathBuf, url: Url, personeelsnummer: &str) -> GenResult<()> {
    // If kuma preferences already exists, skip
    if path.exists() {
        return Ok(());
    }

    let username = var("KUMA_USERNAME")?;
    let password = var("KUMA_PASSWORD")?;
    let kuma_client = connect_to_kuma(&url, username, password).await?;
    let notification_id = create_notification(&kuma_client, personeelsnummer,&url).await?;
    create_monitor(&kuma_client, personeelsnummer,notification_id).await?;
    Ok(())
}

async fn connect_to_kuma(url: &Url, username: String, password: String) -> GenResult<Client> {
    Ok(Client::connect(kuma_client::Config {
        url: url.to_owned(),
        username: Some(username),
        password: Some(password),
        ..Default::default()
    })
    .await?)
}

async fn create_monitor(kuma_client: &Client, personeelsnummer: &str, notification_id: i32) -> GenResult<()> {
    let heartbeat_interval: i32 = var("KUMA_HEARTBEAT_INTERVAL")?.parse()?;
    let heartbeat_retry: i32 = var("KUMA_HEARTBEAT_RETRY")?.parse()?;

    let monitor = monitor::MonitorPush {
        name: Some(personeelsnummer.to_string()),
        interval: Some(heartbeat_interval),
        max_retries: Some(heartbeat_retry),
        push_token: Some(personeelsnummer.to_string()),
        notification_id_list: Some(HashMap::from([(notification_id.to_string(),true)])),
        ..Default::default()
    };
    let monitor_response = kuma_client.add_monitor(monitor).await?;
    println!("Monitor response: {:?}", monitor_response);
    Ok(())
}

async fn create_notification(kuma_client: &Client, personeelsnummer: &str, kuma_url: &Url) -> GenResult<i32> {
    let base_html = read_to_string("./templates/email_base.html").unwrap();
    let offline_html = read_to_string("./templates/kuma_offline.html").unwrap();
    let online_html = read_to_string("./templates/kuma_online.html").unwrap();

    let body_online = strfmt!(&base_html,
        content => strfmt!(&online_html,
            kuma_url => kuma_url.to_string()
        )?,
        banner_color => COLOR_GREEN 
    )?;
    let body_offline = strfmt!(&base_html,
        content => strfmt!(&offline_html,
            kuma_url => kuma_url.to_string(),
            msg => "{{msg}}"
        )?,
        banner_color => COLOR_RED 
    )?;
    let body = format!("{{% if status contains \"Up\" %}}
{body_online}
{{% else %}}
{body_offline}
{{% endif %}}");

    let email_env = email::EnvMailVariables::new()?;

    let config = serde_json::json!({
        "smtpHost": email_env.smtp_server,
        "smtpPort": 465,
        "smtpUsername": email_env.smtp_username,
        "smtpPassword": email_env.smtp_password,
        "smtpTo": email_env.mail_to,
        "smtpFrom": "Uptime <shifts@emphisia.nl>",
        "customBody": body,
        "customSubject": "{% if status contains \"Up\" %}
Webcom Ical weer online
{% else %}
!! Webcom Ical offline !!
{% endif %}",
        "type": "smtp",
        "smtpSecure": true

    });
    let notification = notification::Notification{
        name: Some(format!("{}_mail",personeelsnummer.to_string())),
        config: Some(config),
        ..Default::default()
    };
    let notification_response = kuma_client.add_notification(notification.clone()).await?;
    Ok(notification_response.id.unwrap())
}
