use sea_orm::FromQueryResult;

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

#[derive(FromQueryResult, Debug)]
struct EmailProperties {
    smtp_server: String,
    smtp_username: String,
    smtp_password: String,
    mail_from: String,
    support_mail: String,
}

#[derive(FromQueryResult, Debug)]
struct KumaProperties {
    domain: String,
    smtp_server: String,
    smtp_username: String,
    smtp_password: String,
    mail_from: String,
    mail_port: u32,
    use_ssl: bool,
    hearbeat_retry: u32,
    offline_mail_resend_hours: u32
}

#[derive(FromQueryResult, Debug)]
pub struct GeneralProperties {
    save_target: String,
    ical_domain: String,
    webcal_domain: String,
    pdf_shift_domain: String,
    signin_fail_execution_reduce: u32,
    signin_fail_mail_reduce: u32,
    execution_interval_minutes: u32,
    expected_execution_time_seconds: u32,
    execution_retry_count: u32,
    #[sea_orm(nested)]
    email_properties: Option<EmailProperties>,
    #[sea_orm(nested)]
    kuma_properties: Option<KumaProperties>,
    #[sea_orm(nested)]
    donation_texts: Option<DonationText>
}

#[derive(FromQueryResult, Debug)]
struct DonationText {
    donate_link: String,
    donate_text: String,
    donate_service_name: String,
    iban: String,
    iban_name: String
}