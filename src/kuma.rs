use dotenvy::var;
use kuma_client::monitor::{MonitorGroup, MonitorType};
use kuma_client::{monitor, Client, notification};
use strfmt::strfmt;
use std::collections::HashMap;
use std::fs::read_to_string;
use url::Url;

use crate::{email, set_get_name};

type GenResult<T> = Result<T, Box<dyn std::error::Error>>;

const COLOR_RED: &str = "#a51d2d";
const COLOR_GREEN: &str = "#26a269";

pub async fn first_run(url: &str, personeelsnummer: &str) -> GenResult<()> {
    let url: Url = url.parse()?;
    let username = var("KUMA_USERNAME")?;
    let password = var("KUMA_PASSWORD")?;
    let kuma_client = connect_to_kuma(&url, username, password).await?;
    let notification_id = create_notification(&kuma_client, personeelsnummer,&url).await?;
    if let Some(monitor_id) = get_monitor_type_id(&kuma_client, personeelsnummer, MonitorType::Push, false).await?{
        debug!("id found in kuma online, ID: {monitor_id}");
        if notification_id.1 {
            info!("Assigning new notification to monitor");
            let mut monitor = kuma_client.get_monitor(monitor_id).await?;
            *monitor.common_mut().notification_id_list_mut() = Some(HashMap::from([(notification_id.0.to_string(),true)]));
            kuma_client.edit_monitor(monitor).await?;
        }
        
        return Ok(())
    }
    let _monitor_id = create_monitor(&kuma_client, personeelsnummer,notification_id.0).await?;
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
    let group_id: i32 = get_monitor_type_id(kuma_client, "Webcom Ical", MonitorType::Group,true).await?.unwrap_or_default();
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

// Create a new notification if it does not already exist. The second value tells that a new notification has been created
async fn create_notification(kuma_client: &Client, personeelsnummer: &str, kuma_url: &Url) -> GenResult<(i32, bool)> {
    let base_html = read_to_string("./templates/email_base.html").unwrap();
    let offline_html = read_to_string("./templates/kuma_offline.html").unwrap();
    let online_html = read_to_string("./templates/kuma_online.html").unwrap();

    let body_online = strfmt!(&base_html,
        content => strfmt!(&online_html,
            name => set_get_name(None),
            kuma_url => kuma_url.to_string()
        )?,
        banner_color => COLOR_GREEN,
        footer => ""
    )?;
    let body_offline = strfmt!(&base_html,
        content => strfmt!(&offline_html,
            name => set_get_name(None),
            kuma_url => kuma_url.to_string(),
            msg => "{{msg}}"
        )?,
        banner_color => COLOR_RED,
        footer => ""
    )?;
    let body = format!("{{% if status contains \"Up\" %}}
{body_online}
{{% else %}}
{body_offline}
{{% endif %}}");
    debug!("Searching if notification already exists");
    let current_notifications = kuma_client.get_notifications().await?;
    for notification in current_notifications {
        if let Some(name) = notification.name {
            if name == format!("{}_mail",personeelsnummer) {
                debug!("Notification for user {personeelsnummer} already exists, ID: {:?}. Not creating new one",notification.id);
                return Ok((notification.id.unwrap_or_default(), false));
            }
        }
    };
    info!("Notification for user {personeelsnummer} does NOT yet exist, creating one");

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
        "smtpSecure": secure,
        "htmlBody": true

    });
    let notification = notification::Notification{
        name: Some(format!("{}_mail",personeelsnummer.to_string())),
        config: Some(config),
        ..Default::default()
    };
    
    let notification_response = kuma_client.add_notification(notification.clone()).await?;
    let id = notification_response.id.unwrap();
    info!("Created notification with ID {id}");
    Ok((id, true))
}

async fn get_monitor_type_id(kuma_client: &Client, group_name: &str, monitor_type: MonitorType, create_new: bool) -> GenResult<Option<i32>> {
    let current_monitors = kuma_client.get_monitors().await?;
    // Check if a group with the same name of "group_name" exists
    for (_id, monitor) in current_monitors.into_iter(){
        if monitor.monitor_type() == monitor_type{
            if monitor.common().name() == &Some(group_name.to_string()) {
                debug!("Existing monitor group has been found, ID: {:?}", monitor.common().id());
                return Ok(Some(monitor.common().id().unwrap_or_default()));
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
        let id = new_monitor.common().id().unwrap_or_default();
        info!(", created new one with id {id}");
        return Ok(Some(new_monitor.common().id().unwrap_or_default()));
    }
    info!(", not creating new one");
    Ok(None)
}