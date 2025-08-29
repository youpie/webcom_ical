use std::{fmt::Display, fs::write, hash::{DefaultHasher, Hash, Hasher}, path::PathBuf};

use dotenvy::var;
use serde::{Deserialize, Serialize};
use thirtyfour::{By, WebDriver};
use thiserror::Error;

use crate::{create_path, email, GenResult};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Error)]
pub enum SignInFailure {
    #[error("Er zijn te veel incorrecte inlogpogingen in een korte periode gedaan")]
    TooManyTries,
    #[error("Inloggegevens kloppen niet")]
    IncorrectCredentials,
    #[error("Webcomm heeft een storing")]
    WebcomDown,
    #[error("Onbekende fout: {0}")]
    Other(String),
}

#[derive(Debug, Error, PartialEq, Clone, Serialize, Deserialize, Default)]
pub enum FailureType {
    #[error("Webcom ical was niet in staat na meerdere pogingen diensten correct in te laden")]
    TriesExceeded,
    #[error("Webcom ical kan geen verbinding maken met de interne browser")]
    GeckoEngine,
    #[error("Webcom ical kon niet inloggen. Fout: {}",0.to_string())]
    SignInFailed(SignInFailure),
    #[error("Webcom ical kon geen verbinding maken met de Webcomm site")]
    ConnectError,
    #[error("Een niet-specifieke fout is opgetreden: {0}")]
    Other(String),
    #[error("Ok")]
    #[default]
    OK,
}

pub trait OptionResult<T> {
    fn result(self) -> GenResult<T>;
}

impl<T> OptionResult<T> for Option<T> {
    fn result(self) -> GenResult<T> {
        match self {
            Some(value) => Ok(value),
            None => Err("Option Unwrap".into()),
        }
    }
}

pub async fn check_sign_in_error(driver: &WebDriver) -> GenResult<FailureType> {
    error!("Sign in failed");
    match driver.find(By::Id("ctl00_lblMessage")).await {
        Ok(element) => {
            let element_text = element.text().await?;
            let sign_in_error_type = get_sign_in_error_type(&element_text);
            info!("Found error banner: {:?}", &sign_in_error_type);
            return Ok(FailureType::SignInFailed(sign_in_error_type));
        }
        Err(_) => {
            info!("Geen fout banner gevonden");
        }
    };
    Ok(FailureType::SignInFailed(SignInFailure::Other(
        "Geen idee waarom er niet ingelogd kon worden".to_string(),
    )))
}

// See if there is a text which indicated webcom is offline
pub fn check_if_webcom_unavailable(h3_text: Option<String>) -> bool {
    match h3_text {
        Some(text) => {
            if text == "De servertoepassing is niet beschikbaar.".to_owned() {
                return true;
            }
        }
        None => (),
    };
    false
}

