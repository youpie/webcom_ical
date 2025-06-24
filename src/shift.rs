use std::{hash::{DefaultHasher, Hash, Hasher}, str::Split};

use serde::{Deserialize, Serialize};
use time::{Date, Duration, Time};

use crate::get_time;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum ShiftState {
    New,
    Changed,
    Deleted,
    Unchanged,
    #[default]
    Unknown
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shift {
    pub date: Date,
    pub start: Time,
    pub end_date: Date,
    pub end: Time,
    pub duration: Duration,
    pub number: String,
    pub kind: String,
    pub location: String,
    pub description: String,
    pub is_broken: bool,
    // If the shift is broken, between what time is the user free
    pub broken_period: Option<(Time, Time)>,
    pub magic_number: i64,
    // This field is not always needed. Especially when serializing.
    #[serde(skip, default)]
    pub state: ShiftState
}

impl Shift {
    /*
    Creates a new Shift struct from a simple string straight from webcom
    Also hashes the string to see if it has been updated
    Looks intimidating, bus is mostly boilerplate + a bit of logic for correctly parsing the duration
    */
    pub fn new(text: String, date: Date) -> Self {
        let text_clone = text.clone();
        let parts = text_clone.split("\u{a0}• \u{a0}• ");
        let mut location_modifier = 1;
        let parts_clean: Vec<String> = parts
            .map(|x| {
                let y = x.replace("\u{a0}• ", "");
                y
            })
            .collect();
        let mut parts_list: Vec<Split<'_, &str>> =
            parts_clean.iter().map(|x| x.split(": ")).collect();
        let number: String = parts_list[0].nth(1).unwrap().to_string();
        let _date: String = parts_list[1].nth(1).unwrap().to_string();
        let time: String = parts_list[2].nth(1).unwrap_or("").to_string();
        let shift_duration: String = parts_list[3].nth(1).unwrap_or("").to_string();
        let _working_hours: String = parts_list[4].nth(1).unwrap_or("").to_string();
        let _day_of_week: String = parts_list[5].nth(1).unwrap_or("").to_string();
        let kind: String = parts_list[6].nth(1).unwrap_or("").to_string();
        let mut location = "Onbekend".to_string();
        if parts_list[7].next().unwrap_or("") == "Startplaats" {
            location_modifier = 0;
            location = parts_list[7].next().unwrap_or("").to_string();
        }
        let description: String = parts_list[8 - location_modifier]
            .nth(1)
            .unwrap_or("")
            .to_string();
        let start_time_str = time.split_whitespace().nth(0).unwrap();
        let end_time_str = time.split_whitespace().nth(2).unwrap();
        let start = get_time(start_time_str);
        let end = get_time(end_time_str);
        let mut is_broken = false;
        let shift_type = number.chars().nth(0).unwrap();
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let magic_number = (hasher.finish() as i128 - i64::MAX as i128) as i64;
        if shift_type == 'g' || shift_type == 'G' {
            is_broken = true;
        }

        let duration_split = shift_duration.split_whitespace().nth(0).unwrap().split(":");
        let duration_minutes = Duration::minutes(
            duration_split
                .clone()
                .nth(1)
                .unwrap()
                .parse::<i64>()
                .unwrap(),
        );
        let duration_hours = Duration::hours(
            duration_split
                .clone()
                .nth(0)
                .unwrap()
                .parse::<i64>()
                .unwrap(),
        );
        let duration = duration_hours + duration_minutes;
        let mut end_date = date;
        if end < start {
            end_date = date + Duration::days(1);
        }
        Self {
            date,
            number,
            start,
            end_date,
            end,
            duration,
            kind,
            location,
            description,
            is_broken,
            broken_period: None,
            magic_number,
            state: ShiftState::Unknown
        }
    }

    // Create two new shifts from one broken shift.
    // Assumes second shift cannot start after midnight
    // None means no broken times have been found for the shift
    pub fn split_broken(
        &self,
    ) -> Option<Vec<Self>> {
        let mut part_one = self.clone();
        let break_period = match self.broken_period {
            Some(period) => period,
            None => {return None;}
        };
        part_one.end = break_period.0;
        let mut part_two = self.clone();
        part_two.start = break_period.1;
        let shifts: Vec<Self> = vec![part_one, part_two];
        Some(shifts)
    }

    // Create two new shifts from one broken shift.
    // Assumes second shift cannot start after midnight
    pub fn new_from_existing(
        new_between_times: (Time, Time),
        existing_shift: &Self,
        start_next_day: bool,
    ) -> Vec<Self> {
        let mut part_one = existing_shift.clone();
        part_one.end = new_between_times.0;
        part_one.end_date = match start_next_day {
            true => existing_shift.end_date,
            false => existing_shift.date,
        };
        let mut part_two = existing_shift.clone();
        part_two.start = new_between_times.1;
        part_two.date = match start_next_day {
            true => existing_shift.end_date,
            false => existing_shift.date,
        };
        let shifts: Vec<Self> = vec![part_one, part_two];
        shifts
    }

}