//! Seed Model — WASM module for the ProductModel.Seed integration.
//!
//! Builds a Product Model knowledge graph from external signals: GitHub REST
//! API (repo metadata, PRs, issues, commits), Datadog API (monitors, events),
//! and Temper AlertCycle history. Writes the assembled knowledge graph to
//! TemperFS and returns SeedComplete.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "seed_model: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // 1. Read ProductModel entity state
        let repo_url = fields
            .get("repo_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let _project_harness_id = fields
            .get("project_harness_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let signal_source_config = fields
            .get("signal_source_config")
            .cloned()
            .unwrap_or(json!({}));

        if repo_url.is_empty() {
            return Err("seed_model: repo_url is required".to_string());
        }

        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // 2. Read config
        let temper_api_url = ctx
            .config
            .get("temper_api_url")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());
        let github_token = ctx
            .config
            .get("github_token")
            .cloned()
            .unwrap_or_default();
        let dd_api_key = ctx
            .config
            .get("dd_api_key")
            .cloned()
            .unwrap_or_default();
        let dd_app_key = ctx
            .config
            .get("dd_app_key")
            .cloned()
            .unwrap_or_default();
        let dd_site = ctx
            .config
            .get("dd_site")
            .cloned()
            .unwrap_or_else(|| "datadoghq.com".to_string());

        let tenant = &ctx.tenant;
        let temper_headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-tenant-id".to_string(), tenant.to_string()),
            ("x-temper-principal-kind".to_string(), "agent".to_string()),
            ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        // Parse owner/repo from repo_url
        let (owner, repo) = parse_owner_repo(repo_url)
            .ok_or_else(|| format!("seed_model: cannot parse owner/repo from {repo_url}"))?;

        // 3. Query GitHub REST API
        let mut gh_headers = vec![
            ("accept".to_string(), "application/vnd.github+json".to_string()),
            ("user-agent".to_string(), "openpaw-seed-model".to_string()),
        ];
        if !github_token.is_empty() {
            gh_headers.push(("authorization".to_string(), format!("Bearer {github_token}")));
        }

        // GET /repos/{owner}/{repo}
        let repo_meta_url = format!("https://api.github.com/repos/{owner}/{repo}");
        let repo_meta_resp = ctx.http_call("GET", &repo_meta_url, &gh_headers, "")?;
        let repo_meta: Value = if repo_meta_resp.status >= 200 && repo_meta_resp.status < 300 {
            serde_json::from_str(&repo_meta_resp.body).unwrap_or(json!({}))
        } else {
            ctx.log(
                "warn",
                &format!(
                    "seed_model: failed to fetch repo metadata (HTTP {})",
                    repo_meta_resp.status
                ),
            );
            json!({})
        };

        // GET /repos/{owner}/{repo}/pulls?state=all&per_page=20
        let prs_url = format!(
            "https://api.github.com/repos/{owner}/{repo}/pulls?state=all&per_page=20"
        );
        let prs_resp = ctx.http_call("GET", &prs_url, &gh_headers, "")?;
        let prs: Value = if prs_resp.status >= 200 && prs_resp.status < 300 {
            serde_json::from_str(&prs_resp.body).unwrap_or(json!([]))
        } else {
            ctx.log("warn", &format!("seed_model: failed to fetch PRs (HTTP {})", prs_resp.status));
            json!([])
        };

        // GET /repos/{owner}/{repo}/issues?state=open&per_page=20
        let issues_url = format!(
            "https://api.github.com/repos/{owner}/{repo}/issues?state=open&per_page=20"
        );
        let issues_resp = ctx.http_call("GET", &issues_url, &gh_headers, "")?;
        let issues: Value = if issues_resp.status >= 200 && issues_resp.status < 300 {
            serde_json::from_str(&issues_resp.body).unwrap_or(json!([]))
        } else {
            ctx.log(
                "warn",
                &format!("seed_model: failed to fetch issues (HTTP {})", issues_resp.status),
            );
            json!([])
        };

        // GET /repos/{owner}/{repo}/commits?per_page=20
        let commits_url = format!(
            "https://api.github.com/repos/{owner}/{repo}/commits?per_page=20"
        );
        let commits_resp = ctx.http_call("GET", &commits_url, &gh_headers, "")?;
        let commits: Value = if commits_resp.status >= 200 && commits_resp.status < 300 {
            serde_json::from_str(&commits_resp.body).unwrap_or(json!([]))
        } else {
            ctx.log(
                "warn",
                &format!("seed_model: failed to fetch commits (HTTP {})", commits_resp.status),
            );
            json!([])
        };

        // Compute change hotspots from commits (files changed most frequently)
        let change_hotspots = compute_change_hotspots(&commits);

        // Query codebase structure: top-level directory, key subdirs, languages
        let root_contents_url = format!(
            "https://api.github.com/repos/{owner}/{repo}/contents/"
        );
        let root_contents_resp = ctx.http_call("GET", &root_contents_url, &gh_headers, "")?;
        let root_contents: Value = if root_contents_resp.status >= 200 && root_contents_resp.status < 300 {
            let raw: Value = serde_json::from_str(&root_contents_resp.body).unwrap_or(json!([]));
            summarize_directory_listing(&raw)
        } else {
            ctx.log(
                "warn",
                &format!(
                    "seed_model: failed to fetch root contents (HTTP {})",
                    root_contents_resp.status
                ),
            );
            json!([])
        };

        // Try key subdirectories: src, platform, crates, packages
        let subdir_names = ["src", "platform", "crates", "packages", "lib", "apps"];
        let mut subdirectories = json!({});
        for subdir in &subdir_names {
            let subdir_url = format!(
                "https://api.github.com/repos/{owner}/{repo}/contents/{subdir}"
            );
            let subdir_resp = ctx.http_call("GET", &subdir_url, &gh_headers, "")?;
            if subdir_resp.status >= 200 && subdir_resp.status < 300 {
                let raw: Value = serde_json::from_str(&subdir_resp.body).unwrap_or(json!([]));
                subdirectories[*subdir] = summarize_directory_listing(&raw);
            }
            // Silently skip 404s — the subdir may not exist
        }

        // GET /repos/{owner}/{repo}/languages
        let languages_url = format!(
            "https://api.github.com/repos/{owner}/{repo}/languages"
        );
        let languages_resp = ctx.http_call("GET", &languages_url, &gh_headers, "")?;
        let languages: Value = if languages_resp.status >= 200 && languages_resp.status < 300 {
            serde_json::from_str(&languages_resp.body).unwrap_or(json!({}))
        } else {
            ctx.log(
                "warn",
                &format!(
                    "seed_model: failed to fetch languages (HTTP {})",
                    languages_resp.status
                ),
            );
            json!({})
        };

        ctx.log("info", "seed_model: GitHub signals collected");

        // 4. Query Datadog API
        let dd_headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("dd-api-key".to_string(), dd_api_key.clone()),
            ("dd-application-key".to_string(), dd_app_key.clone()),
        ];

        let mut dd_monitors = json!([]);
        let mut dd_events = json!([]);

        if !dd_api_key.is_empty() && !dd_app_key.is_empty() {
            // GET /api/v1/monitor
            let monitors_url = format!("https://api.{dd_site}/api/v1/monitor");
            let monitors_resp = ctx.http_call("GET", &monitors_url, &dd_headers, "")?;
            if monitors_resp.status >= 200 && monitors_resp.status < 300 {
                dd_monitors = serde_json::from_str(&monitors_resp.body).unwrap_or(json!([]));
            } else {
                ctx.log(
                    "warn",
                    &format!(
                        "seed_model: failed to fetch Datadog monitors (HTTP {})",
                        monitors_resp.status
                    ),
                );
            }

            // GET /api/v1/events
            let events_url = format!("https://api.{dd_site}/api/v1/events");
            let events_resp = ctx.http_call("GET", &events_url, &dd_headers, "")?;
            if events_resp.status >= 200 && events_resp.status < 300 {
                dd_events = serde_json::from_str(&events_resp.body).unwrap_or(json!([]));
            } else {
                ctx.log(
                    "warn",
                    &format!(
                        "seed_model: failed to fetch Datadog events (HTTP {})",
                        events_resp.status
                    ),
                );
            }
        } else {
            ctx.log("info", "seed_model: Datadog keys not configured, skipping");
        }

        ctx.log("info", "seed_model: Datadog signals collected");

        // 5. Query AlertCycle history from Temper
        let alert_cycles_url = format!(
            "{temper_api_url}/tdata/AlertCycles?$top=10&$orderby=created_at desc"
        );
        let alert_cycles_resp = ctx.http_call("GET", &alert_cycles_url, &temper_headers, "")?;
        let alert_cycles: Value = if alert_cycles_resp.status >= 200 && alert_cycles_resp.status < 300 {
            let body: Value =
                serde_json::from_str(&alert_cycles_resp.body).unwrap_or(json!({}));
            body.get("value").cloned().unwrap_or(json!([]))
        } else {
            ctx.log(
                "warn",
                &format!(
                    "seed_model: failed to fetch AlertCycles (HTTP {})",
                    alert_cycles_resp.status
                ),
            );
            json!([])
        };

        ctx.log("info", "seed_model: AlertCycle history collected");

        // 6. Build knowledge graph
        let default_branch = repo_meta
            .get("default_branch")
            .and_then(|v| v.as_str())
            .unwrap_or("main");
        let tech_stack = extract_tech_stack(&repo_meta, &signal_source_config);

        let knowledge_graph = json!({
            "repo": {
                "url": repo_url,
                "tech_stack": tech_stack,
                "default_branch": default_branch
            },
            "codebase": {
                "directories": root_contents,
                "subdirectories": subdirectories,
                "languages": languages
            },
            "modules": extract_modules(&signal_source_config),
            "dependencies": extract_dependencies(&signal_source_config),
            "signals": {
                "github": {
                    "prs": summarize_prs(&prs),
                    "issues": summarize_issues(&issues),
                    "commits": summarize_commits(&commits),
                    "change_hotspots": change_hotspots
                },
                "datadog": {
                    "monitors": summarize_monitors(&dd_monitors),
                    "recent_alerts": extract_recent_alerts(&dd_events),
                    "trends": extract_dd_trends(&dd_monitors)
                },
                "alert_cycles": alert_cycles
            },
            "trends": build_trend_summary(&dd_monitors, &dd_events),
            "seeded_at": get_timestamp(&ctx)
        });

        let signal_count = count_signals(&prs, &issues, &commits, &dd_monitors, &dd_events, &alert_cycles);

        ctx.log(
            "info",
            &format!("seed_model: knowledge graph built, {signal_count} signals"),
        );

        // 7. Write to TemperFS: POST /tdata/Files, then PUT content
        let file_create_url = format!("{temper_api_url}/tdata/Files");
        let file_name = format!("product_model_{entity_id}.json");
        let file_create_body = json!({
            "name": file_name,
            "content_type": "application/json",
            "description": format!("ProductModel knowledge graph for {entity_id}")
        });
        let file_create_resp = ctx.http_call(
            "POST",
            &file_create_url,
            &temper_headers,
            &file_create_body.to_string(),
        )?;

        let file_id = if file_create_resp.status >= 200 && file_create_resp.status < 300 {
            let file_parsed: Value =
                serde_json::from_str(&file_create_resp.body).unwrap_or(json!({}));
            file_parsed
                .get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string()
        } else {
            ctx.log(
                "warn",
                &format!(
                    "seed_model: failed to create File entity (HTTP {})",
                    file_create_resp.status
                ),
            );
            "unknown".to_string()
        };

        if file_id != "unknown" {
            let file_content_url =
                format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
            let content_headers = vec![
                ("content-type".to_string(), "application/json".to_string()),
                ("x-tenant-id".to_string(), tenant.to_string()),
                ("x-temper-principal-kind".to_string(), "agent".to_string()),
                ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
                ("x-temper-agent-type".to_string(), "system".to_string()),
            ];
            let put_resp = ctx.http_call(
                "PUT",
                &file_content_url,
                &content_headers,
                &knowledge_graph.to_string(),
            )?;
            if put_resp.status < 200 || put_resp.status >= 300 {
                ctx.log(
                    "warn",
                    &format!(
                        "seed_model: failed to PUT file content (HTTP {})",
                        put_resp.status
                    ),
                );
            }
        }

        ctx.log(
            "info",
            &format!("seed_model: knowledge graph written to file {file_id}"),
        );

        // 8. Dispatch SeedComplete action via OData API
        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let seed_complete_url = format!(
            "{temper_api_url}/tdata/ProductModels('{entity_id}')/OpenPaw.Foresight.SeedComplete"
        );
        let seed_complete_body = json!({
            "model_snapshot_file_id": file_id,
            "signal_count": signal_count.to_string(),
            "last_signal_refresh": get_timestamp(&ctx)
        });
        let callback_headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-tenant-id".to_string(), ctx.tenant.clone()),
            ("x-temper-principal-kind".to_string(), "agent".to_string()),
            ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];
        let resp = ctx.http_call(
            "POST",
            &seed_complete_url,
            &callback_headers,
            &seed_complete_body.to_string(),
        )?;
        if resp.status >= 200 && resp.status < 300 {
            ctx.log("info", "seed_model: SeedComplete dispatched");
        } else {
            ctx.log(
                "warn",
                &format!(
                    "seed_model: SeedComplete dispatch failed ({}): {}",
                    resp.status, resp.body
                ),
            );
        }

        set_success_result("", &json!({}));
        ctx.log("info", "seed_model: done");
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Parse "owner/repo" from a GitHub URL.
fn parse_owner_repo(url: &str) -> Option<(String, String)> {
    // Handle https://github.com/owner/repo, git@github.com:owner/repo, or owner/repo
    let stripped = url
        .trim_end_matches('/')
        .trim_end_matches(".git");
    if let Some(rest) = stripped.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = rest.splitn(2, '/').collect();
        if parts.len() == 2 {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    if let Some(rest) = stripped.strip_prefix("git@github.com:") {
        let parts: Vec<&str> = rest.splitn(2, '/').collect();
        if parts.len() == 2 {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    // Try bare owner/repo
    let parts: Vec<&str> = stripped.splitn(2, '/').collect();
    if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        return Some((parts[0].to_string(), parts[1].to_string()));
    }
    None
}

/// Compute change hotspots from commit data.
fn compute_change_hotspots(commits: &Value) -> Value {
    // Hotspots are derived from commit messages referencing files/paths
    // In a real implementation, this would use the commits API with file stats
    let count = commits.as_array().map(|a| a.len()).unwrap_or(0);
    json!({ "commit_count": count, "note": "derived from recent commit activity" })
}

/// Extract tech stack from repo metadata and signal source config.
fn extract_tech_stack(repo_meta: &Value, config: &Value) -> Value {
    let language = repo_meta
        .get("language")
        .cloned()
        .unwrap_or(json!(null));
    let topics = repo_meta
        .get("topics")
        .cloned()
        .unwrap_or(json!([]));
    let configured = config
        .get("tech_stack")
        .cloned()
        .unwrap_or(json!(null));
    json!({
        "primary_language": language,
        "topics": topics,
        "configured": configured
    })
}

/// Extract modules from signal source config.
fn extract_modules(config: &Value) -> Value {
    config.get("modules").cloned().unwrap_or(json!([]))
}

/// Extract dependencies from signal source config.
fn extract_dependencies(config: &Value) -> Value {
    config.get("dependencies").cloned().unwrap_or(json!([]))
}

/// Summarize PRs into compact form.
fn summarize_prs(prs: &Value) -> Value {
    let items = prs.as_array().map(|arr| {
        arr.iter()
            .map(|pr| {
                json!({
                    "number": pr.get("number"),
                    "title": pr.get("title"),
                    "state": pr.get("state"),
                    "created_at": pr.get("created_at"),
                    "merged_at": pr.get("merged_at"),
                    "user": pr.get("user").and_then(|u| u.get("login"))
                })
            })
            .collect::<Vec<_>>()
    });
    json!(items.unwrap_or_default())
}

/// Summarize issues into compact form.
fn summarize_issues(issues: &Value) -> Value {
    let items = issues.as_array().map(|arr| {
        arr.iter()
            .filter(|i| i.get("pull_request").is_none()) // Exclude PRs from issues endpoint
            .map(|issue| {
                json!({
                    "number": issue.get("number"),
                    "title": issue.get("title"),
                    "state": issue.get("state"),
                    "created_at": issue.get("created_at"),
                    "labels": issue.get("labels")
                })
            })
            .collect::<Vec<_>>()
    });
    json!(items.unwrap_or_default())
}

/// Summarize commits into compact form.
fn summarize_commits(commits: &Value) -> Value {
    let items = commits.as_array().map(|arr| {
        arr.iter()
            .map(|c| {
                json!({
                    "sha": c.get("sha").and_then(|v| v.as_str()).map(|s| &s[..7.min(s.len())]),
                    "message": c.get("commit").and_then(|cm| cm.get("message")),
                    "date": c.get("commit").and_then(|cm| cm.get("author")).and_then(|a| a.get("date")),
                    "author": c.get("commit").and_then(|cm| cm.get("author")).and_then(|a| a.get("name"))
                })
            })
            .collect::<Vec<_>>()
    });
    json!(items.unwrap_or_default())
}

/// Summarize Datadog monitors.
fn summarize_monitors(monitors: &Value) -> Value {
    let items = monitors.as_array().map(|arr| {
        arr.iter()
            .map(|m| {
                json!({
                    "id": m.get("id"),
                    "name": m.get("name"),
                    "type": m.get("type"),
                    "overall_state": m.get("overall_state"),
                    "query": m.get("query")
                })
            })
            .collect::<Vec<_>>()
    });
    json!(items.unwrap_or_default())
}

/// Extract recent alerts from Datadog events.
fn extract_recent_alerts(events: &Value) -> Value {
    let event_list = events
        .get("events")
        .cloned()
        .unwrap_or_else(|| events.clone());
    let items = event_list.as_array().map(|arr| {
        arr.iter()
            .filter(|e| {
                e.get("alert_type")
                    .and_then(|v| v.as_str())
                    .map(|t| t == "error" || t == "warning")
                    .unwrap_or(false)
            })
            .map(|e| {
                json!({
                    "title": e.get("title"),
                    "date_happened": e.get("date_happened"),
                    "alert_type": e.get("alert_type"),
                    "source": e.get("source_type_name")
                })
            })
            .collect::<Vec<_>>()
    });
    json!(items.unwrap_or_default())
}

/// Extract trend indicators from Datadog monitors.
fn extract_dd_trends(monitors: &Value) -> Value {
    let mut alert_count = 0u64;
    let mut warn_count = 0u64;
    let mut ok_count = 0u64;
    if let Some(arr) = monitors.as_array() {
        for m in arr {
            match m.get("overall_state").and_then(|v| v.as_str()) {
                Some("Alert") => alert_count += 1,
                Some("Warn") => warn_count += 1,
                Some("OK") => ok_count += 1,
                _ => {}
            }
        }
    }
    json!({
        "monitors_in_alert": alert_count,
        "monitors_in_warn": warn_count,
        "monitors_ok": ok_count
    })
}

/// Build a high-level trend summary.
fn build_trend_summary(monitors: &Value, _events: &Value) -> Value {
    let dd_trends = extract_dd_trends(monitors);
    json!({
        "error_rate": { "current": null, "weekly_delta": null, "note": "populate from metrics query" },
        "latency": { "current": null, "weekly_delta": null, "note": "populate from metrics query" },
        "monitor_health": dd_trends
    })
}

/// Count total signals collected.
fn count_signals(
    prs: &Value,
    issues: &Value,
    commits: &Value,
    monitors: &Value,
    events: &Value,
    alert_cycles: &Value,
) -> usize {
    let pr_count = prs.as_array().map(|a| a.len()).unwrap_or(0);
    let issue_count = issues.as_array().map(|a| a.len()).unwrap_or(0);
    let commit_count = commits.as_array().map(|a| a.len()).unwrap_or(0);
    let monitor_count = monitors.as_array().map(|a| a.len()).unwrap_or(0);
    let event_list = events.get("events").unwrap_or(events);
    let event_count = event_list.as_array().map(|a| a.len()).unwrap_or(0);
    let alert_count = alert_cycles.as_array().map(|a| a.len()).unwrap_or(0);
    pr_count + issue_count + commit_count + monitor_count + event_count + alert_count
}

/// Summarize a GitHub Contents API directory listing into compact form.
fn summarize_directory_listing(contents: &Value) -> Value {
    let items = contents.as_array().map(|arr| {
        arr.iter()
            .map(|entry| {
                json!({
                    "name": entry.get("name"),
                    "type": entry.get("type"),
                    "size": entry.get("size")
                })
            })
            .collect::<Vec<_>>()
    });
    json!(items.unwrap_or_default())
}

/// Get timestamp from trigger params or entity state, falling back to "now".
/// WASM has no std::time clock, so we rely on the caller to pass a timestamp
/// via the action params or entity state fields.
fn get_timestamp(ctx: &Context) -> String {
    // Try trigger params first (set by the caller/script)
    if let Some(ts) = ctx.trigger_params.get("last_signal_refresh").and_then(|v| v.as_str()) {
        if !ts.is_empty() {
            return ts.to_string();
        }
    }
    // Fall back to existing entity state
    if let Some(fields) = ctx.entity_state.get("fields") {
        if let Some(ts) = fields.get("last_signal_refresh").and_then(|v| v.as_str()) {
            if !ts.is_empty() {
                return ts.to_string();
            }
        }
    }
    // Last resort — the platform will update this on next action dispatch
    "pending".to_string()
}