fn get_sign_in_error_type(text: &str) -> SignInFailure {
    match text {
        "Uw aanmelding was niet succesvol. Voer a.u.b. het personeelsnummer of 'naam, voornaam' in" => {
            SignInFailure::IncorrectCredentials
        }
        "Te veel verkeerde aanmeldpogingen" => SignInFailure::TooManyTries,
        _ => SignInFailure::Other(text.to_string()),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct IncorrectCredentialsCount {
    pub retry_count: usize,
    pub error: Option<SignInFailure>,
    pub previous_password_hash: Option<u64>,
}

impl IncorrectCredentialsCount {
    fn new() -> Self {
        Self {
            retry_count: 0,
            error: None,
            previous_password_hash: None,
        }
    }
}

// Loads the sign in failure counter. Creates it if it does not exist
fn load_sign_in_failure_count(path: &PathBuf) -> GenResult<IncorrectCredentialsCount> {
    let failure_count_json = std::fs::read_to_string(path)?;
    let failure_counter: IncorrectCredentialsCount = serde_json::from_str(&failure_count_json)?;
    Ok(failure_counter)
}

// Save the sign in failure count file
fn save_sign_in_failure_count(
    path: &PathBuf,
    counter: &IncorrectCredentialsCount,
) -> GenResult<()> {
    let failure_counter_serialised = serde_json::to_string(counter)?;
    write(path, failure_counter_serialised.as_bytes())?;
    Ok(())
}

fn get_password_hash() -> GenResult<u64> {
    let current_password = var("PASSWORD")?;
    let mut hasher = DefaultHasher::new();
    current_password.hash(&mut hasher);
    Ok(hasher.finish())
}

pub fn update_signin_failure(failed: bool, failure_type: Option<SignInFailure>) -> GenResult<()> {
    let path = create_path("sign_in_failure_count.json");
    let mut failure_counter = match load_sign_in_failure_count(&path) {
        Ok(failure) => failure,
        Err(_) => IncorrectCredentialsCount::default(),
    };
    if let Ok(current_password_hash) = get_password_hash() {
        debug!("Got current password hash: {current_password_hash}");
        failure_counter.previous_password_hash = Some(current_password_hash);
    }
    // if failed == true, set increment counter and set error
    if failed == true {
        failure_counter.error = failure_type;
        if failure_counter.retry_count == 0 {
            failure_counter.retry_count += 1;
            email::send_failed_signin_mail(&failure_counter, true)?;
        }
    }
    // if failed == false, reset counter
    else if failed == false {
        if failure_counter.error.is_some() {
            info!("Sign in succesful again!");
            email::send_sign_in_succesful()?;
        }
        failure_counter.retry_count = 0;
        failure_counter.error = None;
    }
    save_sign_in_failure_count(&path, &failure_counter)?;
    Ok(())
}

// If returning None, continue execution
pub fn sign_in_failed_check() -> GenResult<Option<SignInFailure>> {
    let resend_error_mail_count: usize = var("SIGNIN_FAIL_MAIL_REPEAT")
        .unwrap_or("24".to_string())
        .parse()
        .unwrap_or(24);
    // let sign_in_attempt_reduce: usize = var("SIGNIN_FAILED_REDUCE")
    //     .unwrap_or("12".to_string())
    //     .parse()
    //     .unwrap_or(12);
    let path = create_path("sign_in_failure_count.json");
    // Load the existing failure counter, create a new one if one doesn't exist yet
    let mut failure_counter = match load_sign_in_failure_count(&path) {
        Ok(value) => value,
        Err(_) => {
            let new = IncorrectCredentialsCount::new();
            save_sign_in_failure_count(&path, &new)?;
            new
        }
    };
    let return_value: Option<SignInFailure>;
    if let Some(previous_password_hash) = failure_counter.previous_password_hash {
        if let Ok(current_password_hash) = get_password_hash() {
            if previous_password_hash != current_password_hash {
                info!("Password hash has changed, resuming execution");
                return Ok(None);
            }
        }
    }
    warn!("Skipped execution due to previous sign in error");
    failure_counter.retry_count += 1;
    return_value = Some(failure_counter.error.clone().result()?);
    // // else check if retry counter == reduce_ammount, if not, stop running
    // if failure_counter.retry_count == 0 {
    //     return_value = None;
    // } else if failure_counter.retry_count % sign_in_attempt_reduce == 0 {
    //     warn!(
    //         "Continuing execution with sign in error, reduce val: {sign_in_attempt_reduce}, current count {}",
    //         failure_counter.retry_count
    //     );
    //     failure_counter.retry_count += 1;
    //     return_value = None;
    // } else {
    //     warn!("Skipped execution due to previous sign in error");
    //     failure_counter.retry_count += 1;
    //     return_value = Some(failure_counter.error.clone().result()?);
    // }

    if failure_counter.retry_count % resend_error_mail_count == 0 && failure_counter.error.is_some()
    {
        email::send_failed_signin_mail(&failure_counter, false)?;
    }
    save_sign_in_failure_count(&path, &failure_counter)?;
    Ok(return_value)
}

pub trait ResultLog<T,E> {
    fn warn(&self, function_name: &str);
    fn warn_owned(self, function_name: &str) -> Self;
    fn info(&self, function_name: &str);
}

impl<T,E> ResultLog<T,E> for Result<T,E> where E: Display{
    fn info(&self, function_name: &str) {
        match self {
            Err(err) => {info!("Error in function \"{function_name}\": {}",err.to_string())},
            _ => ()
        }
    }
    fn warn_owned(self, function_name: &str) -> Self {
        self.inspect_err(|err| {
            warn!("Error in function \"{function_name}\": {}",err.to_string())
        })
    }
    fn warn(&self, function_name: &str) {
        match self {
            Err(err) => {warn!("Error in function \"{function_name}\": {}",err.to_string())},
            _ => ()
        }
    }
    
}