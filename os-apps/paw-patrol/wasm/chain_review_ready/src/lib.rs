//! chain_review_ready — one concern: attached ReviewRuns pass validate.py.
//!
//! Fired by Effort.PassReview. GETs each id in review_run_ids. On any miss,
//! set_error_result so on_failure retracts review_passed.
//!
//! Does not dispatch. Does not write rows.

use std::collections::BTreeSet;

use temper_wasm_sdk::prelude::*;

const MODEL_REVIEWERS: &[&str] = &["grok", "codex", "fable"];
const MIN_MODELS: usize = 2;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let ids = id_list(&fields, "review_run_ids");
        if ids.is_empty() {
            return Err("chain_review_ready: review_run_ids is empty".to_string());
        }
        let base_url = resolve_api_url(&ctx);
        let headers = odata_headers(&ctx);
        let mut runs = Vec::new();
        for id in &ids {
            runs.push(get_entity(&ctx, &base_url, &headers, "ReviewRuns", id)?);
        }
        review_panel_holds(&runs, None)?;
        ctx.log(
            "info",
            &format!("chain_review_ready: {} ReviewRuns hold", runs.len()),
        );
        set_success_result("", &json!({ "status": "review_ready", "runs": runs.len() }));
        Ok(())
    })();
    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}

pub fn review_panel_holds(runs: &[Value], require_commit: Option<&str>) -> Result<(), String> {
    if runs.is_empty() {
        return Err("chain_review_ready: no ReviewRuns".to_string());
    }
    let mut commits = BTreeSet::new();
    let mut reviewers = BTreeSet::new();
    let mut recorded = 0usize;
    for (i, run) in runs.iter().enumerate() {
        let fields = run.get("fields").unwrap_or(run);
        let status = str_of(fields, "status").or_else(|| str_of(fields, "Status"));
        if status.as_deref() != Some("Recorded") {
            return Err(format!(
                "chain_review_ready: ReviewRun[{i}] status {status:?} is not Recorded"
            ));
        }
        if !bool_of(fields, "record_present") {
            return Err(format!(
                "chain_review_ready: ReviewRun[{i}] record_present is false"
            ));
        }
        if bool_of(fields, "fix_it_failed") {
            return Err(format!(
                "chain_review_ready: ReviewRun[{i}] fix-it rubrics failed"
            ));
        }
        let findings = json_field(fields, "findings")?;
        if open_act_on_count(&findings) > 0 {
            return Err(format!(
                "chain_review_ready: ReviewRun[{i}] has an open act-on finding"
            ));
        }
        let commit = str_of(fields, "commit")
            .or_else(|| str_of(fields, "Commit"))
            .unwrap_or_default();
        if !is_full_sha(&commit) {
            return Err(format!(
                "chain_review_ready: ReviewRun[{i}] commit is not a 40-char sha"
            ));
        }
        commits.insert(commit);
        if let Some(reviewer) = str_of(fields, "reviewer_id").or_else(|| str_of(fields, "ReviewerId"))
        {
            if !reviewer.is_empty() {
                reviewers.insert(reviewer);
            }
        }
        for reviewer in string_list(fields, "reviewers_ran") {
            reviewers.insert(reviewer);
        }
        recorded += 1;
    }
    if commits.len() != 1 {
        return Err("chain_review_ready: ReviewRuns do not share one commit".to_string());
    }
    let commit = commits.iter().next().expect("one commit");
    if let Some(required) = require_commit
        && required != commit
    {
        return Err(format!(
            "chain_review_ready: commit {commit} != required {required}"
        ));
    }
    let models = MODEL_REVIEWERS
        .iter()
        .filter(|m| reviewers.iter().any(|r| r == *m))
        .count();
    if models < MIN_MODELS {
        return Err(format!(
            "chain_review_ready: {models}/{} model reviewers ran ({:?})",
            MODEL_REVIEWERS.len(),
            reviewers
        ));
    }
    let _ = recorded;
    Ok(())
}

fn open_act_on_count(findings: &Value) -> usize {
    findings
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|f| {
                    f.get("severity").and_then(|s| s.as_str()) == Some("act-on")
                        && f.get("resolved").and_then(|r| r.as_bool()) != Some(true)
                })
                .count()
        })
        .unwrap_or(0)
}

