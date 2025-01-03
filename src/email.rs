use std::path::{Path, PathBuf};

use dotenvy::var;
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, Message,
    SmtpTransport, Transport,
};
use thirtyfour::error::WebDriverResult;
use time::{macros::format_description, Date};

use crate::{Shift, Shifts};

type GenResult<T> = Result<T, Box<dyn std::error::Error>>;

pub struct EnvMailVariables {
    smtp_server: String,
    smtp_username: String,
    smtp_password: String,
    mail_from: String,
    mail_to: String,
    mail_error_to: String,
    send_email_new_shift: bool,
    send_mail_updated_shift: bool,
    send_error_mail: bool,
}
const ERROR_VALUE: &str = "Foutje gemaakt oops";

/*
Loads all env variables needed for sending mails
Does not load defaults if they are not found and will just error
*/
impl EnvMailVariables {
    pub fn new() -> GenResult<Self> {
        let smtp_server = var("SMTP_SERVER")?;
        let smtp_username = var("SMTP_USERNAME")?;
        let smtp_password = var("SMTP_PASSWORD")?;
        let mail_from = var("MAIL_FROM")?;
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
*/
pub fn send_emails(current_shifts: &Vec<Shift>) -> GenResult<()> {
    let env = EnvMailVariables::new()?;
    let mailer = load_mailer(&env)?;
    let previous_shifts = match load_previous_shifts() {
        Ok(x) => x,
        _ => {
            println!("loading_shifts_failed");
            return Ok(());
        } //If there is any error loading previous shifts, just do an early return..
    };
    find_send_shift_mails(&mailer, &previous_shifts, current_shifts, &env)?;
    Ok(())
}

// Creates SMTPtransport from username, password and server found in env
fn load_mailer(env: &EnvMailVariables) -> GenResult<SmtpTransport> {
    let creds = Credentials::new(env.smtp_username.clone(), env.smtp_password.clone());
    let mailer = SmtpTransport::relay(&env.smtp_server)?
        .credentials(creds)
        .build();
    Ok(mailer)
}

// Loads shifts from last time this program was run
fn load_previous_shifts() -> GenResult<Vec<Shift>> {
    let path = Path::new("./previous_shifts.toml");
    let shifts_toml = std::fs::read_to_string(path)?;
    let shifts: Shifts = toml::from_str(&shifts_toml)?;
    Ok(shifts.shifts)
}

/*
A really ugly function that often displays weird behaviour
Will search for new shifts given previous shifts.
Will be ran twice, If provided new shifts, it will look for updated shifts instead
Will send an email is send_mail is true
*/
fn find_send_shift_mails(
    mailer: &SmtpTransport,
    previous_shifts: &Vec<Shift>,
    current_shifts: &Vec<Shift>,
    env: &EnvMailVariables,
) -> GenResult<Vec<Shift>> {
    let mut updated_shifts = Vec::new();
    let mut new_shifts_list = Vec::new();
    let current_date: Date = Date::parse(
        &chrono::offset::Local::now().format("%d-%m-%Y").to_string(),
        format_description!("[day]-[month]-[year]"),
    )?;

    // Track shifts by start date
    let previous_by_start: std::collections::HashMap<_, _> = previous_shifts
        .iter()
        .map(|shift| (shift.date, shift))
        .collect();

    // Iterate through the current shifts to check for updates or new shifts
    for current_shift in current_shifts {
        if current_shift.date < current_date {
            continue; // Skip old shifts
        }

        match previous_by_start.get(&current_shift.date) {
            Some(previous_shift) => {
                // If the shift exists, compare its full details for updates
                if current_shift.magic_number != previous_shift.magic_number {
                    updated_shifts.push(current_shift.clone());
                }
            }
            None => {
                // It's a new shift
                new_shifts_list.push(current_shift.clone());
            }
        }
    }

    if !new_shifts_list.is_empty() && env.send_email_new_shift {
        create_send_new_email(mailer, &new_shifts_list, env, false)?;
    }

    if !updated_shifts.is_empty() && env.send_mail_updated_shift {
        create_send_new_email(mailer, &updated_shifts, env, true)?;
    }

    Ok(updated_shifts)
}

/*
Composes and sends mail with either new shifts or updated shifts if required. in plaintext
Depending on if update is true or false
Will always send under the name of Peter
*/
fn create_send_new_email(
    mailer: &SmtpTransport,
    new_shifts: &Vec<Shift>,
    env: &EnvMailVariables,
    update: bool,
) -> GenResult<()> {
    let email_shift_s = if new_shifts.len() != 1 { "en" } else { "" };
    let date_description = format_description!("[day]-[month]-[year]");
    let time_description = format_description!("[hour]:[minute]");
    let name = &new_shifts.first().unwrap().name;
    let new_update_text = match update {
        true => "geupdate",
        false => "nieuwe",
    };
    let mut email_body: String = format!(
        "Hoi {}!\n\nJe hebt {} {} dienst{}: ",
        &name,
        new_update_text,
        &new_shifts.len(),
        email_shift_s
    );
    for shift in new_shifts {
        email_body.push_str(&format!(
            "\n\nDienst {}
Datum: {}
Begintijd: {}
Eindtijd: {}
Duur: {} uur {} minuten",
            shift.number,
            shift.date.format(date_description)?,
            shift.start.format(time_description)?,
            shift.end.format(time_description)?,
            shift.duration.whole_hours(),
            shift.duration.whole_minutes() % 60
        ));
    }
    println!("{}", email_body);
    let email = Message::builder()
        .from(format!("Peter <{}>", &env.mail_from).parse()?)
        .to(format!("{} <{}>", &name, &env.mail_to).parse()?)
        .subject(format!(
            "Je hebt {} {} dienst{}",
            &new_shifts.len(),
            new_update_text,
            email_shift_s
        ))
        .header(ContentType::TEXT_PLAIN)
        .body(email_body)?;
    mailer.send(&email)?;
    Ok(())
}

/*
Composes and sends email of found errors, in plaintext
List of errors can be as long as possible, but for now is always 3
*/
pub fn send_errors(errors: Vec<Box<dyn std::error::Error>>, name: &str) -> GenResult<()> {
    let env = EnvMailVariables::new()?;
    if !env.send_error_mail {
        println!("tried to send error mail, but is disabled");
        return Ok(());
    }
    println!(
        "Er zijn fouten gebeurt, mailtje met fouten wordt gestuurd naar {}",
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
    let env = EnvMailVariables::new()?;
    if !env.send_error_mail {
        println!("tried to send error mail, but is disabled");
        return Ok(());
    }
    let mailer = load_mailer(&env)?;
    let mut email_errors = "!!! KAN NIET VERBINDEN MET GECKO !!!\n".to_string();
    email_errors.push_str(&format!("Error: \n{}\n\n", error.err().unwrap().to_string()));
    let email = Message::builder()
        .from(format!("Foutje Berichtmans <{}>", &env.mail_from).parse()?)
        .to(format!("{} <{}>", "user", &env.mail_error_to).parse()?)
        .subject(&format!("KAN NIET VERBINDEN MET GECKO"))
        .header(ContentType::TEXT_PLAIN)
        .body(email_errors)?;
    mailer.send(&email)?;
    Ok(())
}

pub fn send_welcome_mail(path: &PathBuf, username: &str, name: &str) -> GenResult<()>{
    if path.exists() {return Ok(());}
    let send_welcome_mail = EnvMailVariables::str_to_bool(&var("SEND_WELCOME_MAIL").unwrap_or("false".to_string()));
    println!("welkom mail sturen {send_welcome_mail}");
    if !send_welcome_mail {return Ok(());}
    
    let env = EnvMailVariables::new()?;
    let mailer = load_mailer(&env)?;
    let domain = var("DOMAIN").unwrap_or(ERROR_VALUE.to_string());
    let ical_username = var("ICAL_USER").unwrap_or(ERROR_VALUE.to_string());
    let ical_password = var("ICAL_PASS").unwrap_or(ERROR_VALUE.to_string());
    let ical_url = format!("{}/{}.ics",domain,username);
    let body = format!("Welkom bij Webcom Ical {}!\n\nJe shifts zijn voor het eerst succesvol ingeladen. De link om deze in te laden is: \n{}\nOoit staat hier ook een uitleg om deze link toe te voegen aan je agenda, maar voor nu moet je het zelf uitzoeken :)\n\nInloggegevens website:\nUsername:{}\nPassword{}",name,ical_url,ical_username,ical_password);
    let email = Message::builder()
        .from(format!("Peter <{}>", &env.mail_from).parse()?)
        .to(format!("{} <{}>", name, &env.mail_to).parse()?)
        .subject(&format!("Welkom bij Webcom Ical {}!",name))
        .header(ContentType::TEXT_PLAIN)
        .body(body)?;
    mailer.send(&email)?;
    Ok(())
}
