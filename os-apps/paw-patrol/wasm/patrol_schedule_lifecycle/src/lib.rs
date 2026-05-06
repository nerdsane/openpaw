//! Patrol Schedule Lifecycle - recurring sweeps and briefs.
//!
//! Triggered by `PatrolSchedule.Activate`, `PatrolSchedule.Resume`, and
//! `PatrolSchedule.Trigger`. The schedule is Temper-native: `schedule_at`
//! re-dispatches `Trigger`; this WASM only computes the next time and creates
//! the existing Patrol entities (`RepoGraphSnapshot` and `DailyBrief`).

use temper_wasm_sdk::prelude::*;

const REPO_GRAPH_SNAPSHOTS_PATH: &str = "/tdata/RepoGraphSnapshots";
const DAILY_BRIEFS_PATH: &str = "/tdata/DailyBriefs";
const PATROL_START_SCAN: &str = "TemperPaw.Patrol.StartScan";
const PATROL_START_DAILY_BRIEF: &str = "TemperPaw.Patrol.Start";

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let base_url = resolve_api_url(&ctx);
        let headers = odata_headers(&ctx);
        let mode = ctx.config.get("mode").map(String::as_str).unwrap_or("activate");

        match mode {
            "trigger" => handle_trigger(&ctx, &base_url, &headers, &fields),
            _ => handle_activate(&ctx, &fields),
        }?;

        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}

fn handle_activate(ctx: &Context, fields: &Value) -> Result<(), String> {
    let cadence = string_from_fields(fields, "cadence", "Cadence");
    let now_secs = now_secs();
    let interval = parse_patrol_interval(&cadence)?;
    let next_run_at = unix_to_iso8601(now_secs + interval);
    let summary = format!(
        "PatrolSchedule {} activated; next run at {next_run_at}.",
        entity_id(ctx)
    );

    set_success_result(
        "ActivateComplete",
        &json!({
            "next_run_at": next_run_at,
            "last_summary": summary
        }),
    );
    Ok(())
}

fn handle_trigger(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
) -> Result<(), String> {
    let now_secs = now_secs();
    let now_label = unix_to_iso8601(now_secs);
    let cadence = string_from_fields(fields, "cadence", "Cadence");
    let interval = parse_patrol_interval(&cadence)?;
    let next_run_at = unix_to_iso8601(now_secs + interval);
    let run_count = counter_from_fields(fields, "run_count", "RunCount");
    let max_runs = string_from_fields(fields, "max_runs", "MaxRuns")
        .parse::<u64>()
        .unwrap_or(0);

    if max_runs > 0 && run_count > max_runs {
        set_success_result("Expire", &json!({}));
        return Ok(());
    }

    let mut repo_graph_snapshot_id = String::new();
    let mut daily_brief_id = String::new();

    if bool_from_fields(fields, "enable_repo_sweep", "EnableRepoSweep", true) {
        repo_graph_snapshot_id = create_entity(ctx, base_url, headers, REPO_GRAPH_SNAPSHOTS_PATH)?;
        let commit_sha = nonempty_or(
            &string_from_fields(fields, "commit_sha", "CommitSha"),
            "scheduled-current-checkout",
        );
        post_action(
            ctx,
            base_url,
            headers,
            "RepoGraphSnapshots",
            &repo_graph_snapshot_id,
            PATROL_START_SCAN,
            &json!({
                "commit_sha": format!("{commit_sha} schedule-run-{run_count}")
            }),
        )?;
    }

    if bool_from_fields(fields, "enable_daily_brief", "EnableDailyBrief", true) {
        daily_brief_id = create_entity(ctx, base_url, headers, DAILY_BRIEFS_PATH)?;
        post_action(
            ctx,
            base_url,
            headers,
            "DailyBriefs",
            &daily_brief_id,
            PATROL_START_DAILY_BRIEF,
            &json!({
                "brief_date": date_from_iso8601(&now_label)
            }),
        )?;
    }

    let summary = format!(
        "PatrolSchedule {} ran at {now_label}; repo_graph_snapshot_id={}; daily_brief_id={}; next_run_at={next_run_at}.",
        entity_id(ctx),
        empty_label(&repo_graph_snapshot_id),
        empty_label(&daily_brief_id),
    );

    set_success_result(
        "TriggerComplete",
        &json!({
            "next_run_at": next_run_at,
            "last_run_at": now_label,
            "last_repo_graph_snapshot_id": repo_graph_snapshot_id,
            "last_daily_brief_id": daily_brief_id,
            "last_summary": summary
        }),
    );
    Ok(())
}

