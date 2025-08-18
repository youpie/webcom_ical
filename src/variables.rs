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

struct EmailProperties {
    smtp_server: String,
    smtp_username: String,
    smtp_password: String,
    mail_from: String,
    support_mail: String,
}

struct KumaProperties {
    domain: String,
    smtp_server: String,
    smtp_username: String,
    smtp_password: String,
    mail_from: String,
    mail_port: usize,
    use_ssl: bool,
    hearbeat_retry: usize,
    offline_mail_resend_hours: usize
}

struct GeneralProperties {
    save_target: String,
    ical_domain: String,
    webcal_domain: String,
    pdf_shift_domain: String,
    signin_fail_execution_reduce: usize,
    signin_fail_mail_reduce: usize,
    execution_interval_minutes: usize,
    expected_execution_time_seconds: usize,
    execution_retry_count: usize,
    email_properties: EmailProperties,
    kuma_properties: KumaProperties,
    donation_texts: DonationText
}

struct DonationText {
    donate_link: String,
    donate_text: String,
    donate_service_name: String,
    iban: String,
    iban_name: String
}