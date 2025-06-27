use dotenvy::var;
use kuma_client::monitor::{MonitorGroup, MonitorType};
use kuma_client::{monitor, Client, notification};
use serde::{Deserialize, Serialize};
use strfmt::strfmt;
use std::collections::HashMap;
use std::fs::{read_to_string, File};
use std::io::Write;
use std::path::PathBuf;
use url::Url;

use crate::email;

type GenResult<T> = Result<T, Box<dyn std::error::Error>>;

const COLOR_RED: &str = "#a51d2d";
const COLOR_GREEN: &str = "#26a269";

const KUMA_DATA_PATH: &str = "./kuma/kuma_data.toml";

#[derive(Debug, Serialize, Deserialize)]
struct KumaData {
    monitor_id: i32,
}

impl KumaData {
    fn save(&self, path: PathBuf) -> GenResult<()> {
        let failure_counter_serialised = toml::to_string(self)?;
        let mut output = File::create(path).unwrap();
        write!(output, "{}", failure_counter_serialised)?;
        Ok(())
    }

    fn _load(&self, path: PathBuf) -> GenResult<Self> {
        let self_toml = std::fs::read_to_string(path)?;
        let self_struct: Self = toml::from_str(&self_toml)?;
        Ok(self_struct)
    }
}

pub async fn first_run(url: &str, personeelsnummer: &str) -> GenResult<()> {
    // If kuma preferences already exists, skip
    let data_pathbuf = PathBuf::from(KUMA_DATA_PATH);
    if data_pathbuf.exists() {
        info!("Kuma ID found on disk");
        return Ok(());
    }
    let url: Url = url.parse().unwrap();
    warn!("Kuma ID not found on disk");
    let username = var("KUMA_USERNAME")?;
    let password = var("KUMA_PASSWORD")?;
    let kuma_client = connect_to_kuma(&url, username, password).await?;
    if let Some(monitor_id) = get_monitor_type_id(&kuma_client, personeelsnummer, MonitorType::Push, false).await?{
        info!("id found in kuma online, saving to disk. ID: {monitor_id}");
        KumaData{monitor_id}.save(PathBuf::from(KUMA_DATA_PATH))?;
        return Ok(())
    }
    let notification_id = create_notification(&kuma_client, personeelsnummer,&url).await?;
    let monitor_id = create_monitor(&kuma_client, personeelsnummer,notification_id).await?;
    KumaData{monitor_id}.save(PathBuf::from(KUMA_DATA_PATH))?;
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

async fn create_monitor(kuma_client: &Client, personeelsnummer: &str, notification_id: i32) -> GenResult<i32> {
    let heartbeat_interval: i32 = var("KUMA_HEARTBEAT_INTERVAL")?.parse()?;
    let heartbeat_retry: i32 = var("KUMA_HEARTBEAT_RETRY")?.parse()?;
    let group_id: i32 = get_monitor_type_id(kuma_client, "Webcom Ical", MonitorType::Group,true).await?.unwrap();
    let monitor = monitor::MonitorPush {
        name: Some(personeelsnummer.to_string()),
        interval: Some(heartbeat_interval),
        max_retries: Some(heartbeat_retry),
        retry_interval: Some(heartbeat_interval),
        push_token: Some(personeelsnummer.to_string()),
        notification_id_list: Some(HashMap::from([(notification_id.to_string(),true)])),
        parent: Some(group_id),
        ..Default::default()
    };
    let monitor_response = kuma_client.add_monitor(monitor).await?;
    let monitor_id = monitor_response.common().id().unwrap();
    info!("Monitor has been created, id: {monitor_id}");
    Ok(monitor_id)
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
    let kuma_url_string = kuma_url.to_string();
//     let body = format!("{{% if status contains \"Up\" %}}
// Hoi,
// Webcom Ical heeft weer wat van zich laten horen! Je krijgt weer mailtjes bij nieuwe diensten en je agenda zal weer geüpdate worden!

// Bekijk de actuele status op: {kuma_url_string}
// {{% else %}}
// Hoi,
// Webcom Ical heeft al een tijdje niks van zich laten horen, hierdoor krijg je waarschijnlijk geen mails bij nieuwe diensten, en wordt je agenda niet meer geüpdate tot nader bericht.

// Bekijk de actuele status op: {kuma_url_string}

// De fout is: {{{{msg}}}}
// {{% endif %}}");

    let email_env = email::EnvMailVariables::new(true)?;
    let port = var("KUMA_MAIL_PORT")?;
    let secure = match var("KUMA_MAIL_SECURE").unwrap_or(var("KUMA_MAUL_SECURE").unwrap_or("true".into())).as_str(){
        "true" => true,
        _ => false
    };
    let config = serde_json::json!({
        "smtpHost": email_env.smtp_server,
        "smtpPort": port,
        "smtpUsername": email_env.smtp_username,
        "smtpPassword": email_env.smtp_password,
        "smtpTo": email_env.mail_to,
        "smtpFrom": email_env.mail_from,
        "customBody": body,
        "customSubject": "{% if status contains \"Up\" %}
Webcom Ical weer online
{% else %}
!! Webcom Ical OFFline !!
{% endif %}",
        "type": "smtp",
        "smtpSecure": secure

    });
    let notification = notification::Notification{
        name: Some(format!("{}_mail",personeelsnummer.to_string())),
        config: Some(config),
        ..Default::default()
    };
    
    let notification_response = kuma_client.add_notification(notification.clone()).await?;
    let id = notification_response.id.unwrap();
    warn!("Created notification with id {id}");
    Ok(id)
}

async fn get_monitor_type_id(kuma_client: &Client, group_name: &str, monitor_type: MonitorType, create_new: bool) -> GenResult<Option<i32>> {
    let current_monitors = kuma_client.get_monitors().await?;
    // Check if a group with the same name of "group_name" exists
    for (_id, monitor) in current_monitors.into_iter(){
        if monitor.monitor_type() == monitor_type{
            if monitor.common().name() == &Some(group_name.to_string()) {
                info!("Existing monitor group has been found");
                return Ok(Some(monitor.common().id().unwrap()));
            }
        }
    }
    info!("Monitor group has not been found");
    // otherwise create a new one
    if create_new {
        
        let new_monitor = kuma_client.add_monitor(MonitorGroup{
            name: Some(group_name.to_string()),
            ..Default::default()
        }).await?;
        let id = new_monitor.common().id().unwrap();
        info!(", created new one with id {id}");
        return Ok(Some(new_monitor.common().id().unwrap()));
    }
    info!(", not creating new one");
    Ok(None)
}