use std::{fs::{read_to_string, write}, path::PathBuf, time::SystemTime};

use serde::{Deserialize, Serialize};

use crate::{ical::CALENDAR_VERSION, shift::Shift, FailureType, GenResult, BASE_DIRECTORY};

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ApplicationLogbook {
    pub state: FailureType,
    // How long the application has been in the same state
    pub repeat_count: usize,
    pub application_state: ApplicationState
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

    // Populate the logbook values and save it to disk
    pub fn save(&mut self, state: &FailureType) -> GenResult<()> {
        let path = ApplicationLogbook::create_path();
        let execution_time = SystemTime::now().duration_since(self.application_state.system_time.ok_or("Previous system time not set!")?).and_then(|duration| Ok(duration.as_millis() as usize)).unwrap_or_default();
        self.application_state.execution_time_ms = execution_time;
        self.repeat_count = if self.state == *state {self.repeat_count + 1} else {0};
        self.application_state.calendar_version = CALENDAR_VERSION.to_owned();
        self.state = state.clone();
        write(path, serde_json::to_string_pretty(&self)?)?;
        Ok(())
    }
    fn create_path() -> PathBuf {
        let mut path = PathBuf::from(BASE_DIRECTORY);
        path.push("logbook.json");
        path
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ApplicationState {
    #[serde(default, skip)]
    system_time: Option<SystemTime>,
    pub execution_time_ms: usize,
    pub shifts: usize,
    pub broken_shifts: usize,
    pub calendar_version: String,
}