fn is_full_sha(commit: &str) -> bool {
    commit.len() == 40 && commit.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn json_field(fields: &Value, name: &str) -> Result<Value, String> {
    let pascal = pascal(name);
    let raw = fields
        .get(name)
        .or_else(|| fields.get(&pascal))
        .cloned()
        .unwrap_or(json!([]));
    match raw {
        Value::String(s) => serde_json::from_str(&s)
            .map_err(|e| format!("chain_review_ready: {name} is not JSON: {e}")),
        other => Ok(other),
    }
}

fn string_list(fields: &Value, name: &str) -> Vec<String> {
    json_field(fields, name)
        .ok()
        .and_then(|v| match v {
            Value::Array(items) => Some(
                items
                    .into_iter()
                    .filter_map(|i| i.as_str().map(str::to_string))
                    .collect(),
            ),
            Value::String(s) if !s.is_empty() => Some(vec![s]),
            _ => None,
        })
        .unwrap_or_default()
}

pub fn id_list(fields: &Value, name: &str) -> Vec<String> {
    string_list(fields, name)
}

fn str_of(fields: &Value, name: &str) -> Option<String> {
    fields
        .get(name)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn bool_of(fields: &Value, name: &str) -> bool {
    let pascal = pascal(name);
    fields
        .get(name)
        .or_else(|| fields.get(&pascal))
        .and_then(|v| v.as_bool())
        == Some(true)
}

fn pascal(field: &str) -> String {
    field
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
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

fn get_entity(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    set: &str,
    id: &str,
) -> Result<Value, String> {
    if id.is_empty() || id.contains('\'') || id.contains('/') {
        return Err(format!("chain_review_ready: bad {set} id"));
    }
    let url = format!("{}/tdata/{set}('{id}')", base_url.trim_end_matches('/'));
    let resp = ctx.http_call("GET", &url, headers, "")?;
    if resp.status >= 400 {
        return Err(format!(
            "chain_review_ready: GET {set} {id} HTTP {}",
            resp.status
        ));
    }
    serde_json::from_str(&resp.body)
        .map_err(|e| format!("chain_review_ready: {set} {id} body: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review_run(reviewer: &str, commit: &str, open_act_on: bool, fix_it: bool) -> Value {
        let findings = if open_act_on {
            json!([{"severity":"act-on","file_line":"a.rs:1","resolved":false}])
        } else {
            json!([])
        };
        json!({
            "fields": {
                "status": "Recorded",
                "record_present": true,
                "fix_it_failed": fix_it,
                "commit": commit,
                "reviewer_id": reviewer,
                "findings": findings.to_string(),
            }
        })
    }

    const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn panel_of_three_models_holds() {
        let runs = vec![
            review_run("grok", SHA, false, false),
            review_run("codex", SHA, false, false),
            review_run("fable", SHA, false, false),
        ];
        assert!(review_panel_holds(&runs, Some(SHA)).is_ok());
    }

    #[test]
    fn two_models_hold_one_may_skip() {
        let runs = vec![
            review_run("grok", SHA, false, false),
            review_run("codex", SHA, false, false),
        ];
        assert!(review_panel_holds(&runs, None).is_ok());
    }

    #[test]
    fn one_model_fails() {
        let runs = vec![review_run("grok", SHA, false, false)];
        assert!(review_panel_holds(&runs, None).unwrap_err().contains("1/"));
    }

    #[test]
    fn open_act_on_fails() {
        let runs = vec![
            review_run("grok", SHA, true, false),
            review_run("codex", SHA, false, false),
        ];
        assert!(
            review_panel_holds(&runs, None)
                .unwrap_err()
                .contains("open act-on")
        );
    }

    #[test]
    fn fix_it_fails() {
        let runs = vec![
            review_run("grok", SHA, false, true),
            review_run("codex", SHA, false, false),
        ];
        assert!(
            review_panel_holds(&runs, None)
                .unwrap_err()
                .contains("fix-it")
        );
    }

    #[test]
    fn commit_pin_fails() {
        let runs = vec![
            review_run("grok", SHA, false, false),
            review_run("codex", SHA, false, false),
        ];
        let other = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        assert!(
            review_panel_holds(&runs, Some(other))
                .unwrap_err()
                .contains("!=")
        );
    }

    #[test]
    fn mismatched_commits_fail() {
        let other = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let runs = vec![
            review_run("grok", SHA, false, false),
            review_run("codex", other, false, false),
        ];
        assert!(
            review_panel_holds(&runs, None)
                .unwrap_err()
                .contains("share one commit")
        );
    }

    #[test]
    fn not_recorded_fails() {
        let mut first = review_run("grok", SHA, false, false);
        first["fields"]["status"] = json!("Requested");
        assert!(
            review_panel_holds(&[first, review_run("codex", SHA, false, false)], None)
                .unwrap_err()
                .contains("not Recorded")
        );
    }
}
