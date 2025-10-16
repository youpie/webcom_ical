use arc_swap::ArcSwap;
use entity::{
    donation_text, email_properties, general_properties_db, kuma_properties, user_data,
    user_properties,
};
use sea_orm::DerivePartialModel;
use thirtyfour::WebDriver;

pub struct UserInstance {
    pub data: ArcSwap<UserData>,
    pub general_settings: ArcSwap<GeneralProperties>,
    pub driver: WebDriver,
}

#[allow(dead_code)]
#[derive(DerivePartialModel, Debug)]
#[sea_orm(entity = "general_properties_db::Entity")]
pub struct GeneralProperties {
    save_target: String,
    ical_domain: String,
    webcal_domain: String,
    pdf_shift_domain: String,
    signin_fail_execution_reduce: i32,
    signin_fail_mail_reduce: i32,
    execution_interval_minutes: i32,
    expected_execution_time_seconds: i32,
    execution_retry_count: i32,
    support_mail: String,
    password_reset_link: String,
    #[sea_orm(nested)]
    kuma_properties: KumaProperties,
    #[sea_orm(nested, alias = "general_email")]
    general_email_properties: EmailProperties,
    #[sea_orm(nested)]
    donation_text: DonationText,
}

#[allow(dead_code)]
#[derive(DerivePartialModel, Debug)]
#[sea_orm(entity = "email_properties::Entity")]
struct EmailProperties {
    smtp_server: String,
    smtp_username: String,
    smtp_password: String,
    mail_from: String,
}

#[allow(dead_code)]
#[derive(DerivePartialModel, Debug)]
#[sea_orm(entity = "kuma_properties::Entity")]
pub struct KumaProperties {
    domain: String,
    hearbeat_retry: i32,
    offline_mail_resend_hours: i32,
    #[sea_orm(from_col = "kuma_email_properties")]
    email_id: i32,
    #[sea_orm(nested, alias = "kuma_email")]
    kuma_email_properties: email_properties::Model,
    mail_port: i32,
    use_ssl: bool,
}

#[allow(dead_code)]
#[derive(DerivePartialModel, Debug)]
#[sea_orm(entity = "donation_text::Entity")]
struct DonationText {
    donate_link: String,
    donate_text: String,
    donate_service_name: String,
    iban: String,
    iban_name: String,
}

#[allow(dead_code)]
#[derive(DerivePartialModel, Debug)]
#[sea_orm(entity = "user_data::Entity")]
struct UserData {
    personeelsnummer: String,
    password: String,
    email: String,
    file_name: String,
    #[sea_orm(nested)]
    user_properties: user_properties::Model,
}
