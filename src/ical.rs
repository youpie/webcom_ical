use std::{fs::{read_to_string, write}, path::{Path, PathBuf}, time::{Duration, SystemTime, UNIX_EPOCH}};

use crate::{create_ical_filename, create_shift_link, set_get_name, GenResult, Shift, ShiftState};
use chrono::{Datelike, Local, Months, NaiveDate, NaiveDateTime, NaiveTime};
use dotenvy::var;
use icalendar::{
    Calendar, CalendarComponent, CalendarDateTime, Component, Event, EventLike,
    parser::{read_calendar, unfold},
};
use serde_json::from_str;
use time::{Date, Month, OffsetDateTime, Time};

const PREVIOUS_EXECUTION_DATE_PATH: &str = "./kuma/previous_execution_date";
const NON_RELEVANT_EVENTS_PATH: &str = "./kuma/non_relevant_events";
const RELEVANT_EVENTS_PATH: &str = "./kuma/relevant_events";

pub fn load_ical_file(path: &Path) -> GenResult<Calendar> {
    let calendar_string = read_to_string(path)?;
    Ok(read_calendar(&unfold(&calendar_string))?.into())
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

// Returns two vecs of events, one of shifts more than 28 days ago, one of less than that
// Only the shifts less than a month ago are actually used
// 1st element is relevant, second element is non-relevant. If it returns none, something went wrong getting the current date
fn split_calendar(events: Vec<Event>) -> (Vec<Event>, Option<Vec<Event>>) {
    // The date is how many days have elapsed since 1-1-2025. Assuming 31 days per month
    let today = Local::now().date_naive();
    let first_of_this_month = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();

    // Subtract one month, with proper handling for end-of-month behavior
    let cutoff = first_of_this_month
        .checked_sub_months(Months::new(1))
        .unwrap(); 

    let mut non_relevant_events = vec![];
    let mut relevant_events = vec![];
    for event in events {
        // If event date is unknown. Just add it to the non relevant events
        let event_date = if let Some(event_date) = event.get_start() {
            event_date.date_naive()
        } else {
            non_relevant_events.push(event);
            continue;
        };
        if event_date < cutoff {
            relevant_events.push(event);
        } else {
            non_relevant_events.push(event);
        }
    }

    return (
        relevant_events,
        Some(non_relevant_events)
    );
}

// If true, the partial calendars need to be recreated. If date has changed
// If false, doesn't need to happen
// None, unknown, error occured
fn is_partial_calendar_regeneration_needed() -> Option<bool> {
    let current_date = match OffsetDateTime::now_local() {
        Ok(date) => date.date(),
        Err(_err) => {
            warn!("failed to get current date");
            return None;
        }
    };
    let previous_execution_date =  match || -> GenResult<Date> {
        let previous_execution_file = read_to_string(PREVIOUS_EXECUTION_DATE_PATH)?;
        Ok(from_str::<Date>(&previous_execution_file)?)
    }() {
        Ok(date) => date,
        Err(err) => {warn!("Getting previous execution date went wrong. Err: {}",err.to_string());
            _  = write(PREVIOUS_EXECUTION_DATE_PATH, serde_json::to_string(&current_date).unwrap().as_bytes());
            return None}
    };
    debug!("Current date: {current_date}");
    _ = write(PREVIOUS_EXECUTION_DATE_PATH, serde_json::to_string(&current_date).unwrap().as_bytes());
    if previous_execution_date != current_date {
        Some(true)
    }
    else {
        Some(false)
    }
}

fn create_shift_hashmap(events: Vec<Event>) -> Vec<Shift> {
    let mut previous_shift_map = vec![];
    for event in events {
        if let Some(shift_string) = event.property_value("X-BUSSIE-METADATA") {
            if let Ok(shift) = serde_json::from_str::<Shift>(shift_string) {
                let mut shift = shift;
                // All shifts are marked to be deleted. As if they are not marked that later on we know they really should be deleted
                shift.state = ShiftState::Deleted;
                previous_shift_map.push(shift);
            }
        }
    } 
    previous_shift_map
}

// Save relevant shifts to disk
pub fn save_relevant_shifts(relevant_shifts: &Vec<Shift>) -> GenResult<()> {
    match write(RELEVANT_EVENTS_PATH, serde_json::to_string_pretty(relevant_shifts)?) {
        Ok(_) => info!("Saving Relevant shifts to disk was succesful"),
        Err(err) => error!("Saving Relevant shifts to disk FAILED. ERROR: {}",err.to_string())
    };
    Ok(())
}

#[derive(Debug)]
pub struct PreviousShiftInformation {
    pub previous_relevant_shifts: Vec<Shift>,
    pub previous_non_relevant_shifts: Vec<Shift>,
}

impl PreviousShiftInformation {
    pub fn new_from_relevant(shifts: Vec<Shift>) -> Self{
        Self { previous_relevant_shifts: shifts, previous_non_relevant_shifts: vec![] }
    }
    pub fn new() -> Self {
        Self {
            previous_non_relevant_shifts: vec![],
            previous_relevant_shifts: vec![]
        }
    }
}

pub fn get_previous_shifts() -> GenResult<Option<PreviousShiftInformation>> {
    let relevant_events_exist = Path::new(RELEVANT_EVENTS_PATH).exists();
    let non_relevant_events_exist = Path::new(NON_RELEVANT_EVENTS_PATH).exists();
    let main_ical_path = PathBuf::from(&format!(
        "{}{}",
        var("SAVE_TARGET").unwrap(),
        create_ical_filename()?
    ));
    if is_partial_calendar_regeneration_needed().is_none_or(|needed| needed) || !(relevant_events_exist && non_relevant_events_exist) {
        info!("calendar regeneration needed");
        if !main_ical_path.exists() {
            return Ok(None);
        }
        let main_calendar = load_ical_file(&main_ical_path)?;
        let calendar_events = get_calendar_events(main_calendar);
        let calendar_split = split_calendar(calendar_events);
        let previous_shifts_hash = create_shift_hashmap(calendar_split.0);
        // match write(RELEVANT_EVENTS_PATH, toml::to_string_pretty(&previous_shifts_hash).unwrap()) {
        //     Ok(_) => debug!("Saving relevant shifts to disk was succesful"),
        //     Err(err) => error!("Saving relevant shifts to disk FAILED. ERROR: {}",err.to_string())
        // };
        let previous_non_relevant_shifts: Vec<Shift> = create_shift_hashmap(calendar_split.1.unwrap_or_default());
        match write(NON_RELEVANT_EVENTS_PATH, serde_json::to_string_pretty(&previous_non_relevant_shifts)?) {
            Ok(_) => debug!("Saving non-relevant shifts to disk was succesful"),
            Err(err) => error!("Saving non-relevant shifts to disk FAILED. ERROR: {}",err.to_string())
        };
        Ok(Some(PreviousShiftInformation {
            previous_relevant_shifts: previous_shifts_hash,
            previous_non_relevant_shifts,
        }))
    } else {
        info!("Calendar regeneration NOT needed");
        let relevant_shift_str = read_to_string(RELEVANT_EVENTS_PATH).unwrap();
        let irrelevant_shift_str = read_to_string(NON_RELEVANT_EVENTS_PATH).unwrap();
        let previous_relevant_shifts: Vec<Shift> = serde_json::from_str(&relevant_shift_str).unwrap_or_default();
        // All relevant shifts MUST FIRST BE MARKED AS DELETED for deleted shift detection to work
        let previous_relevant_shifts = previous_relevant_shifts.into_iter().map(|mut shift| {shift.state = ShiftState::Deleted; shift}).collect();
        let previous_non_relevant_shifts: Vec<Shift> = serde_json::from_str(&irrelevant_shift_str).unwrap_or_default();
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

fn create_event(shift: &Shift) -> Event {
    let shift_link = create_shift_link(shift).unwrap_or("ERROR".to_owned());
    Event::new()
                .summary(&format!("Shift - {}", shift.number))
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
                    &serde_json::to_string(shift).unwrap(),
                ))
                .starts(create_dateperhapstime(shift.date, shift.start))
                .ends(create_dateperhapstime(shift.end_date, shift.end))
                .done()
}

/*
Creates the ICAL file to add to the calendar
*/
pub fn create_ical(relevant_shifts: &Vec<Shift>, non_relevant_shifts: Vec<Shift>) -> String {
    let mut shifts = non_relevant_shifts;
    shifts.append(&mut relevant_shifts.clone());
    let name = set_get_name(None);
    // get the current systemtime as a unix timestamp
    let current_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::from_secs(0));
    info!("Creating calendar file...");
    let mut calendar = Calendar::new()
        .name(&format!("Hermes rooster - {}", name))
        .append_property(("X-USER-NAME", name.as_str()))
        .append_property(("X-LAST-UPDATED", current_timestamp.as_secs().to_string().as_str()))
        .append_property(("METHOD", "PUBLISH"))
        .timezone("Europe/Amsterdam")
        .done();
    for shift in shifts {
        
        calendar.push(create_event(&shift));
            
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
