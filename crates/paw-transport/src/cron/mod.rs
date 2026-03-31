//! Cron trigger — polls active CronJob entities and dispatches Trigger actions.
//!
//! ONE entity, ONE action per due job. The trigger queries CronJobs with
//! Status='Active', checks `next_run_at`, and dispatches `OpenPaw.Trigger`
//! when a job is due. Everything else (schedule parsing, side effects) is
//! handled by WASM integrations on CronJob state transitions.

mod trigger;

pub use trigger::{CronTrigger, CronTriggerConfig};
