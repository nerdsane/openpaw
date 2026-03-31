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
                        let next_run = self.parse_next_run_at(job);
                        if next_run > 0 && next_run <= now {
                            let job_id = job
                                .get("entity_id")
                                .or_else(|| job.get("Id"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            if !job_id.is_empty() {
                                if let Err(e) = self.trigger_job(job_id).await {
                                    eprintln!("  [cron] Failed to trigger job {job_id}: {e}");
                                }
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