fn parse_patrol_interval(cadence: &str) -> Result<u64, String> {
    let cadence = cadence.trim();
    if cadence.is_empty() || cadence.eq_ignore_ascii_case("daily") {
        return Ok(86_400);
    }
    if cadence.eq_ignore_ascii_case("hourly") {
        return Ok(3_600);
    }
    if cadence.eq_ignore_ascii_case("weekly") {
        return Ok(604_800);
    }
    if let Some(minutes) = cadence
        .strip_suffix('m')
        .and_then(|value| value.parse::<u64>().ok())
    {
        return Ok(minutes.max(1) * 60);
    }
    if let Some(hours) = cadence
        .strip_suffix('h')
        .and_then(|value| value.parse::<u64>().ok())
    {
        return Ok(hours.max(1) * 3_600);
    }

    let parts: Vec<&str> = cadence.split_whitespace().collect();
    if parts.len() == 5 {
        let minute = parts[0];
        let hour = parts[1];
        if let Some(n) = minute.strip_prefix("*/") {
            if let Ok(n) = n.parse::<u64>() {
                return Ok(n.max(1) * 60);
            }
        }
        if minute == "*" && hour == "*" {
            return Ok(60);
        }
        if minute == "0" {
            if let Some(n) = hour.strip_prefix("*/") {
                if let Ok(n) = n.parse::<u64>() {
                    return Ok(n.max(1) * 3_600);
                }
            }
            if hour == "*" {
                return Ok(3_600);
            }
            if hour == "0" {
                return Ok(86_400);
            }
        }
    }

    Err(format!(
        "Unsupported PatrolSchedule cadence '{cadence}'. Use daily, hourly, weekly, 30m, 6h, or a simple cron interval."
    ))
}

fn now_secs() -> u64 {
    (Context::get_time_millis() / 1000) as u64
}

fn create_entity(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    path: &str,
) -> Result<String, String> {
    let url = format!("{base_url}{path}");
    let entity_set = path.rsplit('/').next().unwrap_or(path);
    let resp = ctx.http_call("POST", &url, headers, "{}")?;
    let body = parse_json_response(resp, &format!("create {entity_set}"))?;
    entity_id_from_response(&body).ok_or_else(|| format!("create {entity_set}: missing entity_id"))
}

fn post_action(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    entity_set: &str,
    entity_id: &str,
    action_path: &str,
    body: &Value,
) -> Result<Value, String> {
    let url = format!("{base_url}/tdata/{entity_set}('{entity_id}')/{action_path}");
    let resp = ctx.http_call("POST", &url, headers, &body.to_string())?;
    parse_json_response(
        resp,
        &format!("{action_path} on {entity_set}('{entity_id}')"),
    )
}

fn parse_json_response(resp: HttpResponse, label: &str) -> Result<Value, String> {
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "{label} failed with HTTP {}: {}",
            resp.status,
            truncate(&resp.body, 500)
        ));
    }
    if resp.body.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&resp.body).map_err(|err| format!("{label}: parse response: {err}"))
}

fn entity_id_from_response(value: &Value) -> Option<String> {
    value
        .get("entity_id")
        .or_else(|| value.get("id"))
        .or_else(|| value.get("Id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn string_from_fields(fields: &Value, snake: &str, pascal: &str) -> String {
    fields
        .get(snake)
        .and_then(Value::as_str)
        .or_else(|| fields.get(pascal).and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

fn bool_from_fields(fields: &Value, snake: &str, pascal: &str, default: bool) -> bool {
    fields
        .get(snake)
        .or_else(|| fields.get(pascal))
        .and_then(|value| {
            value.as_bool().or_else(|| {
                value
                    .as_str()
                    .map(|value| value.eq_ignore_ascii_case("true"))
            })
        })
        .unwrap_or(default)
}

fn counter_from_fields(fields: &Value, snake: &str, pascal: &str) -> u64 {
    fields
        .get(snake)
        .or_else(|| fields.get(pascal))
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        })
        .unwrap_or(0)
}

fn nonempty_or(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn empty_label(value: &str) -> &str {
    if value.is_empty() { "none" } else { value }
}

fn date_from_iso8601(value: &str) -> String {
    value.split('T').next().unwrap_or(value).to_string()
}

fn resolve_api_url(ctx: &Context) -> String {
    ctx.config
        .get("temper_api_url")
        .filter(|value| !value.is_empty() && !value.contains("{secret:"))
        .cloned()
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string())
}

fn odata_headers(ctx: &Context) -> Vec<(String, String)> {
    vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("x-tenant-id".to_string(), ctx.tenant.clone()),
        ("x-temper-principal-kind".to_string(), "agent".to_string()),
        ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
        ("x-temper-agent-type".to_string(), "system".to_string()),
    ]
}

fn entity_id(ctx: &Context) -> String {
    if ctx.entity_id.trim().is_empty() {
        "unknown".to_string()
    } else {
        ctx.entity_id.clone()
    }
}

fn unix_to_iso8601(secs: u64) -> String {
    let mut days = (secs / 86_400) as i64;
    let day_secs = secs % 86_400;
    let hour = day_secs / 3_600;
    let minute = (day_secs % 3_600) / 60;
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
    for (index, month_days) in mdays.iter().enumerate() {
        if days < *month_days as i64 {
            month = index;
            break;
        }
        days -= *month_days as i64;
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

fn truncate(input: &str, max: usize) -> String {
    if input.len() <= max {
        input.to_string()
    } else {
        format!("{}[truncated]", input.chars().take(max).collect::<String>())
    }
}
