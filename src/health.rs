use std::{fs::{read_to_string, write}, path::PathBuf, time::SystemTime};

use serde::{Deserialize, Serialize};

use crate::FailureType;

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ApplicationLogbook {
    pub state: FailureType,
    // How long the application has been in the same state
    occuring_since: usize,
    entries: Vec<LogbookEntry>,
    application_state: ApplicationState,
}

impl ApplicationLogbook {
    // Load the previous logbook, or create a new one if that fails
    pub fn load() -> ApplicationLogbook {
        let path = ApplicationLogbook::create_path();
        // This match statement tries to load the previous logbook, otherwise it creates a new one
        let mut logbook = match ||->GenResult<Self>{
            let logbook_string = read_to_string(path)?;
            Ok(serde_json::from_str(&logbook_string)?)
        }() {
            Ok(logbook) => logbook,
            Err(err) => {
                info!("Loading previous logbook failed, err: {}. Creating new one", err.to_string());
                ApplicationLogbook::default()}
        };
        let current_system_time = SystemTime::now();
        logbook.application_state.system_time = Some(current_system_time);
        logbook
    }

    pub fn generate_shift_statistics(&mut self, shifts: &Vec<Shift>) {
        let number_of_shifts = shifts.len();
        let number_of_broken_shifts = shifts.iter().filter(|shift| shift.is_broken).count();
        self.application_state.broken_shifts =  number_of_broken_shifts;
        self.application_state.shifts = number_of_shifts;
    }

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Clone)]
struct LogbookEntry {
    state: FailureStateType,
    location: Option<String>,
    additional_info: Option<String>,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Clone)]
struct ApplicationState {
    execution_time_ms: usize,
    shifts: usize,
    broken_shifts: usize,
    calendar_version: String,
}
