use dotenvy::var;
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, Message,
    SmtpTransport, Transport,
};
use thiserror::Error;
use std::{
    collections::HashMap, fs, path::PathBuf
};
use strfmt::strfmt;
use thirtyfour::error::{WebDriverErrorInfo, WebDriverResult};
use time::{macros::format_description, Date};
use crate::ShiftState;

use crate::{create_ical_filename, create_shift_link, set_get_name, IncorrectCredentialsCount, Shift, SignInFailure};

type GenResult<T> = Result<T, Box<dyn std::error::Error>>;

const ERROR_VALUE: &str = "HIER HOORT WAT ANDERS DAN DEZE TEKST TE STAAN, CONFIGURATIE INCORRECT";
const SENDER_NAME: &str = "Peter";
const TIME_DESCRIPTION: &[time::format_description::BorrowedFormatItem<'_>] =
    format_description!("[hour]:[minute]");
pub const DATE_DESCRIPTION: &[time::format_description::BorrowedFormatItem<'_>] =
    format_description!("[day]-[month]-[year]");

const COLOR_BLUE: &str = "#1a5fb4";
const COLOR_RED: &str = "#a51d2d";
const COLOR_GREEN: &str = "#26a269";

trait StrikethroughString {
    fn strikethrough(&self) -> String;
}

impl StrikethroughString for String {
    fn strikethrough(&self) -> String {
    self
            .chars()
            .map(|c| format!("{}{}", c, '\u{0336}'))
            .collect()
    }
}

#[derive(Error, Debug, PartialEq)]
pub enum PreviousShiftsError {
    #[error("Parsing of previous shifts has failed. Error: {0}")]
    Generic(String),
    #[error("Previous shifts file IO error. Error: {0}")]
    Io(String)
}

pub struct EnvMailVariables {
    pub smtp_server: String,
    pub smtp_username: String,
    pub smtp_password: String,
    pub mail_from: String,
    pub mail_to: String,
    mail_error_to: String,
    send_email_new_shift: bool,
    send_mail_updated_shift: bool,
    send_error_mail: bool,
}

/*
Loads all env variables needed for sending mails
Does not load defaults if they are not found and will just error
If kuma is true, it adds KUMA_ to the var names to find ones specific for KUMA
*/
impl EnvMailVariables {
    pub fn new(kuma: bool) -> GenResult<Self> {
        let smtp_server = var(format!("{}SMTP_SERVER",if kuma {"KUMA_"} else {""}))?;
        let smtp_username = var(format!("{}SMTP_USERNAME",if kuma {"KUMA_"} else {""}))?;
        let smtp_password = var(format!("{}SMTP_PASSWORD",if kuma {"KUMA_"} else {""}))?;
        let mail_from = var(format!("{}MAIL_FROM",if kuma {"KUMA_"} else {""}))?;
        let mail_to = var("MAIL_TO")?;
        let mail_error_to = var("MAIL_ERROR_TO")?;
        let send_email_new_shift = Self::str_to_bool(&var("SEND_EMAIL_NEW_SHIFT")?);
        let send_mail_updated_shift = Self::str_to_bool(&var("SEND_MAIL_UPDATED_SHIFT")?);
        let send_error_mail = Self::str_to_bool(&var("SEND_ERROR_MAIL")?);
        Ok(Self {
            smtp_server,
            smtp_username,
            smtp_password,
            mail_from,
            mail_to,
            mail_error_to,
            send_email_new_shift,
            send_mail_updated_shift,
            send_error_mail,
        })
    }
    /*
    Simple function to convert "true" to true, and anything else to false
    */
    fn str_to_bool(input: &str) -> bool {
        match input {
            "true" => true,
            _ => false,
        }
    }
}

/*
Main function for sending mails, it will always be called and will individually check if that function needs to be called
If loading previous shifts fails for whatever it will not error but just do an early return.
Because if the previous shifts file is not, it will just not send mails that time
Returns the list of previously known shifts, updated with new shits
*/
pub fn send_emails(current_shifts: &mut Vec<Shift>, previous_shifts: &Vec<Shift>) -> GenResult<Vec<Shift>> {
    let env = EnvMailVariables::new(false)?;
    let mailer = load_mailer(&env)?;
    if previous_shifts.is_empty() {
        // if the previous were empty, just return the list of current shifts as all new
        error!("!!! PREVIOUS SHIFTS WAS EMPTY. SKIPPING !!!");
        return Ok(current_shifts.iter().cloned().map(|mut shift| {shift.state = ShiftState::New; shift}).collect());
    }
    Ok(find_send_shift_mails(&mailer, previous_shifts, current_shifts, &env)?)
    
}

// Creates SMTPtransport from username, password and server found in env
fn load_mailer(env: &EnvMailVariables) -> GenResult<SmtpTransport> {
    let creds = Credentials::new(env.smtp_username.clone(), env.smtp_password.clone());
    let mailer = SmtpTransport::relay(&env.smtp_server)?
        .credentials(creds)
        .build();
    Ok(mailer)
}

/*
Will search for new shifts given previous shifts.
Will be ran twice, If provided new shifts, it will look for updated shifts instead
Will send an email is send_mail is true
It doesn't make a lot of sense that this function is in Email
*/
fn find_send_shift_mails(
    mailer: &SmtpTransport,
    previous_shifts: &Vec<Shift>,
    current_shifts: &mut Vec<Shift>,
    env: &EnvMailVariables,
) -> GenResult<Vec<Shift>> {
    let current_date: Date = Date::parse(
        &chrono::offset::Local::now().format("%d-%m-%Y").to_string(),
        DATE_DESCRIPTION,
    )?;
    let mut current_shifts_map  = previous_shifts.iter().cloned().map(|shift| {(shift.magic_number,shift)}).collect::<HashMap<i64,Shift>>();
    // Iterate through the current shifts to check for updates or new shifts

    // We start with a list of previously valid shifts. All marked as deleted
    // we will then loop over a list of newly loaded shifts from the website
    for current_shift in &mut *current_shifts { 
        // If the hash of this current shift is found in the previously valid shift list,
        // we know this shift has remained unchanged. So mark it as such
        if let Some(previous_shift) = current_shifts_map.get_mut(&current_shift.magic_number) {
            previous_shift.state = ShiftState::Unchanged;
        } else {
            // if it is not found, we loop over the list of previously known shifts
            for previous_shift in current_shifts_map.clone() {
                // if during the loop, we find a previously valid shift with the same starting date as the current shift
                // whereby we assume only 1 shift can be active per day
                // we know it must have changed, as if it hadn't it would have been found from its hash
                // so it can be marked as changed
                // We must first remove the old shift, then add the new shift
                if previous_shift.1.date == current_shift.date {
                    match current_shifts_map.remove(&previous_shift.0) {
                        Some(_) => (),
                        None => warn!("Tried to remove shift {} as it has been updated, but that failed", previous_shift.1.number)
                    };
                    current_shift.state = ShiftState::Changed;
                    current_shifts_map.insert(current_shift.magic_number, current_shift.clone());
                    break;
                }
            }
            // If after that loop, no previously known shift with the same start date as the new shift was found
            // we know it is a new shift, so we mark it as such and add it to the list of known shifts
            // I THINK this currently marks all removed shifts as new, so check that when i have the brain capacity 
            if current_shift.state != ShiftState::Changed {
                current_shift.state = ShiftState::New;
                current_shifts_map.insert(current_shift.magic_number, current_shift.clone());
            }
            
        }

    }
    let current_shift_vec: Vec<Shift> = current_shifts_map.values().cloned().collect();
    let mut new_shifts: Vec<&Shift> = current_shift_vec.iter().filter(|item| {
        item.state == ShiftState::New
    }).collect();
    let mut updated_shifts: Vec<&Shift> = current_shift_vec.iter().filter(|item| {
        item.state == ShiftState::Changed
    }).collect();
    let mut removed_shifts: Vec<&Shift> = current_shift_vec.iter().filter(|item| {
        item.state == ShiftState::Deleted
    }).collect();
    // debug!("shift vec : {:#?}",current_shift_vec);
    debug!("Removed shift vec size: {}", removed_shifts.len());
    new_shifts.retain(|shift| shift.date >= current_date);
    if !new_shifts.is_empty() && env.send_email_new_shift {
        info!("Found {} new shifts, sending email", new_shifts.len());
        create_send_new_email(mailer, new_shifts, env, false)?;
    }
    updated_shifts.retain(|shift| shift.date >= current_date);
    if !updated_shifts.is_empty() && env.send_mail_updated_shift {
        info!("Found {} updated shifts, sending email", updated_shifts.len());
        create_send_new_email(mailer, updated_shifts, env, true)?;
    }
    if !removed_shifts.is_empty() && env.send_mail_updated_shift {
        info!("Removing {} shifts", removed_shifts.len());
        removed_shifts.retain(|shift| shift.date >= current_date);
        if !removed_shifts.is_empty() {
            send_removed_shifts_mail(mailer, env, removed_shifts)?;
        }
        
    }
    // At last remove all shifts marked as removed from the vec
    let current_shift_vec = current_shift_vec.into_iter().filter(|shift| shift.state != ShiftState::Deleted).collect();
    Ok(current_shift_vec)
}

/*
Composes and sends mail with either new shifts or updated shifts if required. in plaintext
Depending on if update is true or false
Will always send under the name of Peter
*/
fn create_send_new_email(
    mailer: &SmtpTransport,
    new_shifts: Vec<&Shift>,
    env: &EnvMailVariables,
    update: bool,
) -> GenResult<()> {
    let base_html = fs::read_to_string("./templates/email_base.html").unwrap();
    let mut changed_mail_html = fs::read_to_string("./templates/changed_shift.html").unwrap();
    let shift_table = fs::read_to_string("./templates/shift_table.html").unwrap();
    let email_shift_s = if new_shifts.len() != 1 { "en" } else { "" };
    let name = set_get_name(None);
    let new_update_text = match update {
        true => "geupdate",
        false => "nieuwe",
    };

    let mut shift_tables = String::new();
    for shift in &new_shifts {
        let shift_table_clone = strfmt!(&shift_table,
            shift_number => shift.number.clone(),
            shift_date => shift.date.format(DATE_DESCRIPTION)?.to_string(),
            shift_start => shift.start.format(TIME_DESCRIPTION)?.to_string(),
            shift_end => shift.end.format(TIME_DESCRIPTION)?.to_string(),
            shift_duration_hour => shift.duration.whole_hours().to_string(),
            shift_duration_minute => (shift.duration.whole_minutes() % 60).to_string(),
            shift_link => create_shift_link(shift, false)?
        )?;
        shift_tables.push_str(&shift_table_clone);
    }
    changed_mail_html = strfmt!(
        &changed_mail_html,
        name => name.clone(),
        shift_changed_ammount => new_shifts.len().to_string(),
        new_update => new_update_text.to_string(),
        single_plural => email_shift_s.to_string(),
        shift_tables => shift_tables.to_string()
    )?;
    let email_body_html = strfmt!(&base_html, 
        content => changed_mail_html,
        banner_color => COLOR_BLUE,
        footer => create_footer(false)
    )?;

    let email = Message::builder()
        .from(format!("Peter <{}>", &env.mail_from).parse()?)
        .to(format!("{} <{}>", &name, &env.mail_to).parse()?)
        .subject(format!(
            "Je hebt {} {} dienst{}",
            &new_shifts.len(),
            new_update_text,
            email_shift_s
        ))
        .header(ContentType::TEXT_HTML)
        .body(email_body_html)?;
    mailer.send(&email)?;
    Ok(())
}

fn create_footer(only_url:bool) -> String {
    let footer_text = r#"<tr>
      <td style="background-color:#FFFFFF; text-align:center; padding-top:0px;font-size:12px;">
        <a style="color:#9a9996;">{footer_text}
      </td>
      <tr>
      <td style="background-color:#FFFFFF; text-align:center;font-size:12px;padding-bottom:10px;">
        <a href="{footer_url}" style="color:#9a9996;">{footer_url}</a>
      </td>
      <tr>
      <td style="background-color:#FFFFFF; text-align:center;font-size:12px;padding-bottom:10px;">
        <a style="color:#9a9996;">{admin_email_comment}</a>
      </td>
      </tr>"#;
    let domain = var("DOMAIN").unwrap_or(ERROR_VALUE.to_string());
    let url = format!("{}/{}", domain, create_ical_filename().unwrap_or(ERROR_VALUE.to_owned()));
    let admin_email = var("MAIL_ERROR_TO").ok();
    match only_url {
        true => url,
        false => strfmt!(footer_text,
            footer_text => "Je agenda link:",
            footer_url => url,
            admin_email_comment => if let Some(email) = admin_email {format!("Vragen of opmerkingen? Neem contact op met {email}")} else {"".to_owned()}).unwrap_or("".to_owned()),
        }
}

fn send_removed_shifts_mail(
    mailer: &SmtpTransport,
    env: &EnvMailVariables,
    removed_shifts: Vec<&Shift>,
) -> GenResult<()> {
    let base_html = fs::read_to_string("./templates/email_base.html").unwrap();
    let removed_shift_html = fs::read_to_string("./templates/removed_shift_base.html").unwrap();
    let shift_table = fs::read_to_string("./templates/shift_table.html").unwrap();
    info!("Sending removed shifts mail");
    let enkelvoud_meervoud = if removed_shifts.len() == 1 {
        "is"
    } else {
        "zijn"
    };
    let email_shift_s = if removed_shifts.len() == 1 { "" } else { "en" };
    let name = set_get_name(None);
    let mut shift_tables = String::new();
    for shift in &removed_shifts {
        let shift_table_clone = strfmt!(&shift_table,
            shift_number => shift.number.clone().strikethrough(),
            shift_date => shift.date.format(DATE_DESCRIPTION)?.to_string().strikethrough(),
            shift_start => shift.start.format(TIME_DESCRIPTION)?.to_string().strikethrough(),
            shift_end => shift.end.format(TIME_DESCRIPTION)?.to_string().strikethrough(),
            shift_duration_hour => shift.duration.whole_hours().to_string().strikethrough(),
            shift_duration_minute => (shift.duration.whole_minutes() % 60).to_string().strikethrough(),
            shift_link => create_shift_link(shift, false)?
        )?;
        shift_tables.push_str(&shift_table_clone);
    }
    let removed_shift_html = strfmt!(&removed_shift_html,
        name => name.clone(),
        shift_changed_ammount => removed_shifts.len().to_string(), 
        single_plural_en => email_shift_s, 
        single_plural => enkelvoud_meervoud,
        shift_tables
    )?;
    let email_body_html = strfmt!(&base_html, 
        content => removed_shift_html,
        banner_color => COLOR_BLUE,
        footer => create_footer(false)
    )?;
    let email = Message::builder()
        .from(format!("{} <{}>",SENDER_NAME, &env.mail_from).parse()?)
        .to(format!("{} <{}>", &name, &env.mail_to).parse()?)
        .subject(&format!(
            "{} dienst{} {} verwijderd",
            removed_shifts.len(),
            email_shift_s,
            enkelvoud_meervoud
        ))
        .header(ContentType::TEXT_HTML)
        .body(email_body_html)?;
    mailer.send(&email)?;
    Ok(())
}

/*
Composes and sends email of found errors, in plaintext
List of errors can be as long as possible, but for now is always 3
*/
pub fn send_errors(errors: &Vec<Box<dyn std::error::Error>>, name: &str) -> GenResult<()> {
    let env = EnvMailVariables::new(false)?;
    if !env.send_error_mail {
        info!("tried to send error mail, but is disabled");
        return Ok(());
    }
    warn!(
        "Er zijn fouten opgetreden, mailtje met fouten wordt gestuurd naar {}",
        &env.mail_error_to
    );
    let mailer = load_mailer(&env)?;
    let mut email_errors = "Er zijn fouten opgetreden tijdens het laden van shifts\n".to_string();
    for error in errors {
        email_errors.push_str(&format!("Error: \n{}\n\n", error.to_string()));
    }
    let email = Message::builder()
        .from(format!("Foutje Berichtmans <{}>", &env.mail_from).parse()?)
        .to(format!("{} <{}>", &name, &env.mail_error_to).parse()?)
        .subject(&format!("Fout bij laden shifts van: {}", name))
        .header(ContentType::TEXT_PLAIN)
        .body(email_errors)?;
    mailer.send(&email)?;
    Ok(())
}

pub fn send_gecko_error_mail<T: std::fmt::Debug>(error: WebDriverResult<T>) -> GenResult<()> {
    let env = EnvMailVariables::new(false)?;
    if !env.send_error_mail {
        info!("tried to send GECKO error mail, but is disabled");
        return Ok(());
    }
    let mailer = load_mailer(&env)?;
    let mut email_errors = "!!! KAN NIET VERBINDEN MET GECKO !!!\n".to_string();
    email_errors.push_str(&format!(
        "Error: \n{}\n\n",
        error.err().unwrap_or(thirtyfour::error::WebDriverError::UnknownError(WebDriverErrorInfo::new("Unknown".to_owned()))).to_string()
    ));
    let email = Message::builder()
        .from(format!("Foutje Berichtmans <{}>", &env.mail_from).parse()?)
        .to(format!("{} <{}>", "user", &env.mail_error_to).parse()?)
        .subject(&format!("KAN NIET VERBINDEN MET GECKO"))
        .header(ContentType::TEXT_PLAIN)
        .body(email_errors)?;
    mailer.send(&email)?;
    Ok(())
}

pub fn send_welcome_mail(
    path: &PathBuf,
) -> GenResult<()> {
    if path.exists() {
        return Ok(());
    }
    let send_welcome_mail =
        EnvMailVariables::str_to_bool(&var("SEND_WELCOME_MAIL").unwrap_or("false".to_string()));

    if !send_welcome_mail {
        debug!("{:?}",var("SEND_WELCOME_MAIL"));
        info!("Wanted to send welcome mail. But it is disabled");
        return Ok(());
    }

    let base_html = fs::read_to_string("./templates/email_base.html").unwrap();
    let onboarding_html = fs::read_to_string("./templates/onboarding_base.html").unwrap();
    let auth_html = fs::read_to_string("./templates/onboarding_auth.html").unwrap();

    let env = EnvMailVariables::new(false)?;
    let mailer = load_mailer(&env)?;
    let ical_username = var("ICAL_USER").unwrap_or("".to_owned());
    let ical_password = var("ICAL_PASS").unwrap_or(ERROR_VALUE.to_string());

    let name = set_get_name(None);

    let auth_html = strfmt!(&auth_html, 
        auth_username => ical_username.clone(), 
        auth_password => ical_password.clone(), 
        admin_email => env.mail_error_to.clone())?;

    let agenda_url = create_footer(true);
    let agenda_url_webcal = agenda_url.clone().replace("https", "webcals");
    let kuma_info = if let Ok(kuma_url) = var("KUMA-URL") {
        format!("Als Webcom Ical een storing heeft ontvang je meestal een mail van <em>{}</em> (deze kan in je spam belanden!), op <a href=\"{kuma_url}\" style=\"color:#d97706;text-decoration:none;\">{kuma_url}</a> kan je de actuele status van Webcom Ical bekijken.",var("KUMA_MAIL_FROM").unwrap_or(ERROR_VALUE.to_owned()))
    } else {
        "".to_owned()
    };
    let donation_text = var("DONATION_TEXT").unwrap_or(ERROR_VALUE.to_owned());
    let donation_service = var("DONATION_SERVICE").unwrap_or(ERROR_VALUE.to_owned());
    let donation_link = var("DONATION_LINK").unwrap();
    let iban = var("IBAN").unwrap_or(ERROR_VALUE.to_owned());
    let iban_name = var("IBAN_NAME").unwrap_or(ERROR_VALUE.to_owned());
    let onboarding_html = strfmt!(&onboarding_html, 
        name => name.clone(),
        agenda_url,
        agenda_url_webcal,
        kuma_info,
        donation_service,
        donation_text,
        donation_link,
        iban,
        iban_name,
        auth_credentials => if ical_username.is_empty() {"".to_owned()} else {auth_html}
    )?;
    let email_body_html = strfmt!(&base_html,
        content => onboarding_html,
        banner_color => COLOR_BLUE,
        footer => "".to_owned()
    )?;
    warn!("welkom mail sturen");
    let email = Message::builder()
        .from(format!("{} <{}>",SENDER_NAME, &env.mail_from).parse()?)
        .to(format!("{} <{}>", name, &env.mail_to).parse()?)
        .subject(format!("Welkom bij Webcom Ical {}!", &name))
        .header(ContentType::TEXT_HTML)
        .body(email_body_html)?;
    mailer.send(&email)?;
    Ok(())
}

pub fn send_failed_signin_mail(
    name: &str,
    error: &IncorrectCredentialsCount,
    first_time: bool,
) -> GenResult<()> {
    let send_failed_sign_in = EnvMailVariables::str_to_bool(
        &var("SEND_MAIL_SIGNIN_FAILED").unwrap_or("true".to_string()),
    );
    if !send_failed_sign_in {
        return Ok(());
    }

    let base_html = fs::read_to_string("./templates/email_base.html").unwrap();
    let login_failure_html = fs::read_to_string("./templates/failed_signin.html").unwrap();

    info!("Sending failed sign in mail");
    let env = EnvMailVariables::new(false)?;
    let mailer = load_mailer(&env)?;
    let still_not_working_modifier = if first_time { "" } else { "nog steeds " };

    let verbose_error = match &error.error {
        None => "Een onbekende fout...",
        Some(SignInFailure::IncorrectCredentials) => {
            "Incorrecte inloggegevens, heb je misschien je wachtwoord veranderd?"
        }
        Some(SignInFailure::TooManyTries) => "Te veel incorrecte inlogpogingen…",
        Some(SignInFailure::WebcomDown) => "Webcom heeft op dit moment een storing",
        Some(SignInFailure::Other(fault)) => fault,
    };

    let login_failure_html = strfmt!(&login_failure_html, 
        still_not_working_modifier,
        retry_counter => error.retry_count,
        signin_error => verbose_error.to_string(),
        admin_email => env.mail_error_to.clone()
    )?;
    let email_body_html = strfmt!(&base_html, 
        content => login_failure_html,
        banner_color => COLOR_RED,
        footer => create_footer(false)
    )?;

    let email = Message::builder()
        .from(format!("WEBCOM ICAL <{}>", &env.mail_from).parse()?)
        .to(format!("{} <{}>", name, &env.mail_to).parse()?)
        .subject("INLOGGEN WEBCOM NIET GELUKT!")
        .header(ContentType::TEXT_HTML)
        .body(email_body_html)?;
    mailer.send(&email)?;
    Ok(())
}

pub fn send_sign_in_succesful(name: &str) -> GenResult<()> {
    let send_failed_sign_in = EnvMailVariables::str_to_bool(
        &var("SEND_MAIL_SIGNIN_FAILED").unwrap_or("true".to_string()),
    );
    if !send_failed_sign_in {
        return Ok(());
    }

    let base_html = fs::read_to_string("./templates/email_base.html").unwrap();
    let login_success_html = fs::read_to_string("./templates/signin_succesful.html").unwrap();

    info!("Sending succesful sign in mail");
    let env = EnvMailVariables::new(false)?;
    let mailer = load_mailer(&env)?;
    let email_body_html = strfmt!(&base_html, 
        content => login_success_html,
        banner_color => COLOR_GREEN,
        footer => create_footer(false)
    )?;
    
    let email = Message::builder()
        .from(format!("WEBCOM ICAL <{}>", &env.mail_from).parse()?)
        .to(format!("{} <{}>", name, &env.mail_to).parse()?)
        .subject("Webcom Ical kan weer inloggen!")
        .header(ContentType::TEXT_HTML)
        .body(email_body_html)?;
    mailer.send(&email)?;
    Ok(())
}
