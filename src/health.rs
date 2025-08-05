use serde::{Deserialize, Serialize};

use crate::FailureType;

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Clone)]
struct ApplicationLogbook {
    state: FailureState,
    // How long the application has been in the same state
    occuring_since: usize,
    entries: Vec<LogbookEntry>,
    application_state: ApplicationState,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Clone)]
enum FailureState {
    Failing = 0,
    External,
    NonCritical,
    #[default]
    Ok,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Clone)]
enum FailureStateType {
    Failing(String),
    External(FailureType),
    NonCritical(String),
    #[default]
    Ok,
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
