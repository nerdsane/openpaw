//! Cron Activate — computes the first `next_run_at` from a cron schedule.
//!
//! Parses the `schedule` field (cron expression) and computes the next
//! fire time as an ISO 8601 timestamp. Called on Activate and Resume.

use temper_wasm_sdk::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "cron_activate: computing first next_run_at");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let schedule = fields
            .get("schedule")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if schedule.is_empty() {
            return Err("No schedule configured on CronJob".to_string());
        }

        let interval_secs = parse_cron_interval(schedule);
        if interval_secs == 0 {
            return Err(format!(
                "Could not parse cron schedule '{schedule}' into an interval"
            ));
        }

        // Compute next_run_at = now + interval
        let now_secs = current_unix_secs();
        let next_secs = now_secs + interval_secs;
        let next_run_at = unix_to_iso8601(next_secs);

        ctx.log(
            "info",
            &format!(
                "cron_activate: schedule='{}' interval={}s next_run_at={}",
                schedule, interval_secs, next_run_at
            ),
        );

        set_success_result(
            "ActivateComplete",
            &json!({ "next_run_at": next_run_at }),
        );

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Simple cron interval parser — extracts a repeat interval in seconds.
///
/// Supports common patterns:
/// - `* * * * *` → every 60s (every minute)
/// - `*/5 * * * *` → every 300s (every 5 minutes)
/// - `0 * * * *` → every 3600s (every hour)
/// - `0 */2 * * *` → every 7200s (every 2 hours)
/// - `0 */6 * * *` → every 21600s (every 6 hours)
/// - `0 0 * * *` → every 86400s (every day)
///
/// Returns 0 if the expression can't be parsed into a simple interval.
fn parse_cron_interval(schedule: &str) -> u64 {
    let parts: Vec<&str> = schedule.split_whitespace().collect();
    if parts.len() != 5 {
        return 0;
    }

    let (minute, hour) = (parts[0], parts[1]);

    // Every N minutes: */N * * * *
    if let Some(n) = minute.strip_prefix("*/") {
        if let Ok(n) = n.parse::<u64>() {
            return n * 60;
        }
    }

    // Every minute: * * * * *
    if minute == "*" && hour == "*" {
        return 60;
    }

    // Fixed minute, variable hour patterns
    if minute == "0" {
        // Every N hours: 0 */N * * *
        if let Some(n) = hour.strip_prefix("*/") {
            if let Ok(n) = n.parse::<u64>() {
                return n * 3600;
            }
        }
        // Every hour: 0 * * * *
        if hour == "*" {
            return 3600;
        }
        // Every day: 0 0 * * *
        if hour == "0" {
            return 86400;
        }
    }

    0
}

/// Get current time as unix seconds.
fn current_unix_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Convert unix seconds to ISO 8601 UTC string.
fn unix_to_iso8601(secs: u64) -> String {
    let mut days = (secs / 86400) as i64;
    let day_secs = secs % 86400;
    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;
    let second = day_secs % 60;

    let mut year = 1970i64;
    loop {
        let ydays = if is_leap_year(year) { 366 } else { 365 };
        if days < ydays {
            break;
        }
        days -= ydays;
        year += 1;
    }

    let leap = is_leap_year(year);
    let mdays = [
        31,
        if leap { 29 } else { 28 },
        31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    let mut month = 0usize;
    for (i, &md) in mdays.iter().enumerate() {
        if days < md as i64 {
            month = i;
            break;
        }
        days -= md as i64;
    }

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month + 1,
        days + 1,
        hour,
        minute,
        second
    )
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
