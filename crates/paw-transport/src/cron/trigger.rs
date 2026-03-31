//! Cron trigger — periodic scanner for due CronJob entities.
//!
//! ONE entity, ONE action. Queries active CronJobs, dispatches `Trigger`
//! on any job whose `next_run_at` is in the past. Everything else (computing
//! next run, performing the scheduled work) is WASM integrations.

use serde_json::json;

use crate::PawApiClient;

/// Configuration for the cron trigger.
#[derive(Debug, Clone)]
pub struct CronTriggerConfig {
    /// How often (in seconds) to re-scan for new/changed jobs. Default: 60.
    pub check_interval_secs: u64,
}

/// Cron trigger — background loop that fires due CronJob entities.
pub struct CronTrigger {
    config: CronTriggerConfig,
    api: PawApiClient,
}

impl CronTrigger {
    /// Create a new cron trigger.
    pub fn new(config: CronTriggerConfig, api: PawApiClient) -> Self {
        Self { config, api }
    }

    /// Run the cron trigger loop. This never returns under normal operation.
    ///
    /// Each iteration:
    /// 1. Query all Active CronJobs via OData
    /// 2. For each job with `next_run_at` <= now, dispatch `OpenPaw.Trigger`
    /// 3. Sleep for `check_interval_secs`
    pub async fn run(&self) -> Result<(), String> {
        println!("  [cron] Trigger started");
        loop {
            let jobs = self.query_active_jobs().await;

            match jobs {
                Ok(jobs) if !jobs.is_empty() => {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();

                    for job in &jobs {
                        let job_id = job
                            .get("entity_id")
                            .or_else(|| job.get("Id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if job_id.is_empty() {
                            continue;
                        }

                        let should_trigger = self.should_trigger(job, now);
                        if should_trigger {
                            println!("  [cron] Triggering job {job_id}");
                            if let Err(e) = self.trigger_job(job_id).await {
                                eprintln!("  [cron] Failed to trigger job {job_id}: {e}");
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("  [cron] Failed to query jobs: {e}");
                }
                _ => {} // No active jobs, just wait
            }

            tokio::time::sleep(std::time::Duration::from_secs(self.config.check_interval_secs))
                .await;
        }
    }

    /// Query all CronJob entities with Status='Active'.
    async fn query_active_jobs(&self) -> Result<Vec<serde_json::Value>, String> {
        self.api
            .query_entities("CronJobs", "Status eq 'Active'")
            .await
    }

    /// Dispatch the `OpenPaw.Trigger` action on a CronJob entity.
    async fn trigger_job(&self, job_id: &str) -> Result<(), String> {
        self.api
            .dispatch_action("CronJobs", job_id, "OpenPaw.Trigger", json!({}))
            .await
            .map(|_| ())
    }

    /// Determine if a CronJob should be triggered now.
    ///
    /// Strategy:
    /// 1. If `next_run_at` is set and <= now → trigger
    /// 2. If `next_run_at` is empty but `schedule` exists:
    ///    a. Parse schedule as simple interval (e.g. "*/5 * * * *" → every 5 min)
    ///    b. If `last_run_at` is empty → trigger (first run)
    ///    c. If `last_run_at` + interval <= now → trigger
    fn should_trigger(&self, job: &serde_json::Value, now: u64) -> bool {
        let fields = job.get("fields").unwrap_or(job);

        // Check next_run_at first
        let next_run = self.parse_next_run_at(job);
        if next_run > 0 && next_run <= now {
            return true;
        }

        // Fallback: use schedule field with last_run_at
        let schedule = fields
            .get("schedule")
            .or_else(|| fields.get("Schedule"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if schedule.is_empty() {
            return false;
        }

        let last_run = fields
            .get("last_run_at")
            .or_else(|| fields.get("LastRunAt"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // If never run, trigger immediately
        if last_run.is_empty() {
            return true;
        }

        // Parse last_run_at and compute interval from schedule
        let last_run_secs = parse_iso8601_to_unix(last_run).unwrap_or(0);
        if last_run_secs == 0 {
            return true; // Can't parse last run, trigger
        }

        let interval_secs = parse_cron_interval(schedule);
        if interval_secs == 0 {
            return false; // Can't parse schedule
        }

        last_run_secs + interval_secs <= now
    }

    /// Parse the `next_run_at` field from a CronJob entity as unix seconds.
    ///
    /// Supports ISO 8601 timestamps (e.g. `2025-01-15T10:30:00Z`) and plain
    /// unix-second integers. Returns 0 if the field is missing or unparseable.
    fn parse_next_run_at(&self, job: &serde_json::Value) -> u64 {
        let Some(val) = job.get("next_run_at") else {
            return 0;
        };

        // Try as integer first (unix seconds).
        if let Some(n) = val.as_u64() {
            return n;
        }

        // Try as string — either numeric or ISO 8601.
        if let Some(s) = val.as_str() {
            if s.is_empty() {
                return 0;
            }
            // Plain numeric string.
            if let Ok(n) = s.parse::<u64>() {
                return n;
            }
            // ISO 8601 — parse manually without pulling in chrono.
            // Accepts: YYYY-MM-DDTHH:MM:SSZ or YYYY-MM-DDTHH:MM:SS+00:00
            return parse_iso8601_to_unix(s).unwrap_or(0);
        }

        0
    }
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

    let (minute, hour, _dom, _month, _dow) = (parts[0], parts[1], parts[2], parts[3], parts[4]);

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

    // Every N hours: 0 */N * * *
    if minute == "0" {
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

/// Minimal ISO 8601 parser — converts a UTC timestamp to unix seconds.
///
/// We avoid pulling in the `chrono` crate just for this one conversion.
/// Supports `YYYY-MM-DDTHH:MM:SSZ` and `YYYY-MM-DDTHH:MM:SS+00:00`.
fn parse_iso8601_to_unix(s: &str) -> Option<u64> {
    // Strip trailing 'Z' or '+00:00'.
    let s = s.trim();
    let s = s
        .strip_suffix('Z')
        .or_else(|| s.strip_suffix("+00:00"))
        .unwrap_or(s);

    // Split into date and time at 'T'.
    let (date_part, time_part) = s.split_once('T')?;
    let mut date_iter = date_part.split('-');
    let year: u64 = date_iter.next()?.parse().ok()?;
    let month: u64 = date_iter.next()?.parse().ok()?;
    let day: u64 = date_iter.next()?.parse().ok()?;

    let mut time_iter = time_part.split(':');
    let hour: u64 = time_iter.next()?.parse().ok()?;
    let min: u64 = time_iter.next()?.parse().ok()?;
    let sec: u64 = time_iter.next()?.parse().ok()?;

    // Days from year 0 to epoch (1970-01-01) using a simplified algorithm.
    fn days_from_civil(y: u64, m: u64, d: u64) -> u64 {
        // Shift March-based year for leap-year simplicity.
        let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
        let era = y / 400;
        let yoe = y - era * 400;
        let doy = (153 * m + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe
    }

    let epoch_days = days_from_civil(1970, 1, 1);
    let target_days = days_from_civil(year, month, day);
    if target_days < epoch_days {
        return None;
    }
    let days_since_epoch = target_days - epoch_days;

    Some(days_since_epoch * 86400 + hour * 3600 + min * 60 + sec)
}
