use std::{
    collections::HashMap,
    fs::{self, read_to_string, write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::{
    FailureType, GenResult, Shift, ShiftState, create_ical_filename, create_shift_link,
    set_get_name,
};
use crate::{email::TIME_DESCRIPTION, errors::ResultLog};
use chrono::{Datelike, Local, Months, NaiveDate, NaiveDateTime, NaiveTime};
use dotenvy::var;
use icalendar::{
    Calendar, CalendarComponent, CalendarDateTime, Component, Event, EventLike,
    parser::{read_calendar, unfold},
};
use serde_json::from_str;
use thiserror::Error;
use time::{Date, OffsetDateTime, Time};

trait ToNaive {
    fn to_naive(&self) -> Option<NaiveDate>;
}

impl ToNaive for time::Date {
    fn to_naive(&self) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(self.year() as i32, self.month() as u32, self.day() as u32)
    }
}

// UPDATE THIS WHENEVER ANYTHING CHANGES IN THE ICAL
// Add B if it modifies of removes an already existing value
// Add W if it is wanted to resend the welcome mail
pub const CALENDAR_VERSION: &str = "3B";

const PREVIOUS_EXECUTION_DATE_PATH: &str = "./kuma/previous_execution_date";
pub const NON_RELEVANT_EVENTS_PATH: &str = "./kuma/non_relevant_events";
pub const RELEVANT_EVENTS_PATH: &str = "./kuma/relevant_events";

#[derive(Debug, Error, Clone, PartialEq)]
enum CalendarVersionError {
    #[error("Calendar version changed with a breaking change")]
    BreakingChange,
    #[error("Calendar version has changed, and welcome mail is requested")]
    WelcomeChange,
}

pub fn load_ical_file(path: &Path) -> GenResult<Calendar> {
    let calendar_string = read_to_string(path)?;
    let calendar: Calendar = read_calendar(&unfold(&calendar_string))?.into();
    // Check if the calendar has changed, and if that change was breaking
    match calendar.property_value("X-CAL-VERSION").unwrap_or_default() {
        version if version != CALENDAR_VERSION => {
            warn!("Calendar version has changed!");
            if let Some(version_type) = CALENDAR_VERSION.chars().last() {
                match version_type {
                    'B' => {
                        warn!("Breaking change");
                        return Err(Box::new(CalendarVersionError::BreakingChange));
                    }
                    'W' => {
                        warn!("Welcome change");
                        return Err(Box::new(CalendarVersionError::WelcomeChange));
                    }
                    _ => {
                        info!("Non beaking change");
                    }
                }
            }
        }
        _ => (),
    };
    Ok(calendar)
}

pub fn get_calendar_events(calendar: Calendar) -> Vec<Event> {
    let mut events = vec![];
    let components = calendar.components;
    for component in components {
        if let CalendarComponent::Event(shift_event) = component {
            events.push(shift_event)
        }
    }
    events
}

// Returns two vecs of events, one of shifts more than one month, one of less than that
// Only the shifts less than a month ago are actually used
// 1st element is relevant, second element is non-relevant.
pub fn split_relevant_shifts(shifts: Vec<Shift>) -> (Vec<Shift>, Vec<Shift>) {
    // The date is how many days have elapsed since 1-1-2025. Assuming 31 days per month
    let today = Local::now().date_naive();
    let first_of_this_month = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();

    // Subtract one month, with proper handling for end-of-month behavior
    let cutoff = first_of_this_month
        .checked_sub_months(Months::new(1))
        .unwrap();

    let mut non_relevant_events = vec![];
    let mut relevant_events = vec![];
    for shift in shifts {
        // If event date is unknown. Just add it to the non relevant events
        let event_date = if let Some(date) = shift.date.to_naive() {
            date
        } else {
            non_relevant_events.push(shift);
            continue;
        };
        if event_date >= cutoff {
            relevant_events.push(shift);
        } else {
            non_relevant_events.push(shift);
        }
    }

    return (relevant_events, non_relevant_events);
}

// If true, the partial calendars need to be recreated. If date has changed
// If false, doesn't need to happen
// None, unknown, error occured
fn is_partial_calendar_regeneration_needed() -> GenResult<Option<bool>> {
    let current_date = match OffsetDateTime::now_local() {
        Ok(date) => date.date(),
        Err(_err) => {
            warn!("failed to get current date");
            return Ok(None);
        }
    };
    let previous_execution_date = match || -> GenResult<Date> {
        let previous_execution_date_str = read_to_string(PREVIOUS_EXECUTION_DATE_PATH)?;
        Ok(from_str::<Date>(&previous_execution_date_str)?)
    }() {
        Ok(date) => date,
        Err(err) => {
            warn!(
                "Getting previous execution date went wrong. Err: {}",
                err.to_string()
            );
            _ = write(
                PREVIOUS_EXECUTION_DATE_PATH,
                serde_json::to_string(&current_date)?.as_bytes(),
            );
            return Ok(None);
        }
    };
    debug!("Current date: {current_date}");
    _ = write(
        PREVIOUS_EXECUTION_DATE_PATH,
        serde_json::to_string(&current_date)?.as_bytes(),
    );
    if previous_execution_date != current_date {
        Ok(Some(true))
    } else {
        Ok(Some(false))
    }
}

fn event_to_shift(events: Vec<Event>) -> Vec<Shift> {
    let mut previous_shift_map = vec![];
    for event in events {
        if let Some(shift_string) = event.property_value("X-BUSSIE-METADATA") {
            if let Ok(shift) = serde_json::from_str::<Shift>(shift_string) {
                // let mut shift = shift;
                // All shifts are marked to be deleted. As if they are not marked that later on we know they really should be deleted
                // shift.state = ShiftState::Deleted;
                previous_shift_map.push(shift);
            }
        }
    }
    previous_shift_map
}

// Save relevant shifts to disk
pub fn save_partial_shift_files(shifts: &Vec<Shift>) -> GenResult<()> {
    let (relevant_shifts, non_relevant_shifts) = split_relevant_shifts(shifts.clone());
    write(
        RELEVANT_EVENTS_PATH,
        serde_json::to_string_pretty(&relevant_shifts)?,
    )
    .warn("Saving relevant shifts");
    write(
        NON_RELEVANT_EVENTS_PATH,
        serde_json::to_string_pretty(&non_relevant_shifts)?,
    )
    .warn("Saving non-relevant shifts");
    Ok(())
}

#[derive(Debug)]
pub struct PreviousShiftInformation {
    pub previous_relevant_shifts: Vec<Shift>,
    pub previous_non_relevant_shifts: Vec<Shift>,
}

impl PreviousShiftInformation {
    pub fn new() -> Self {
        Self {
            previous_non_relevant_shifts: vec![],
            previous_relevant_shifts: vec![],
        }
    }
}

pub fn get_ical_path() -> GenResult<PathBuf> {
    // var("SAVE_TARGET")?
    let mut ical_path = PathBuf::new();
    ical_path.push(var("SAVE_TARGET")?);
    ical_path.push(create_ical_filename()?);
    Ok(ical_path)
}

pub fn get_previous_shifts() -> GenResult<Option<PreviousShiftInformation>> {
    let relevant_events_exist = Path::new(RELEVANT_EVENTS_PATH).exists();
    let non_relevant_events_exist = Path::new(NON_RELEVANT_EVENTS_PATH).exists();
    let main_ical_path = get_ical_path()?;
    if is_partial_calendar_regeneration_needed()?.is_none_or(|needed| needed)
        || !(relevant_events_exist && non_relevant_events_exist)
    {
        info!("calendar regeneration needed");
        if !main_ical_path.exists() {
            return Ok(None);
        }
        let main_calendar = match load_ical_file(&main_ical_path) {
            Ok(calendar) => calendar,
            Err(err) => {
                return match err.downcast_ref::<CalendarVersionError>() {
                    Some(ver_err) if ver_err == &CalendarVersionError::BreakingChange => Ok(None),
                    Some(ver_err) if ver_err == &CalendarVersionError::WelcomeChange => {
                        info!("Removing existing calendar file");
                        _ = fs::remove_file(main_ical_path);
                        Ok(None)
                    }
                    _ => Err(err),
                };
            }
        };
        let calendar_events = get_calendar_events(main_calendar);
        let calendar_shifts = event_to_shift(calendar_events);
        let (previous_relevant_shifts, previous_non_relevant_shifts) =
            split_relevant_shifts(calendar_shifts);

        let previous_relevant_shifts: Vec<Shift> = previous_relevant_shifts
            .into_iter()
            .map(|mut shift| {
                shift.state = ShiftState::Deleted;
                shift
            })
            .collect();
        debug!(
            "Got {} relevant and {} non-relevant events",
            previous_relevant_shifts.len(),
            previous_non_relevant_shifts.len()
        );
        Ok(Some(PreviousShiftInformation {
            previous_relevant_shifts,
            previous_non_relevant_shifts,
        }))
    } else {
        info!("Calendar regeneration NOT needed");
        let relevant_shift_str = read_to_string(RELEVANT_EVENTS_PATH)?;
        let non_relevant_shifts_str = read_to_string(NON_RELEVANT_EVENTS_PATH)?;
        let previous_relevant_shifts: Vec<Shift> = serde_json::from_str(&relevant_shift_str)?;
        // All relevant shifts MUST FIRST BE MARKED AS DELETED for deleted shift detection to work
        let previous_relevant_shifts = previous_relevant_shifts
            .into_iter()
            .map(|mut shift| {
                shift.state = ShiftState::Deleted;
                shift
            })
            .collect();
        let previous_non_relevant_shifts: Vec<Shift> =
            serde_json::from_str(&non_relevant_shifts_str)?;
        Ok(Some(PreviousShiftInformation {
            previous_relevant_shifts,
            previous_non_relevant_shifts,
        }))
    }
}

/*
let previous_execution_date = match Date::parse(&read_to_string(PREVIOUS_EXECUTION_DATE_PATH).unwrap_or_default(), DATE_DESCRIPTION) {
        Ok(date) => (date.year()-2025 * 365) + 31*date.month().into() + date.day(),
        Err(err) => {warn!("Getting previous execution date went wrong. Err: {}",err.to_string());
            return (events,None)}
    }; */

fn create_event(shift: &Shift, metadata: Option<&Shift>) -> Event {
    let shift_link = create_shift_link(shift, true).unwrap_or("ERROR".to_owned());
    let cut_off_end_time = if let Some(end_time) = shift.original_end_time {
        format!(
            " ⏺ \nEindtijd - {}",
            end_time.format(TIME_DESCRIPTION).unwrap_or_default()
        )
    } else {
        String::new()
    };
    Event::new()
        .summary(&format!("Dienst - {}{cut_off_end_time}", shift.number))
        .description(&format!(
            "Dienstsoort • {}
Duur • {} uur {} minuten
Omschrijving • {}
Shift sheet • {}",
            shift.kind,
            shift.duration.whole_hours(),
            shift.duration.whole_minutes() % 60,
            shift.description,
            shift_link
        ))
        .location(&shift.location)
        .append_property(icalendar::Property::new(
            "X-BUSSIE-METADATA",
            &serde_json::to_string(metadata.unwrap_or(shift)).unwrap_or_default(),
        ))
        .starts(create_dateperhapstime(shift.date, shift.start))
        .ends(create_dateperhapstime(shift.end_date, shift.end))
        .done()
}

/*
Creates the ICAL file to add to the calendar
Needs previous exit code so it can add it to the calendar
Will later be replaced with current exit code if its different
*/
pub fn create_ical(
    shifts: &Vec<Shift>,
    metadata: Vec<Shift>,
    previous_exit_code: &FailureType,
) -> String {
    let metadata_shifts_hashmap: HashMap<i64, Shift> = metadata
        .into_iter()
        .map(|x| (x.magic_number, x)) // Replace `operation(x)` with your specific operation
        .collect();
    let name = set_get_name(None);
    let admin_email = var("MAIL_ERROR_TO").unwrap_or_default();
    // get the current systemtime as a unix timestamp
    let current_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0));
    let heartbeat_interval: i32 = var("KUMA_HEARTBEAT_INTERVAL")
        .unwrap_or("0".to_owned())
        .parse()
        .unwrap_or(0);
    info!("Creating calendar file...");
    let mut calendar = Calendar::new()
        .name(&format!("Hermes rooster - {}", name))
        .append_property(("X-USER-NAME", name.as_str()))
        .append_property((
            "X-LAST-UPDATED",
            current_timestamp.as_secs().to_string().as_str(),
        ))
        .append_property((
            "X-UPDATE-INTERVAL-SECONDS",
            heartbeat_interval.to_string().as_str(),
        ))
        .append_property(("X-CAL-VERSION", CALENDAR_VERSION.to_string().as_str()))
        .append_property(("X-ADMIN-EMAIL", admin_email.as_str()))
        .append_property((
            "X-EXIT-CODE",
            serde_json::to_string(&previous_exit_code)
                .unwrap_or("OK".to_owned())
                .as_str(),
        ))
        .append_property(("METHOD", "PUBLISH"))
        .timezone("Europe/Amsterdam")
        .done();
    for shift in shifts {
        let metadata_shift = metadata_shifts_hashmap.get(&shift.magic_number);
        calendar.push(create_event(&shift, metadata_shift));
    }
    String::from(calendar.to_string())
}

/*
I use the create Time to keep track of dates and time. But the crate used for creating the ICAL file uses chrono to keep time.
*/
fn create_dateperhapstime(date: Date, time: Time) -> CalendarDateTime {
    let date_day = date.day();
    let date_month = date.month() as u8;
    let date_year = date.year();
    let time_hrs = time.hour();
    let time_min = time.minute();
    let naive_time = NaiveTime::from_hms_opt(time_hrs as u32, time_min as u32, 0).unwrap();
    let naive_date =
        NaiveDate::from_ymd_opt(date_year, date_month as u32, date_day as u32).unwrap();
    let naive_date_time = NaiveDateTime::new(naive_date, naive_time);
    CalendarDateTime::WithTimezone {
        date_time: naive_date_time,
        tzid: "Europe/Amsterdam".to_string(),
    }
}
