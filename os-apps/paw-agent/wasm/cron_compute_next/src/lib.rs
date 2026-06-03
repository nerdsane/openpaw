//! Cron Compute Next — single WASM module for cron schedule computation.
//!
//! Handles both Activate and Trigger modes:
//! - **activate**: Parses `schedule` field, computes first `next_run_at`,
//!   returns `ActivateComplete { next_run_at }`.
//! - **trigger**: Same schedule parsing, plus template substitution on
//!   `user_message_template` (replacing `{{run_count}}`, `{{last_result}}`,
//!   `{{now}}`), returns `TriggerComplete { next_run_at, user_message }`.
//!
//! Mode is determined by the `mode` key in integration config.

use temper_wasm_sdk::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CronSessionConfig {
    model: String,
    provider: String,
}

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let mode = ctx
            .config
            .get("mode")
            .map(|s| s.as_str())
            .unwrap_or("activate");

        ctx.log("info", &format!("cron_compute_next: mode={mode}"));

        // Parse schedule → interval → next_run_at
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

        let now_millis = Context::get_time_millis();
        let now_secs = (now_millis / 1000) as u64;
        let next_secs = now_secs + interval_secs;
        let next_run_at = unix_to_iso8601(next_secs);

        ctx.log(
            "info",
            &format!(
                "cron_compute_next: schedule='{}' interval={}s next_run_at={}",
                schedule, interval_secs, next_run_at
            ),
        );

        match mode {
            "trigger" => {
                // Template substitution for user_message
                let (template, existing_user_message) = cron_message_sources(&fields);
                let run_count = fields
                    .get("run_count")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let last_result = fields
                    .get("last_result")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let now_str = unix_to_iso8601(now_secs);

                let user_message = render_user_message(
                    &template,
                    &existing_user_message,
                    run_count,
                    last_result,
                    &now_str,
                )?;
                let config_json = config_json(&ctx);
                let session_config = resolve_cron_session_config(&fields, &config_json)?;

                ctx.log(
                    "info",
                    &format!(
                        "cron_compute_next: user_message length={}",
                        user_message.len()
                    ),
                );

                set_success_result(
                    "TriggerComplete",
                    &json!({
                        "next_run_at": next_run_at,
                        "user_message": user_message,
                        "model": session_config.model,
                        "provider": session_config.provider,
                    }),
                );
            }
            _ => {
                // activate mode (default)
                set_success_result("ActivateComplete", &json!({ "next_run_at": next_run_at }));
            }
        }

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

fn render_user_message(
    template: &str,
    existing_user_message: &str,
    run_count: i64,
    last_result: &str,
    now: &str,
) -> Result<String, String> {
    let source = if !template.trim().is_empty() {
        template
    } else {
        existing_user_message
    };
    let rendered = source
        .replace("{{run_count}}", &run_count.to_string())
        .replace("{{last_result}}", last_result)
        .replace("{{now}}", now);

    if rendered.trim().is_empty() {
        return Err(
            "CronJob user message is empty; configure user_message_template or user_message"
                .to_string(),
        );
    }

    Ok(rendered)
}

fn cron_message_sources(fields: &Value) -> (String, String) {
    (
        string_field_preserve_templates(fields, &["user_message_template", "UserMessageTemplate"])
            .unwrap_or_default(),
        string_field_preserve_templates(fields, &["user_message", "UserMessage"])
            .unwrap_or_default(),
    )
}

fn string_field_preserve_templates(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn resolve_cron_session_config(
    fields: &Value,
    config: &Value,
) -> Result<CronSessionConfig, String> {
    let model = string_field(fields, &["model", "Model"])
        .or_else(|| string_field(config, &["default_llm_model"]))
        .ok_or("CronJob model is empty; configure model or tenant llm_model")?;
    let provider = string_field(fields, &["provider", "Provider"])
        .or_else(|| string_field(config, &["default_llm_provider"]))
        .ok_or("CronJob provider is empty; configure provider or tenant llm_provider")?;

    Ok(CronSessionConfig { model, provider })
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty() && !value.contains("{secret:"))
            .map(str::to_string)
    })
}

fn config_json(ctx: &Context) -> Value {
    let mut object = serde_json::Map::new();
    for (key, value) in &ctx.config {
        object.insert(key.clone(), json!(value));
    }
    Value::Object(object)
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
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_user_message_falls_back_to_existing_message_when_template_empty() {
        let rendered = render_user_message(
            "",
            "Continue the katagami review queue.",
            3,
            "last result",
            "2026-06-03T13:00:00Z",
        )
        .expect("rendered message");

        assert_eq!(rendered, "Continue the katagami review queue.");
    }

    #[test]
    fn resolved_cron_session_config_uses_trigger_defaults_for_missing_model_provider() {
        let fields = json!({
            "model": "",
            "provider": ""
        });
        let config = json!({
            "default_llm_model": "gpt-5",
            "default_llm_provider": "openai_codex"
        });

        let resolved = resolve_cron_session_config(&fields, &config).expect("resolved config");

        assert_eq!(resolved.model, "gpt-5");
        assert_eq!(resolved.provider, "openai_codex");
    }

    #[test]
    fn render_user_message_reads_camel_case_cron_fields() {
        let fields = json!({
            "UserMessageTemplate": "",
            "UserMessage": "Proof message from OData casing"
        });

        let (template, existing_user_message) = cron_message_sources(&fields);
        let rendered = render_user_message(
            &template,
            &existing_user_message,
            1,
            "",
            "2026-06-03T15:00:00Z",
        )
        .expect("camel-case UserMessage should be accepted");

        assert_eq!(rendered, "Proof message from OData casing");
    }
}
