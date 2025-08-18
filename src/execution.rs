use std::{fs::{read_to_string, write}, sync::Arc, time::Duration};

use chrono::{Timelike, Utc};
use dotenvy::var;
use tokio::{sync::Notify, time::sleep};

use crate::{create_path, GenResult};

fn get_execution_properties() -> (Duration, u8) {
    let cycle_time = || -> GenResult<u64> {
        Ok(var("CYCLE_TIME")
            .unwrap_or((var("KUMA_HEARTBEAT_INTERVAL")?.parse::<u64>()? - 400).to_string())
            .parse::<u64>()?)
    }()
    .unwrap_or(7200);
    let starting_minute = || -> GenResult<u8> {
        let path = create_path("starting_minute");
        let starting_minute_str =
            read_to_string(&path).unwrap_or(rand::random_range(0..60).to_string());
        _ = write(&path, starting_minute_str.as_bytes());
        Ok(starting_minute_str.parse()?)
    }()
    .unwrap_or(rand::random_range(0..60));
    (Duration::from_secs(cycle_time), starting_minute)
}

pub async fn execution_manager(notification: Arc<Notify>, instant_run: bool) {
    let execution_properties = get_execution_properties();
    let current_time = Utc::now();
    if current_time.minute() != execution_properties.1 as u32 || instant_run {
        let current_minute = current_time.minute() as i8;
        let mut waiting_minutes = execution_properties.1 as i8 - current_minute;
        if waiting_minutes < 0 {
            waiting_minutes += 60;
        }
        debug!("Waiting {waiting_minutes} minutes until execution");
        sleep(Duration::from_secs(waiting_minutes as u64 * 60)).await;
    }
    loop {
        info!("Starting execution loop");
        notification.notify_waiters();
        sleep(execution_properties.0).await;
    }

}