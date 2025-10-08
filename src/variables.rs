use entity::{donation_text, email_properties, kuma_properties, general_properties_db};
use entity::prelude::GeneralPropertiesDb;
use sea_orm::DerivePartialModel;

struct UserData {
    personeelsnummer: String,
    wachtwoord: String,
    email: String,
    filename: String,
    properties: UserProperties,
    custom_general_properties: Option<GeneralProperties>
}

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

struct EmailPropertiesA {
    smtp_server: String,
    smtp_username: String,
    smtp_password: String,
    mail_from: String,
}

pub struct KumaPropertiesA {
    domain: String,
    hearbeat_retry: i32,
    offline_mail_resend_hours: i32,
    email_properties: EmailPropertiesA,
    mail_port: i32,
    use_ssl: bool,
}

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
    expected_exectution_time_seconds: i32,
    execution_retry_count: i32,
    support_mail: String,
    password_reset_link: String,
    #[sea_orm(nested)]
    pub kuma_properties: kuma_properties::Model,
    #[sea_orm(nested)]
    email_properties: email_properties::Model,
    #[sea_orm(nested)]
    donation_text: donation_text::Model,

}

struct DonationTextA {
    donate_link: String,
    donate_text: String,
    donate_service_name: String,
    iban: String,
    iban_name: String
}