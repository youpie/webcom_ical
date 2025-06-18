use std::{fs::read_to_string, path::Path};

use crate::{GenResult, Shift, create_shift_link, set_get_name};
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use icalendar::{
    Calendar, CalendarComponent, CalendarDateTime, Component, Event, EventLike,
    parser::{read_calendar, unfold},
};
use time::{Date, OffsetDateTime, Time};

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
// 1st element is relevant, second element is non-relevant. If it returns none, something went wrong getting the previous execution date
fn split_calendar(events: Vec<Event>) -> (Vec<Event>, Option<Vec<Event>>) {
    const PREVIOUS_EXECUTION_DATE_PATH: &str = "./kuma/previous_execution_date";
    // The date is how many days have elapsed since 1-1-2025. Assuming 31 days per month
    let current_date = match OffsetDateTime::now_local().map(|date_time| {
        let date = date_time.date();
        (date.year() - 2025 * 365) + 31 * date.month() as i32 + date.day() as i32
    }) {
        Ok(date) => date,
        Err(err) => {
            warn!("failed to get current date");
            return (events, None);
        }
    };
    let mut non_relevant_events = vec![];
    let mut relevant_events = vec![];
    for event in events {
        // If event date is unknown. Just add it to the non relevant events
        let event_date = if let Some(event_date) = event.get_start() {
            let date = event_date.date_naive();
            (date.year() as i32 - 2025) * 365
                + (date.month0() as i32 + 1) * 31
                + (date.day0() as i32 + 1)
        } else {
            non_relevant_events.push(event);
            continue;
        };
        if current_date - event_date < 28 {
            relevant_events.push(event);
        } else {
            non_relevant_events.push(event);
        }
    }

    (
        relevant_events,
        if non_relevant_events.is_empty() {
            None
        } else {
            Some(non_relevant_events)
        },
    )
}

/*
let previous_execution_date = match Date::parse(&read_to_string(PREVIOUS_EXECUTION_DATE_PATH).unwrap_or_default(), DATE_DESCRIPTION) {
        Ok(date) => (date.year()-2025 * 365) + 31*date.month().into() + date.day(),
        Err(err) => {warn!("Getting previous execution date went wrong. Err: {}",err.to_string());
            return (events,None)}
    }; */

/*
Creates the ICAL file to add to the calendar
*/
pub fn create_ical(shifts: &Vec<Shift>) -> String {
    let name = set_get_name(None);
    info!("Creating calendar file...");
    let mut calendar = Calendar::new()
        .name(&format!("Hermes rooster - {}", name))
        .append_property(("X-USER-NAME", name.as_str()))
        .append_property(("METHOD", "PUBLISH"))
        .timezone("Europe/Amsterdam")
        .done();
    for shift in shifts {
        let shift_link = create_shift_link(shift).unwrap_or("ERROR".to_owned());
        calendar.push(
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
                .done(),
        );
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
