use entity::prelude::{DonationText, EmailProperties, GeneralPropertiesDb, KumaProperties};
use entity::{donation_text, email_properties, general_properties_db, kuma_properties};
use sea_orm::{DerivePartialModel, FromQueryResult};
use thirtyfour::WebDriver;

#[allow(dead_code)]
struct UserData {
    personeelsnummer: String,
    wachtwoord: String,
    email: String,
    filename: String,
    properties: UserProperties,
    custom_general_properties: Option<GeneralProperties>,
}

#[allow(dead_code)]
struct UserProperties {
    send_mail_new_shift: bool,
    send_mail_updated_shift: bool,
    send_mail_removed_shift: bool,
    send_failed_signin_mail: bool,
    send_welcome_mail: bool,
    send_error_mail: bool,
    split_night_shift: bool,
    stop_midnight_shift: bool,
}

#[allow(dead_code)]
#[derive(DerivePartialModel, Debug)]
#[sea_orm(entity = "EmailProperties")]
struct EmailPropertiesA {
    smtp_server: String,
    smtp_username: String,
    smtp_password: String,
    mail_from: String,
}

#[allow(dead_code)]
#[derive(DerivePartialModel, Debug)]
#[sea_orm(entity = "KumaProperties")]
pub struct KumaPropertiesA {
    domain: String,
    hearbeat_retry: i32,
    offline_mail_resend_hours: i32,
    #[sea_orm(nested)]
    kuma_email_properties: email_properties::Model,
    mail_port: i32,
    use_ssl: bool,
}

#[allow(dead_code)]
#[derive(DerivePartialModel, Debug)]
#[sea_orm(entity = "GeneralPropertiesDb")]
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
    kuma_properties: KumaPropertiesA,
    #[sea_orm(nested)]
    general_email_properties: email_properties::Model,
    #[sea_orm(nested)]
    donation_text: DonationTextA,
}

#[allow(dead_code)]
#[derive(DerivePartialModel, Debug)]
#[sea_orm(entity = "DonationText")]
struct DonationTextA {
    donate_link: String,
    donate_text: String,
    donate_service_name: String,
    iban: String,
    iban_name: String,
}

// #[allow(dead_code)]
// #[derive(FromQueryResult, Debug)]
// #[sea_orm(model = "email_properties")]
// struct EmailPropertiesA {
//     #[sea_orm(primary_key)]
//     email_id: i32,
//     smtp_server: String,
//     smtp_username: String,
//     smtp_password: String,
//     mail_from: String,
// }

// #[allow(dead_code)]
// #[derive(FromQueryResult, Debug)]
// #[sea_orm(model = "kuma_properties")]
// pub struct KumaPropertiesA {
//     #[sea_orm(primary_key)]
//     kuma_id: i32,
//     domain: String,
//     hearbeat_retry: i32,
//     offline_mail_resend_hours: i32,
//     #[sea_orm(nested)]
//     kuma_email_properties: EmailPropertiesA,
//     mail_port: i32,
//     use_ssl: bool,
// }

// #[allow(dead_code)]
// #[derive(FromQueryResult, Debug)]
// #[sea_orm(model = "general_properties_db")]
// pub struct GeneralProperties {
//     #[sea_orm(primary_key)]
//     general_properties_id: i32,
//     save_target: String,
//     ical_domain: String,
//     webcal_domain: String,
//     pdf_shift_domain: String,
//     signin_fail_execution_reduce: i32,
//     signin_fail_mail_reduce: i32,
//     execution_interval_minutes: i32,
//     expected_execution_time_seconds: i32,
//     execution_retry_count: i32,
//     support_mail: String,
//     password_reset_link: String,
//     #[sea_orm(nested)]
//     kuma_properties: KumaPropertiesA,
//     #[sea_orm(nested)]
//     general_email_properties: EmailPropertiesA,
//     #[sea_orm(nested)]
//     donation_text: DonationTextA,
// }

// #[allow(dead_code)]
// #[derive(FromQueryResult, Debug)]
// #[sea_orm(model = "donation_text")]
// struct DonationTextA {
//     #[sea_orm(primary_key)]
//     donation_id: i32,
//     donate_link: String,
//     donate_text: String,
//     donate_service_name: String,
//     iban: String,
//     iban_name: String,
// }

// struct WebcomIcalInstance {
//     pub user_data: UserData,
//     pub user_properties: UserProperties,
//     pub general_properties: GeneralPropertiesDb,
//     pub general_mail_properties: EmailProperties,
//     pub kuma_properties: KumaProperties,
//     pub kuma_mail_properties: EmailProperties,
//     pub donation_text: DonationText,
//     pub browser_instance: WebDriver,
// }
