fn find_send_shift_mails(
    mailer: &SmtpTransport,
    previous_shifts: &Vec<Shift>,
    current_shifts: &Vec<Shift>,
    new_shifts: Option<&Vec<Shift>>,
    env: &EnvMailVariables,
    send_mail: bool,
) -> GenResult<Vec<Shift>> {
    let mut updated_shifts = Vec::new();
    let mut new_shifts_list = Vec::new();
    let current_date: Date = Date::parse(
        &chrono::offset::Local::now().format("%d-%m-%Y").to_string(),
        format_description!("[day]-[month]-[year]"),
    )?;

    // Track shifts by start date
    let mut previous_by_start: std::collections::HashMap<_, _> = previous_shifts
        .iter()
        .map(|shift| (shift.start, shift))
        .collect();

    // Iterate through the current shifts to check for updates or new shifts
    for current_shift in current_shifts {
        if current_shift.start < current_date {
            continue; // Skip old shifts
        }

        match previous_by_start.get(&current_shift.start) {
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

    if !new_shifts_list.is_empty() && send_mail {
        create_send_new_email(mailer, &new_shifts_list, env, false)?;
    }

    if !updated_shifts.is_empty() && send_mail {
        create_send_new_email(mailer, &updated_shifts, env, true)?;
    }

    Ok(updated_shifts)
}
