use entity::prelude::{DonationText, EmailProperties, GeneralPropertiesDb, KumaProperties};
use entity::{donation_text, email_properties, general_properties_db, kuma_properties};
use sea_orm::{DerivePartialModel, FromQueryResult};
use thirtyfour::WebDriver;

// If you are reading this i'll try to explain it
// The Foreign keys to the email properties are created in migration/src/m20251006_141509_kuma.rs
// And migration/src/m20251006_143409_general_settings.rs

// I understand this might be an unconventional approach, but I really like this workflow so if I could get this working I would be really happy
// Thank you for taking your time looking at my issue btw!
// The debug print of this is printed below the struct

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
    kuma_properties: KumaPropertiesA, // The general properties contains a kuma_properties nested relation
    #[sea_orm(from_col = "general_email_properties")]
    email_id: i32,
    #[sea_orm(nested)]
    general_email_properties: email_properties::Model, // Both the general properties and kuma properties have a email properties relation, but to a different ID of email properties
    #[sea_orm(nested)]
    donation_text: DonationTextA,
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
    #[sea_orm(from_col = "kuma_email_properties")]
    email_id: i32,
    #[sea_orm(nested)]
    kuma_email_properties: email_properties::Model, // The kuma needs to send mails from a different SMTP server
    mail_port: i32,
    use_ssl: bool,
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

/*
GeneralProperties {
    save_target: "./calendar/",
    ical_domain: "calendar.bussie.app",
    webcal_domain: "webcal.bussie.app",
    pdf_shift_domain: "shift.bussie.app",
    signin_fail_execution_reduce: 2,
    signin_fail_mail_reduce: 4,
    execution_interval_minutes: 3600,
    expected_execution_time_seconds: 20,
    execution_retry_count: 8,
    support_mail: "support@bussie.app",
    password_reset_link: "link.bussie.app",
    kuma_properties: KumaPropertiesA {
        domain: "stats.emphisia.nl",
        hearbeat_retry: 1,
        offline_mail_resend_hours: 1,
        email_id: 2,
        kuma_email_properties: Model {
            email_id: 1,
            mail_from: "youplamb@hotmail.nl",
            smtp_server: "mail.emphisia.nl",
            smtp_username: "nextcloud@emphisia.nl",
            smtp_password: "123qwerty",
        },
        mail_port: 465,
        use_ssl: true,
    },
    email_id: 1,
    general_email_properties: Model {
        email_id: 1,
        mail_from: "youplamb@hotmail.nl",
        smtp_server: "mail.emphisia.nl",
        smtp_username: "nextcloud@emphisia.nl",
        smtp_password: "123qwerty",
    },
    donation_text: DonationTextA {
        donate_link: "google.com",
        donate_text: "Thanx",
        donate_service_name: "google",
        iban: "NL1234",
        iban_name: "Youp",
    },
}
 */

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
