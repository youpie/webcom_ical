use std::path::Path;

use dotenvy::var;
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, Message,
    SmtpTransport, Transport,
};
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
    fn str_to_bool(input: &str) -> bool {
        match input {
            "true" => true,
            _ => false,
        }
    }
}

pub fn send_emails(current_shifts: &Vec<Shift>) -> GenResult<()> {
    let env = EnvMailVariables::new()?;
    let creds = Credentials::new(env.smtp_username.clone(), env.smtp_password.clone());
    let mailer = SmtpTransport::relay(&env.smtp_server)?
        .credentials(creds)
        .build();
    let previous_shifts = match load_previous_shifts() {
        Ok(x) => x,
        _ => {
            println!("loading_shifts_failed");
            return Ok(());
        } //If there is any error loading previous shifts, just do an early return..
    };
    println!("Searching new shifts");
    let new_shifts = find_send_shift_mails(
        &mailer,
        &previous_shifts,
        current_shifts,
        None,
        &env,
        env.send_email_new_shift,
    )?;
    println!("Searching updated shifts");
    let _updated_shifts = find_send_shift_mails(
        &mailer,
        &previous_shifts,
        current_shifts,
        Some(&new_shifts),
        &env,
        env.send_mail_updated_shift,
    )?;
    Ok(())
}

fn load_previous_shifts() -> GenResult<Vec<Shift>> {
    let path = Path::new("./previous_shifts.toml");
    let shifts_toml = std::fs::read_to_string(path)?;
    let shifts: Shifts = toml::from_str(&shifts_toml)?;
    Ok(shifts.shifts)
}

fn find_send_shift_mails(
    mailer: &SmtpTransport,
    previous_shifts: &Vec<Shift>,
    current_shifts: &Vec<Shift>,
    new_shifts: Option<&Vec<Shift>>,
    env: &EnvMailVariables,
    send_mail: bool,
) -> GenResult<Vec<Shift>> {
    let mut updated_shifts: Vec<Shift> = current_shifts.clone(); //First add all new shifts, and remove all not new ones
    let mut index_to_be_removed: Vec<usize> = vec![];
    let current_date: Date = Date::parse(
        &chrono::offset::Local::now().format("%d-%m-%Y").to_string(),
        format_description!("[day]-[month]-[year]"),
    )?;
    if new_shifts.is_some() {
        for new_shift in new_shifts.unwrap() {
            for current_shift in current_shifts {
                if new_shift.date == current_shift.date {
                    let index = current_shifts
                        .iter()
                        .position(|r| r.magic_number == current_shift.magic_number)
                        .unwrap();
                    index_to_be_removed.push(index);
                }
            }
        }
    }
    //println!("length shifts {:?}", new_shifts.len());
    for previous_shift in previous_shifts {
        for current_shift in current_shifts {
            if (previous_shift.magic_number == current_shift.magic_number
                || current_shift.date < current_date)
                && new_shifts.is_some()
            {
                let index = current_shifts
                    .iter()
                    .position(|r| r.magic_number == current_shift.magic_number)
                    .unwrap();
                index_to_be_removed.push(index);
            }
            if (previous_shift.date == current_shift.date || current_shift.date < current_date)
                && new_shifts.is_none()
            {
                let index = current_shifts
                    .iter()
                    .position(|r| r.magic_number == current_shift.magic_number)
                    .unwrap();
                index_to_be_removed.push(index);
            }
        }
    }
    index_to_be_removed.sort();
    index_to_be_removed.reverse();
    index_to_be_removed.dedup();
    //println!("shifts to be removed: {:?}", index_to_be_removed);
    index_to_be_removed
        .iter()
        .map(|index| updated_shifts.remove(*index))
        .count();
    //println!("Updated shifts: {:?}", &new_shifts);
    if !updated_shifts.is_empty() && send_mail {
        //println!("New shifts found, sending mail");
        create_send_new_email(mailer, &updated_shifts, env, new_shifts.is_some())?;
    }
    Ok(updated_shifts)
}

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
