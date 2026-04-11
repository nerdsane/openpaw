//! Dispatch `temper.*` and `sandbox.*` method calls to HTTP endpoints.
//!
//! When Monty code calls `temper.create("Issues", {...})`, the Monty
//! interpreter pauses and yields a `FunctionCall`. This module handles
//! that call by making an HTTP request via the WASM host function
//! `ctx.http_call()` and returning the result as JSON.
//!
//! Mirrors the method signatures from `temper-sandbox/src/dispatch.rs`
//! so agents see the exact same Python interface as `mcp__temper__execute`.

use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeSet};
use temper_wasm_sdk::context::Context;

const DEFAULT_TOOLS_ENABLED: &str = "temper_create,temper_get,temper_list,temper_action,temper_patch,temper_submit_specs,temper_show_spec,temper_specs,temper_upload_wasm,temper_get_trajectories,temper_get_insights,temper_get_decisions,temper_poll_decision,temper_approve_decision,temper_deny_decision,temper_submit_policy,temper_list_policies,temper_get_policy,temper_update_policy,temper_delete_policy,temper_install_app,temper_list_apps,temper_spawn_session,temper_list_sessions,temper_abort_session,temper_steer_session,temper_save_memory,temper_recall_memory,temper_write,temper_read,temper_run_coding_agent,temper_get_secret,temper_datadog_query,temper_railway,temper_vercel,temper_web_search,temper_web_fetch,read,write,edit,bash";

/// Tools available in plan mode (ADR-004). Blocks sandbox mutation (write, edit)
/// and governance writes. Allows read ops, research, memory, Plan CRUD, and
/// TemperFS writes (for plan documents).
pub const PLAN_MODE_TOOLS: &str = "temper_create,temper_get,temper_list,temper_action,temper_specs,temper_show_spec,temper_save_memory,temper_recall_memory,temper_read,temper_write,temper_web_search,temper_web_fetch,temper_get_trajectories,temper_get_insights,read,bash";

// Thread-local storage for the done signal. When an agent calls
// temper.done(result), the result is stored here. After all tool
// calls finish, lib.rs checks this and returns RecordResult instead
// of HandleToolResults, completing the session.
thread_local! {
    static DONE_RESULT: RefCell<Option<String>> = RefCell::new(None);
}

// Thread-local storage for lazily provisioned sandbox (ADR-0022).
// When a sandbox tool is called and no sandbox exists, we provision
// one on-demand and cache (sandbox_url, sandbox_id) here. lib.rs
// reads this after tool execution to persist via HandleToolResults.
thread_local! {
    static LAZY_SANDBOX: RefCell<Option<(String, String)>> = RefCell::new(None);
}

// Thread-local storage for Cedar denial. When dispatch detects a
// CEDAR_DENIED_CTX error, the full JSON context is stored here so
// lib.rs can read it after drive_repl_loop returns (even if Monty
// Python code catches the exception).
thread_local! {
    static CEDAR_DENIAL: RefCell<Option<String>> = RefCell::new(None);
    // Dispatch output: when a tool produces important output that the LLM
    // must see (even if the Python code assigns the result to a variable
    // instead of printing it), store it here. The REPL appends it to the
    // tool result if the expression was null/None.
    static DISPATCH_OUTPUT: RefCell<Option<String>> = RefCell::new(None);
}

/// Take the done result (if set). Clears it after reading.
pub fn take_done_result() -> Option<String> {
    DONE_RESULT.with(|cell| cell.borrow_mut().take())
}

/// Take the lazily provisioned sandbox details (if set). Clears after reading.
pub fn take_lazy_sandbox() -> Option<(String, String)> {
    LAZY_SANDBOX.with(|cell| cell.borrow_mut().take())
}

/// Take the Cedar denial context (if set). Clears after reading.
pub fn take_cedar_denial() -> Option<String> {
    CEDAR_DENIAL.with(|cell| cell.borrow_mut().take())
}

/// Take the dispatch output (if set). Clears after reading.
pub fn take_dispatch_output() -> Option<String> {
    DISPATCH_OUTPUT.with(|cell| cell.borrow_mut().take())
}

/// Store a message that should be shown to the LLM as tool output.
pub fn set_dispatch_output(msg: &str) {
    DISPATCH_OUTPUT.with(|cell| *cell.borrow_mut() = Some(msg.to_string()));
}

/// Peek at the lazily provisioned sandbox URL without consuming it.
pub fn peek_lazy_sandbox_url() -> Option<String> {
    LAZY_SANDBOX.with(|cell| cell.borrow().as_ref().map(|(url, _)| url.clone()))
}

/// Dispatch a `temper.<method>()` or `sandbox.<method>()` call.
///
/// Called by the Monty event loop when user code invokes a method on a
/// dataclass object. `obj_name` is `"temper"` or `"sandbox"`, `method`
/// is the method name, and `args` are the JSON-converted positional args.
pub fn dispatch(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    sandbox_url: &str,
    workdir: &str,
    obj_name: &str,
    method: &str,
    args: &[Value],
) -> Result<Value, String> {
    // Lazy sandbox provisioning (ADR-0022): if this tool needs a sandbox and
    // none is attached, provision one on-demand instead of failing.
    let needs_sandbox = obj_name == "sandbox"
        || (obj_name == "temper" && method == "run_coding_agent");

    let effective_sandbox_url = if needs_sandbox && sandbox_url.is_empty() {
        // Check thread-local cache first (already provisioned this invocation)
        if let Some(url) = peek_lazy_sandbox_url() {
            url
        } else {
            // Lazy provision
            match lazy_provision_sandbox(ctx, temper_api_url, tenant) {
                Ok(url) => url,
                Err(e) => {
                    return Err(format!(
                        "This tool requires a code execution sandbox, but sandbox provisioning failed: {e}. \
                         You can still use non-sandbox tools (temper.create, temper.list, temper.web_search, etc.) \
                         to help the user."
                    ));
                }
            }
        }
    } else {
        sandbox_url.to_string()
    };

    ensure_method_enabled(ctx, obj_name, method, &effective_sandbox_url)?;
    let result = match obj_name {
        "temper" => dispatch_temper(
            ctx,
            temper_api_url,
            tenant,
            &effective_sandbox_url,
            workdir,
            method,
            args,
        ),
        "sandbox" => {
            let sandbox_api_key = ctx
                .config
                .get("tensorlake_api_key")
                .cloned()
                .unwrap_or_default();
            dispatch_sandbox(ctx, &effective_sandbox_url, workdir, &sandbox_api_key, method, args)
        }
        _ => Err(format!("unknown object: {obj_name}")),
    };

    // Enrich Cedar denial errors with tool context so the REPL loop can
    // build a complete pause payload (decision ID + what was being attempted).
    if let Err(ref e) = result {
        if let Some(rest) = e.strip_prefix("CEDAR_DENIED:") {
            // rest = "{decision_id}:{body}" — split on first ':'
            let (decision_id, body) = rest.split_once(':').unwrap_or((rest, ""));
            let ctx_json = json!({
                "decision_id": decision_id,
                "method": format!("{obj_name}.{method}"),
                "args": args,
                "body": body,
            });
            // Store in thread-local so lib.rs can detect the denial even if
            // Monty Python code catches the RuntimeError exception.
            let ctx_str = ctx_json.to_string();
            CEDAR_DENIAL.with(|cell| {
                *cell.borrow_mut() = Some(ctx_str.clone());
            });
            return Err(format!("CEDAR_DENIED_CTX:{ctx_str}"));
        }
    }

    result
}

fn ensure_method_enabled(
    ctx: &Context,
    obj_name: &str,
    method: &str,
    sandbox_url: &str,
) -> Result<(), String> {
    let enabled = enabled_tools(ctx);

    match obj_name {
        "sandbox" => {
            if sandbox_url.is_empty() {
                return Err("sandbox is not configured for this session".to_string());
            }

            let Some(token) = sandbox_method_token(method) else {
                return Ok(());
            };
            if enabled.contains(token) {
                return Ok(());
            }

            Err(format!(
                "sandbox.{method}() is not enabled for this session. Enabled tools: {}",
                format_enabled_tools(&enabled)
            ))
        }
        "temper" => {
            let Some(token) = temper_method_token(method) else {
                return Ok(());
            };
            if enabled.contains(token) {
                return Ok(());
            }

            Err(format!(
                "temper.{method}() is not enabled for this session. Enabled tools: {}",
                format_enabled_tools(&enabled)
            ))
        }
        _ => Ok(()),
    }
}

fn enabled_tools(ctx: &Context) -> BTreeSet<String> {
    ctx.entity_state
        .get("fields")
        .and_then(|fields| fields.get("tools_enabled"))
        .and_then(|value| value.as_str())
        .unwrap_or(DEFAULT_TOOLS_ENABLED)
        .split(',')
        .map(str::trim)
        .filter(|tool| !tool.is_empty())
        .map(|tool| match tool {
            "read_entity" => "temper_get",
            "save_memory" => "temper_save_memory",
            "recall_memory" => "temper_recall_memory",
            "spawn_agent" | "spawn_session" => "temper_spawn_session",
            "temper_file_upload" => "temper_write",
            other => other,
        })
        .map(ToOwned::to_owned)
        .collect()
}

fn format_enabled_tools(enabled: &BTreeSet<String>) -> String {
    if enabled.is_empty() {
        "(none)".to_string()
    } else {
        enabled.iter().cloned().collect::<Vec<_>>().join(", ")
    }
}

fn sandbox_method_token(method: &str) -> Option<&'static str> {
    match method {
        "bash" => Some("bash"),
        "read" => Some("read"),
        "write" => Some("write"),
        "edit" => Some("edit"),
        _ => None,
    }
}

fn temper_method_token(method: &str) -> Option<&'static str> {
    match method {
        "get" => Some("temper_get"),
        "list" => Some("temper_list"),
        "create" => Some("temper_create"),
        "action" => Some("temper_action"),
        "patch" => Some("temper_patch"),
        "submit_specs" => Some("temper_submit_specs"),
        "show_spec" | "spec_detail" => Some("temper_show_spec"),
        "specs" => Some("temper_specs"),
        "upload_wasm" => Some("temper_upload_wasm"),
        "get_trajectories" => Some("temper_get_trajectories"),
        "get_insights" => Some("temper_get_insights"),
        "get_decisions" => Some("temper_get_decisions"),
        "poll_decision" => Some("temper_poll_decision"),
        "approve_decision" => Some("temper_approve_decision"),
        "deny_decision" => Some("temper_deny_decision"),
        "submit_policy" => Some("temper_submit_policy"),
        "list_policies" => Some("temper_list_policies"),
        "get_policy" => Some("temper_get_policy"),
        "update_policy" => Some("temper_update_policy"),
        "delete_policy" => Some("temper_delete_policy"),
        "install_app" => Some("temper_install_app"),
        "list_apps" => Some("temper_list_apps"),
        "spawn_session" => Some("temper_spawn_session"),
        "list_sessions" => Some("temper_list_sessions"),
        "abort_session" => Some("temper_abort_session"),
        "steer_session" => Some("temper_steer_session"),
        "save_memory" => Some("temper_save_memory"),
        "recall_memory" => Some("temper_recall_memory"),
        "write" => Some("temper_write"),
        "read" => Some("temper_read"),
        "run_coding_agent" => Some("temper_run_coding_agent"),
        "get_secret" => Some("temper_get_secret"),
        "datadog_query" => Some("temper_datadog_query"),
        "railway" => Some("temper_railway"),
        "vercel" => Some("temper_vercel"),
        "web_search" => Some("temper_web_search"),
        "web_fetch" => Some("temper_web_fetch"),
        "done" | "get_agent_id" | "get_session_id" | "switch_provider" | "switch_mode" => None,
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Temper dispatch
// ---------------------------------------------------------------------------

fn dispatch_temper(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    sandbox_url: &str,
    workdir: &str,
    method: &str,
    args: &[Value],
) -> Result<Value, String> {
    match method {
        // Entity CRUD
        "list" => temper_list(ctx, api_url, tenant, args),
        "get" => temper_get(ctx, api_url, tenant, args),
        "create" => temper_create(ctx, api_url, tenant, args),
        "action" => temper_action(ctx, api_url, tenant, args),
        "patch" => temper_patch(ctx, api_url, tenant, args),

        // Specs
        "submit_specs" => temper_submit_specs(ctx, api_url, tenant, args),
        "show_spec" | "spec_detail" => temper_show_spec(ctx, api_url, tenant, args),
        "specs" => temper_specs(ctx, api_url, tenant),

        // WASM
        "upload_wasm" => temper_upload_wasm(ctx, api_url, tenant, args),

        // Evolution
        "get_trajectories" => temper_get_trajectories(ctx, api_url, tenant, args),
        "get_insights" => temper_get_insights(ctx, api_url, tenant),

        // Governance — decisions
        "get_decisions" => temper_get_decisions(ctx, api_url, tenant),
        "poll_decision" => temper_poll_decision(ctx, api_url, tenant, args),
        "approve_decision" => temper_approve_decision(ctx, api_url, tenant, args),
        "deny_decision" => temper_deny_decision(ctx, api_url, tenant, args),

        // Governance — Cedar policies (all Cedar-gated by platform)
        "submit_policy" => temper_submit_policy(ctx, api_url, tenant, args),
        "list_policies" => temper_list_policies(ctx, api_url, tenant),
        "get_policy" => temper_get_policy(ctx, api_url, tenant, args),
        "update_policy" => temper_update_policy(ctx, api_url, tenant, args),
        "delete_policy" => temper_delete_policy(ctx, api_url, tenant, args),

        // Apps
        "install_app" => temper_install_app(ctx, api_url, tenant, args),
        "list_apps" => temper_list_apps(ctx, api_url, tenant),

        // Agent identity
        "get_agent_id" => {
            let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
            let agent_id = fields
                .get("agent_id")
                .or_else(|| fields.get("AgentId"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Ok(json!(agent_id))
        }
        "get_session_id" => {
            let session_id = ctx
                .entity_state
                .get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Ok(json!(session_id))
        }

        // Switch own LLM provider/model mid-session
        "switch_provider" => {
            let input = args.first().and_then(|v| v.as_object()).cloned().unwrap_or_default();
            let model = input.get("model").and_then(|v| v.as_str()).unwrap_or("");
            let provider = input.get("provider").and_then(|v| v.as_str()).unwrap_or("");
            if model.is_empty() && provider.is_empty() {
                return Err("switch_provider requires at least one of: model, provider".into());
            }
            let agent_id = ctx
                .entity_state
                .get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let mut body = serde_json::Map::new();
            if !model.is_empty() {
                body.insert("model".into(), json!(model));
            }
            if !provider.is_empty() {
                body.insert("provider".into(), json!(provider));
            }
            let url = format!("{api_url}/tdata/Sessions('{agent_id}')/OpenPaw.SwitchProvider");
            let headers: Vec<(String, String)> = vec![
                ("Content-Type".into(), "application/json".into()),
                ("X-Tenant-Id".into(), tenant.to_string()),
                ("x-temper-principal-kind".into(), "agent".into()),
                ("x-temper-principal-id".into(), agent_id.to_string()),
                ("x-temper-agent-type".into(), "agent".into()),
            ];
            let resp = ctx.http_call("POST", &url, &headers, &json!(body).to_string())?;
            if let Some(denial) = check_cedar_denial(resp.status, &resp.body) {
                return Err(denial);
            }
            if resp.status >= 200 && resp.status < 300 {
                Ok(json!({
                    "switched": true,
                    "model": if model.is_empty() { "unchanged" } else { model },
                    "provider": if provider.is_empty() { "unchanged" } else { provider },
                }))
            } else {
                Err(format!("SwitchProvider failed (HTTP {}): {}", resp.status, &resp.body[..resp.body.len().min(200)]))
            }
        }

        // Switch session mode between plan and execute (ADR-004)
        "switch_mode" => {
            let input = args.first().and_then(|v| v.as_object()).cloned().unwrap_or_default();
            let target_mode = input.get("mode").and_then(|v| v.as_str()).unwrap_or("");
            if target_mode != "plan" && target_mode != "execute" {
                return Err("switch_mode requires mode='plan' or mode='execute'".into());
            }
            let agent_id = ctx
                .entity_state
                .get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
            let current_tools = fields
                .get("tools_enabled")
                .and_then(|v| v.as_str())
                .unwrap_or(DEFAULT_TOOLS_ENABLED);

            let mut body = serde_json::Map::new();
            body.insert("session_mode".into(), json!(target_mode));

            if target_mode == "plan" {
                // Stash current tools so they can be restored on switch to execute
                body.insert("pre_plan_tools_enabled".into(), json!(current_tools));
                body.insert("tools_enabled".into(), json!(PLAN_MODE_TOOLS));
            } else {
                // Restore stashed tools
                let stashed = fields
                    .get("pre_plan_tools_enabled")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(DEFAULT_TOOLS_ENABLED);
                body.insert("tools_enabled".into(), json!(stashed));
                body.insert("pre_plan_tools_enabled".into(), json!(""));
            }

            // Carry active_plan_id if provided
            if let Some(plan_id) = input.get("plan_id").and_then(|v| v.as_str()) {
                body.insert("active_plan_id".into(), json!(plan_id));
            }

            let url = format!("{api_url}/tdata/Sessions('{agent_id}')/OpenPaw.SwitchMode");
            let headers: Vec<(String, String)> = vec![
                ("Content-Type".into(), "application/json".into()),
                ("X-Tenant-Id".into(), tenant.to_string()),
                ("x-temper-principal-kind".into(), "agent".into()),
                ("x-temper-principal-id".into(), agent_id.to_string()),
                ("x-temper-agent-type".into(), "agent".into()),
            ];
            let resp = ctx.http_call("POST", &url, &headers, &json!(body).to_string())?;
            if let Some(denial) = check_cedar_denial(resp.status, &resp.body) {
                return Err(denial);
            }
            if resp.status >= 200 && resp.status < 300 {
                Ok(json!({
                    "switched": true,
                    "mode": target_mode,
                }))
            } else {
                Err(format!(
                    "SwitchMode failed (HTTP {}): {}",
                    resp.status,
                    &resp.body[..resp.body.len().min(200)]
                ))
            }
        }

        // Entity operations (ported from tool_runner)
        "spawn_session" => {
            super::entity_ops::spawn_session(ctx, api_url, tenant, sandbox_url, workdir, args)
        }
        "list_sessions" => super::entity_ops::list_sessions(ctx, api_url, tenant, args),
        "abort_session" => super::entity_ops::abort_session(ctx, api_url, tenant, args),
        "steer_session" => super::entity_ops::steer_session(ctx, api_url, tenant, args),
        "save_memory" => super::entity_ops::save_memory(ctx, api_url, tenant, args),
        "recall_memory" => super::entity_ops::recall_memory(ctx, api_url, tenant, args),
        "write" => super::entity_ops::write(ctx, api_url, tenant, args),
        "read" => super::entity_ops::read(ctx, api_url, tenant, args),
        "run_coding_agent" => {
            super::entity_ops::run_coding_agent(ctx, api_url, tenant, sandbox_url, workdir, args)
        }

        // Secrets (Cedar-gated via access_secret on Secret resource)
        "get_secret" => temper_get_secret(ctx, args),

        // Session completion — agent signals "I'm done"
        "done" => temper_done(args),

        // External service integrations (ported from tool_runner)
        "datadog_query" => super::datadog::datadog_query(ctx, args),
        "railway" => super::railway::railway(ctx, args),
        "vercel" => super::vercel::vercel(ctx, args),

        // Web research (backed by standalone WASM modules via WebQuery entity)
        "web_search" => temper_web_search(ctx, api_url, tenant, args),
        "web_fetch" => temper_web_fetch(ctx, api_url, tenant, args),

        _ => Err(format!(
            "unknown temper method '{method}'. Available: \
             list, get, create, action, patch, submit_specs, show_spec, \
             upload_wasm, get_trajectories, get_insights, \
             get_decisions, poll_decision, approve_decision, deny_decision, \
             submit_policy, list_policies, get_policy, update_policy, delete_policy, \
             get_secret, done, install_app, list_apps, get_agent_id, get_session_id, \
             spawn_session, list_sessions, abort_session, steer_session, \
             save_memory, recall_memory, write, read, \
             run_coding_agent, datadog_query, railway, vercel, \
             web_search, web_fetch"
        )),
    }
}

// --- Entity CRUD ---

fn temper_list(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let entity_set = str_arg(args, 0, "entity_set", "list")?;
    let query = opt_str_arg(args, 1).map(|arg| normalize_odata_query_arg(&arg));
    let path = match query {
        Some(ODataQueryArg::Filter(filter)) => {
            let encoded = encode_odata_query_value(&filter);
            format!("/tdata/{entity_set}?$filter={encoded}")
        }
        Some(ODataQueryArg::Raw(raw_query)) => {
            let encoded = encode_odata_query_value(&raw_query);
            format!("/tdata/{entity_set}?{encoded}")
        }
        None => format!("/tdata/{entity_set}"),
    };
    let resp = http_get(ctx, api_url, tenant, &path)?;
    Ok(resp.get("value").cloned().unwrap_or(resp))
}

fn temper_get(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let entity_set = str_arg(args, 0, "entity_set", "get")?;
    let entity_id = str_arg(args, 1, "entity_id", "get")?;
    let key = escape_odata_key(&entity_id);
    http_get(
        ctx,
        api_url,
        tenant,
        &format!("/tdata/{entity_set}('{key}')"),
    )
}

enum ODataQueryArg {
    Filter(String),
    Raw(String),
}

fn normalize_odata_query_arg(arg: &str) -> ODataQueryArg {
    let trimmed = arg.trim().trim_start_matches('?');
    if let Some(filter) = trimmed.strip_prefix("$filter=") {
        ODataQueryArg::Filter(filter.trim().to_string())
    } else if trimmed.starts_with('$') {
        ODataQueryArg::Raw(trimmed.to_string())
    } else {
        ODataQueryArg::Filter(trimmed.to_string())
    }
}

fn encode_odata_query_value(value: &str) -> String {
    value.replace(' ', "%20").replace('\'', "%27")
}

fn temper_create(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let entity_set = str_arg(args, 0, "entity_set", "create")?;
    let body = obj_arg(args, 1, "fields", "create")?;
    http_post(ctx, api_url, tenant, &format!("/tdata/{entity_set}"), &body)
}

fn temper_action(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let entity_set = str_arg(args, 0, "entity_set", "action")?;
    let entity_id = str_arg(args, 1, "entity_id", "action")?;
    let action_name = str_arg(args, 2, "action_name", "action")?;
    let body = obj_arg_or_empty(args, 3);
    let key = escape_odata_key(&entity_id);
    http_post(
        ctx,
        api_url,
        tenant,
        &format!("/tdata/{entity_set}('{key}')/Temper.{action_name}"),
        &body,
    )
}

fn temper_patch(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let entity_set = str_arg(args, 0, "entity_set", "patch")?;
    let entity_id = str_arg(args, 1, "entity_id", "patch")?;
    let body = obj_arg(args, 2, "fields", "patch")?;
    let key = escape_odata_key(&entity_id);
    http_patch(
        ctx,
        api_url,
        tenant,
        &format!("/tdata/{entity_set}('{key}')"),
        &body,
    )
}

// --- Specs ---

fn temper_submit_specs(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let specs = obj_arg(args, 0, "specs", "submit_specs")?;
    let spec_names: Vec<String> = specs.as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    let body = json!({ "tenant": tenant, "specs": specs });
    let result = http_post(ctx, api_url, tenant, "/api/specs/load-inline", &body)?;
    // Surface the outcome via dispatch output so the LLM sees it even
    // if the Python code assigns the return value to a variable.
    let msg = if result.get("ok").and_then(|v| v.as_bool()) == Some(true) || result.is_null() {
        format!("Specs loaded successfully: {}", spec_names.join(", "))
    } else {
        format!("submit_specs response: {result}")
    };
    ctx.log("info", &format!("submit_specs: {msg}"));
    set_dispatch_output(&msg);
    Ok(json!({
        "ok": true,
        "message": msg,
        "specs_submitted": spec_names,
    }))
}

fn temper_show_spec(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let entity_type = str_arg(args, 0, "entity_type", "show_spec")?;
    http_get(
        ctx,
        api_url,
        tenant,
        &format!("/observe/specs/{entity_type}"),
    )
}

fn temper_specs(ctx: &Context, api_url: &str, tenant: &str) -> Result<Value, String> {
    http_get(ctx, api_url, tenant, "/observe/specs")
}

// --- WASM ---

fn temper_upload_wasm(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let module_name = str_arg(args, 0, "module_name", "upload_wasm")?;
    let wasm_base64 = str_arg(args, 1, "wasm_base64", "upload_wasm")?;
    let body = json!({ "wasm_base64": wasm_base64 });
    http_post(
        ctx,
        api_url,
        tenant,
        &format!("/api/wasm/modules/{module_name}"),
        &body,
    )
}

// --- Evolution ---

fn temper_get_trajectories(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let entity_type = opt_str_arg(args, 0);
    let failed_only = args.get(1).and_then(|v| v.as_bool()).unwrap_or(false);
    let limit = args.get(2).and_then(|v| v.as_u64()).unwrap_or(50);
    let mut path = format!("/api/evolution/trajectories?limit={limit}");
    if let Some(et) = entity_type {
        path.push_str(&format!("&entity_type={et}"));
    }
    if failed_only {
        path.push_str("&failed_only=true");
    }
    http_get(ctx, api_url, tenant, &path)
}

fn temper_get_insights(ctx: &Context, api_url: &str, tenant: &str) -> Result<Value, String> {
    http_get(ctx, api_url, tenant, "/api/evolution/insights")
}

// --- Governance ---

fn temper_get_decisions(ctx: &Context, api_url: &str, tenant: &str) -> Result<Value, String> {
    http_get(ctx, api_url, tenant, "/api/decisions")
}

fn temper_poll_decision(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let decision_id = str_arg(args, 0, "decision_id", "poll_decision")?;
    // Poll once — the agent can call repeatedly if needed.
    // Full blocking poll would exceed WASM execution budget.
    http_get(
        ctx,
        api_url,
        tenant,
        &format!("/api/decisions/{decision_id}"),
    )
}

// --- Web research (dispatch wrappers for standalone WASM modules) ---

/// Search the web via WebQuery entity + web_search WASM module.
/// Creates a WebQuery, dispatches ExecuteSearch, reads results.
fn temper_web_search(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let query = str_arg(args, 0, "query", "web_search")?;
    web_query_dispatch(ctx, api_url, tenant, "search", &query, "")
}

/// Fetch a URL via WebQuery entity + web_fetch WASM module.
/// Creates a WebQuery, dispatches ExecuteFetch, reads results.
fn temper_web_fetch(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let url = str_arg(args, 0, "url", "web_fetch")?;
    web_query_dispatch(ctx, api_url, tenant, "fetch", "", &url)
}

/// Shared implementation: create WebQuery entity, dispatch action, return results.
fn web_query_dispatch(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    query_type: &str,
    query: &str,
    url: &str,
) -> Result<Value, String> {
    // 1. Create WebQuery entity
    let body = json!({
        "QueryType": query_type,
        "Query": query,
        "Url": url,
    });
    let entity = http_post(ctx, api_url, tenant, "/tdata/WebQueries", &body)?;

    let entity_id = entity
        .get("entity_id")
        .or_else(|| entity.get("EntityId"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "web_query: failed to get entity_id from created WebQuery".to_string())?;

    // 2. Dispatch the appropriate Execute action
    let action_name = if query_type == "search" {
        "ExecuteSearch"
    } else {
        "ExecuteFetch"
    };
    let action_params = if query_type == "search" {
        json!({"query": query})
    } else {
        json!({"url": url})
    };
    let key = escape_odata_key(entity_id);
    let _ = http_post(
        ctx,
        api_url,
        tenant,
        &format!("/tdata/WebQueries('{key}')/Temper.{action_name}?await_integration=true"),
        &action_params,
    );

    // 3. Read the entity back — WASM integration has run by this point
    let result = http_get(ctx, api_url, tenant, &format!("/tdata/WebQueries('{key}')"))?;
    let result_fields = result.get("fields").cloned().unwrap_or(result.clone());

    let status = result_fields
        .get("Status")
        .or_else(|| result_fields.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if status == "Failed" {
        let error = result_fields
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err(format!("web_{query_type}: {error}"));
    }

    // If result_file_id is set, the full content is in TemperFS (large results).
    // Read the file content, then delete the file — it's ephemeral transport only.
    // WORKAROUND: This delete-after-read pattern exists because Temper's entity field
    // sync truncates values >32KB. The proper fix is platform-level blob-backed fields
    // with TTL. See: https://github.com/nerdsane/temper/issues/106
    let result_file_id = result_fields
        .get("result_file_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());

    if let Some(file_id) = result_file_id {
        let admin_headers = vec![
            ("X-Tenant-Id".to_string(), tenant.to_string()),
            ("x-temper-principal-kind".to_string(), "agent".to_string()),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];
        let read_headers = {
            let mut h = admin_headers.clone();
            h.push(("Accept".to_string(), "text/plain".to_string()));
            h
        };
        let read_url = format!("{api_url}/tdata/Files('{file_id}')/$value");
        let content = match ctx.http_call("GET", &read_url, &read_headers, "") {
            Ok(resp) if resp.status >= 200 && resp.status < 300 => Some(resp.body),
            Ok(resp) => {
                ctx.log("warn", &format!("web_query: failed to read result file {file_id} (HTTP {})", resp.status));
                None
            }
            Err(e) => {
                ctx.log("warn", &format!("web_query: failed to read result file {file_id}: {e}"));
                None
            }
        };

        // Delete the ephemeral file — best effort, don't fail the query if cleanup fails.
        let archive_url = format!("{api_url}/tdata/Files('{file_id}')/Temper.Archive");
        let archive_headers = {
            let mut h = admin_headers;
            h.push(("Content-Type".to_string(), "application/json".to_string()));
            h
        };
        let _ = ctx.http_call("POST", &archive_url, &archive_headers, "{}");

        if let Some(text) = content {
            return Ok(json!(text));
        }
        // Fall through to inline results if file read fails
    }

    let results_raw = result_fields
        .get("results")
        .and_then(|v| v.as_str())
        .unwrap_or("[]");

    // Try to parse as JSON array; if not, return as plain text
    match serde_json::from_str::<Value>(results_raw) {
        Ok(parsed) => Ok(parsed),
        Err(_) => Ok(json!(results_raw)),
    }
}

// --- Session completion ---

fn temper_done(args: &[Value]) -> Result<Value, String> {
    let result = args
        .first()
        .and_then(|v| v.as_str())
        .unwrap_or("(done)")
        .to_string();
    DONE_RESULT.with(|cell| {
        *cell.borrow_mut() = Some(result);
    });
    Ok(json!({"done": true}))
}

// --- Secrets (Cedar-gated via access_secret on Secret resource) ---

fn temper_get_secret(ctx: &Context, args: &[Value]) -> Result<Value, String> {
    let key = str_arg(args, 0, "key", "get_secret")?;
    let value = ctx.get_secret(&key)?;
    Ok(json!(value))
}

// --- Cedar Policy Management (all Cedar-gated by platform) ---

fn temper_submit_policy(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let policy_id = str_arg(args, 0, "policy_id", "submit_policy")?;
    let cedar_text = str_arg(args, 1, "cedar_text", "submit_policy")?;
    let body = json!({ "policy_id": policy_id, "cedar_text": cedar_text });
    http_post(
        ctx,
        api_url,
        tenant,
        &format!("/api/tenants/{tenant}/policies/create"),
        &body,
    )
}

fn temper_list_policies(ctx: &Context, api_url: &str, tenant: &str) -> Result<Value, String> {
    http_get(
        ctx,
        api_url,
        tenant,
        &format!("/api/tenants/{tenant}/policies/list"),
    )
}

fn temper_get_policy(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let policy_id = str_arg(args, 0, "policy_id", "get_policy")?;
    // List all and filter — no single-policy GET endpoint
    let all = http_get(
        ctx,
        api_url,
        tenant,
        &format!("/api/tenants/{tenant}/policies/list"),
    )?;
    let policies = all.get("policies").and_then(|v| v.as_array());
    if let Some(list) = policies {
        for p in list {
            if p.get("policy_id").and_then(|v| v.as_str()) == Some(&policy_id) {
                return Ok(p.clone());
            }
        }
    }
    Err(format!("policy '{policy_id}' not found"))
}

fn temper_update_policy(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let policy_id = str_arg(args, 0, "policy_id", "update_policy")?;
    let cedar_text = str_arg(args, 1, "cedar_text", "update_policy")?;
    let body = json!({ "cedar_text": cedar_text });
    http_patch(
        ctx,
        api_url,
        tenant,
        &format!("/api/tenants/{tenant}/policies/entry/{policy_id}"),
        &body,
    )
}

fn temper_delete_policy(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let policy_id = str_arg(args, 0, "policy_id", "delete_policy")?;
    http_delete(
        ctx,
        api_url,
        tenant,
        &format!("/api/tenants/{tenant}/policies/entry/{policy_id}"),
    )
}

// --- Decision Management (Cedar-gated by platform) ---

fn temper_approve_decision(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let decision_id = str_arg(args, 0, "decision_id", "approve_decision")?;
    let scope = obj_arg(args, 1, "scope", "approve_decision")?;
    let agent_id = ctx
        .entity_state
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("agent");
    let body = json!({ "scope": scope, "decided_by": format!("agent:{agent_id}") });
    http_post(
        ctx,
        api_url,
        tenant,
        &format!("/api/tenants/{tenant}/decisions/{decision_id}/approve"),
        &body,
    )
}

fn temper_deny_decision(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let decision_id = str_arg(args, 0, "decision_id", "deny_decision")?;
    let agent_id = ctx
        .entity_state
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("agent");
    let body = json!({ "decided_by": format!("agent:{agent_id}") });
    http_post(
        ctx,
        api_url,
        tenant,
        &format!("/api/tenants/{tenant}/decisions/{decision_id}/deny"),
        &body,
    )
}

// --- Apps ---

fn temper_install_app(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let app_name = str_arg(args, 0, "app_name", "install_app")?;
    let reason = opt_str_arg(args, 1).unwrap_or_default();
    let payload = opt_str_arg(args, 2).unwrap_or_default();
    let cap_type = opt_str_arg(args, 3).unwrap_or_else(|| "os_app".to_string());
    let agent_id = ctx
        .entity_state
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Create a CapabilityRequest entity (Cedar-governed).
    // Use PascalCase keys to match CSDL property names (Temper maps these to IOA variables).
    let body = json!({
        "CapabilityType": cap_type,
        "CapabilityName": app_name,
        "Reason": reason,
        "RequestingAgentId": agent_id,
        "Payload": payload,
    });
    http_post(ctx, api_url, tenant, "/tdata/CapabilityRequests", &body)
}

fn temper_list_apps(ctx: &Context, api_url: &str, tenant: &str) -> Result<Value, String> {
    http_get(ctx, api_url, tenant, "/api/apps")
}

// ---------------------------------------------------------------------------
// Lazy sandbox provisioning (ADR-0022)
// ---------------------------------------------------------------------------

/// Provision a Tensorlake sandbox on-demand. Called when a sandbox tool is
/// invoked but no sandbox_url is set on the session.
///
/// Returns the sandbox_url on success. Caches (sandbox_url, sandbox_id) in
/// the LAZY_SANDBOX thread-local so subsequent tool calls in the same
/// invocation reuse it, and lib.rs persists it via HandleToolResults params.
fn lazy_provision_sandbox(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
) -> Result<String, String> {
    let api_key = ctx
        .config
        .get("tensorlake_api_key")
        .filter(|s| !s.is_empty() && !s.contains("{secret:"))
        .cloned()
        .ok_or_else(|| {
            "no tensorlake_api_key configured — set TL_API_KEY in .env for sandbox provisioning"
                .to_string()
        })?;

    ctx.log("info", "lazy_provision_sandbox: provisioning via Tensorlake API");

    // Send heartbeat so the user sees a typing indicator
    super::session::send_heartbeat(ctx, temper_api_url, tenant);

    // Create sandbox
    let create_url = "https://api.tensorlake.ai/sandboxes";
    let headers = vec![
        ("authorization".to_string(), format!("Bearer {api_key}")),
        ("content-type".to_string(), "application/json".to_string()),
    ];
    let body = json!({
        "resources": {
            "cpus": 2,
            "memory_mb": 4096
        },
        "timeout_seconds": 3600,
        "internet_access": true
    });
    let resp = ctx.http_call("POST", create_url, &headers, &body.to_string())?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "Tensorlake sandbox creation failed (HTTP {}): {}",
            resp.status,
            &resp.body[..resp.body.len().min(500)]
        ));
    }

    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse Tensorlake response: {e}"))?;
    let sandbox_id = parsed
        .get("sandbox_id")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            parsed
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("tensorlake-sandbox")
        })
        .to_string();
    let sandbox_url = format!("https://{sandbox_id}.sandbox.tensorlake.ai");

    // Poll for readiness (max 12 retries = ~60s)
    let max_checks = 12;
    let health_headers = vec![("authorization".to_string(), format!("Bearer {api_key}"))];
    let health_url = format!("{sandbox_url}/api/v1/files/list?path=/");

    for attempt in 0..max_checks {
        match ctx.http_call("GET", &health_url, &health_headers, "") {
            Ok(r) if r.status >= 200 && r.status < 300 => {
                ctx.log(
                    "info",
                    &format!(
                        "lazy_provision_sandbox: sandbox ready after {} checks: id={sandbox_id}",
                        attempt + 1
                    ),
                );

                // Run post-provisioning setup (gh CLI etc.) — non-fatal
                run_sandbox_setup(ctx, &sandbox_url, &api_key);

                // Cache in thread-local
                LAZY_SANDBOX.with(|cell| {
                    *cell.borrow_mut() = Some((sandbox_url.clone(), sandbox_id.clone()));
                });

                return Ok(sandbox_url);
            }
            Ok(r) => {
                ctx.log(
                    "info",
                    &format!(
                        "lazy_provision_sandbox: sandbox not ready (HTTP {}), check {}/{}",
                        r.status,
                        attempt + 1,
                        max_checks
                    ),
                );
            }
            Err(err) => {
                ctx.log(
                    "info",
                    &format!(
                        "lazy_provision_sandbox: readiness check failed ({}), check {}/{}",
                        err,
                        attempt + 1,
                        max_checks
                    ),
                );
            }
        }

        // Send heartbeat between retries (typing indicator)
        if attempt % 3 == 2 {
            super::session::send_heartbeat(ctx, temper_api_url, tenant);
        }
    }

    Err(format!(
        "Tensorlake sandbox {sandbox_id} did not become ready within {max_checks} readiness checks. \
         The sandbox may still be booting — try again in a moment."
    ))
}

/// Run post-provisioning setup on a sandbox (gh CLI install etc.). Non-fatal.
fn run_sandbox_setup(ctx: &Context, sandbox_url: &str, api_key: &str) {
    if sandbox_url.is_empty() {
        return;
    }

    let gh_setup = r#"
if ! command -v gh &>/dev/null; then
  (type -p wget >/dev/null || (apt-get update && apt-get install wget -y)) && \
  mkdir -p -m 755 /etc/apt/keyrings && \
  out=$(mktemp) && wget -nv -O"$out" https://cli.github.com/packages/githubcli-archive-keyring.gpg && \
  cat "$out" | tee /etc/apt/keyrings/githubcli-archive-keyring.gpg > /dev/null && \
  chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg && \
  echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | tee /etc/apt/sources.list.d/github-cli.list > /dev/null && \
  apt-get update && apt-get install gh -y
fi
gh --version 2>/dev/null || echo 'gh: not installed'
"#;

    let headers = vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("authorization".to_string(), format!("Bearer {api_key}")),
    ];
    let body = json!({
        "command": gh_setup,
        "timeout": 120
    });
    let url = format!("{sandbox_url}/commands");

    match ctx.http_call("POST", &url, &headers, &body.to_string()) {
        Ok(resp) if resp.status >= 200 && resp.status < 300 => {
            ctx.log("info", "lazy_provision_sandbox: gh CLI setup completed");
        }
        Ok(resp) => {
            ctx.log(
                "warn",
                &format!(
                    "lazy_provision_sandbox: gh CLI setup failed (HTTP {})",
                    resp.status
                ),
            );
        }
        Err(e) => {
            ctx.log(
                "warn",
                &format!("lazy_provision_sandbox: gh CLI setup request failed: {e}"),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Sandbox dispatch
// ---------------------------------------------------------------------------

fn dispatch_sandbox(
    ctx: &Context,
    sandbox_url: &str,
    workdir: &str,
    sandbox_api_key: &str,
    method: &str,
    args: &[Value],
) -> Result<Value, String> {
    if sandbox_url.is_empty() {
        return Err(format!("sandbox.{method}(): no sandbox attached"));
    }

    match method {
        "read" => sandbox_read(ctx, sandbox_url, sandbox_api_key, args),
        "write" => sandbox_write(ctx, sandbox_url, sandbox_api_key, args),
        "edit" => sandbox_edit(ctx, sandbox_url, sandbox_api_key, args),
        "bash" => sandbox_bash(ctx, sandbox_url, sandbox_api_key, workdir, args),
        _ => Err(format!(
            "unknown sandbox method '{method}'. Available: read, write, edit, bash"
        )),
    }
}

fn sandbox_headers(api_key: &str) -> Vec<(String, String)> {
    if api_key.is_empty() {
        vec![]
    } else {
        vec![("Authorization".to_string(), format!("Bearer {api_key}"))]
    }
}

fn sandbox_headers_json(api_key: &str) -> Vec<(String, String)> {
    let mut h = sandbox_headers(api_key);
    h.push(("Content-Type".to_string(), "application/json".to_string()));
    h
}

fn sandbox_read(
    ctx: &Context,
    sandbox_url: &str,
    api_key: &str,
    args: &[Value],
) -> Result<Value, String> {
    let path = str_arg(args, 0, "path", "read")?;
    let url = format!("{sandbox_url}/api/v1/files?path={}", urlenc(&path));
    let resp = ctx.http_call("GET", &url, &sandbox_headers(api_key), "")?;
    if resp.status >= 400 {
        return Err(format!("sandbox.read({path}): {}", resp.body));
    }
    Ok(json!(resp.body))
}

fn sandbox_write(
    ctx: &Context,
    sandbox_url: &str,
    api_key: &str,
    args: &[Value],
) -> Result<Value, String> {
    let path = str_arg(args, 0, "path", "write")?;
    let content = str_arg(args, 1, "content", "write")?;
    let url = format!("{sandbox_url}/api/v1/files?path={}", urlenc(&path));
    let resp = ctx.http_call("PUT", &url, &sandbox_headers(api_key), &content)?;
    if resp.status >= 400 {
        return Err(format!("sandbox.write({path}): {}", resp.body));
    }
    Ok(json!({"ok": true}))
}

fn sandbox_edit(
    ctx: &Context,
    sandbox_url: &str,
    api_key: &str,
    args: &[Value],
) -> Result<Value, String> {
    let path = str_arg(args, 0, "path", "edit")?;
    let old_string = str_arg(args, 1, "old_string", "edit")?;
    let new_string = str_arg(args, 2, "new_string", "edit")?;

    let headers = sandbox_headers(api_key);
    let url = format!("{sandbox_url}/api/v1/files?path={}", urlenc(&path));
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status >= 400 {
        return Err(format!("sandbox.edit({path}): read failed: {}", resp.body));
    }

    let content = resp.body;
    if !content.contains(&old_string) {
        return Err(format!(
            "sandbox.edit({path}): old_string not found in file"
        ));
    }
    let new_content = content.replacen(&old_string, &new_string, 1);

    let resp = ctx.http_call("PUT", &url, &headers, &new_content)?;
    if resp.status >= 400 {
        return Err(format!("sandbox.edit({path}): write failed: {}", resp.body));
    }
    Ok(json!({"ok": true}))
}

fn sandbox_bash(
    ctx: &Context,
    sandbox_url: &str,
    api_key: &str,
    workdir: &str,
    args: &[Value],
) -> Result<Value, String> {
    let command = str_arg(args, 0, "command", "bash")?;
    let headers = sandbox_headers(api_key);
    let headers_json = sandbox_headers_json(api_key);

    let unique = format!(
        "{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let out_file = format!("/tmp/.paw-out-{unique}");
    let err_file = format!("/tmp/.paw-err-{unique}");
    let rc_file = format!("/tmp/.paw-rc-{unique}");

    // Prepend cd to set working directory — the Tensorlake processes API
    // may ignore the `cwd` field, so we enforce it in the shell command.
    let cwd = if workdir.is_empty() { "/home/tl-user" } else { workdir };
    let wrapped = format!(
        "mkdir -p {cwd} 2>/dev/null; cd {cwd} && ({command}) > {out_file} 2> {err_file}; echo $? > {rc_file}"
    );

    let body = json!({
        "command": "/bin/bash",
        "args": ["-c", &wrapped],
        "cwd": cwd,
    });
    let resp = ctx.http_call(
        "POST",
        &format!("{sandbox_url}/api/v1/processes"),
        &headers_json,
        &body.to_string(),
    )?;
    if resp.status >= 400 {
        return Err(format!("sandbox.bash(): start failed: {}", resp.body));
    }

    // Poll for exit code file — indicates process completed.
    // The rc_file is written last (after stdout/stderr), so its presence
    // guarantees all output files exist. Network latency provides natural
    // ~50-200ms backoff per attempt (same pattern as run_coding_agent).
    let max_poll = 600;
    for attempt in 0..max_poll {
        let rc_resp = ctx.http_call(
            "GET",
            &format!("{sandbox_url}/api/v1/files?path={}", urlenc(&rc_file)),
            &headers,
            "",
        );
        match rc_resp {
            Ok(r) if r.status < 400 && !r.body.trim().is_empty() => break,
            _ => {}
        }
        if attempt == max_poll - 1 {
            return Err("sandbox.bash(): command timed out waiting for completion".into());
        }
    }

    // Process completed — read output files.
    let stdout = ctx
        .http_call(
            "GET",
            &format!("{sandbox_url}/api/v1/files?path={}", urlenc(&out_file)),
            &headers,
            "",
        )
        .map(|r| r.body)
        .unwrap_or_default();
    let stderr = ctx
        .http_call(
            "GET",
            &format!("{sandbox_url}/api/v1/files?path={}", urlenc(&err_file)),
            &headers,
            "",
        )
        .map(|r| r.body)
        .unwrap_or_default();
    let exit_code = ctx
        .http_call(
            "GET",
            &format!("{sandbox_url}/api/v1/files?path={}", urlenc(&rc_file)),
            &headers,
            "",
        )
        .map(|r| r.body.trim().to_string())
        .unwrap_or_default();

    for f in [&out_file, &err_file, &rc_file] {
        let _ = ctx.http_call(
            "DELETE",
            &format!("{sandbox_url}/api/v1/files?path={}", urlenc(f)),
            &headers,
            "",
        );
    }

    let mut output = String::new();
    if !stdout.is_empty() {
        output.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str("STDERR: ");
        output.push_str(&stderr);
    }
    output.push_str(&format!("\n[exit code: {exit_code}]"));

    Ok(json!(output))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn str_arg(args: &[Value], idx: usize, name: &str, method: &str) -> Result<String, String> {
    args.get(idx)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("temper.{method}(): missing argument '{name}' at position {idx}"))
}

fn opt_str_arg(args: &[Value], idx: usize) -> Option<String> {
    args.get(idx)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn obj_arg(args: &[Value], idx: usize, name: &str, method: &str) -> Result<Value, String> {
    args.get(idx)
        .filter(|v| v.is_object())
        .cloned()
        .ok_or_else(|| {
            format!("temper.{method}(): missing object argument '{name}' at position {idx}")
        })
}

fn obj_arg_or_empty(args: &[Value], idx: usize) -> Value {
    args.get(idx)
        .filter(|v| v.is_object())
        .cloned()
        .unwrap_or(json!({}))
}

fn escape_odata_key(key: &str) -> String {
    key.replace('\'', "''")
}

fn urlenc(s: &str) -> String {
    s.replace('%', "%25")
        .replace(' ', "%20")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('?', "%3F")
        .replace('#', "%23")
        .replace('\'', "%27")
}

fn runtime_headers(tenant: &str) -> Vec<(String, String)> {
    vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        ("X-Tenant-Id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "agent".to_string()),
        ("x-temper-principal-id".to_string(), "system".to_string()),
        ("x-temper-agent-type".to_string(), "system".to_string()),
    ]
}

pub fn check_cedar_denial(status: u16, body: &str) -> Option<String> {
    if status == 403 {
        if let Ok(parsed) = serde_json::from_str::<Value>(body) {
            // Direct decision_id field (e.g. from /api/authorize)
            if let Some(did) = parsed.get("decision_id").and_then(|v| v.as_str()) {
                return Some(format!("CEDAR_DENIED:{}:{}", did, body));
            }
            // Error message formats from different endpoints:
            //   OData:       "... (decision: PD-xxx)"
            //   API (specs): "... Decision PD-xxx"
            if let Some(msg) = parsed
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
            {
                // Format 1: OData — "(decision: PD-xxx)"
                if let Some(start) = msg.find("(decision: ") {
                    let after = &msg[start + "(decision: ".len()..];
                    if let Some(end) = after.find(')') {
                        let did = &after[..end];
                        return Some(format!("CEDAR_DENIED:{}:{}", did, body));
                    }
                }
                // Format 2: API — "Decision PD-xxx" (at end of message)
                if let Some(start) = msg.find("Decision PD-") {
                    let did = msg[start + "Decision ".len()..].trim();
                    if !did.is_empty() {
                        return Some(format!("CEDAR_DENIED:{}:{}", did, body));
                    }
                }
            }
        }
    }
    None
}

fn http_get(ctx: &Context, api_url: &str, tenant: &str, path: &str) -> Result<Value, String> {
    let url = format!("{api_url}{path}");
    let headers = runtime_headers(tenant);
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if let Some(denial) = check_cedar_denial(resp.status, &resp.body) {
        return Err(denial);
    }
    if resp.status >= 400 {
        return Err(format!("HTTP GET {path}: {} {}", resp.status, resp.body));
    }
    serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse response from {path}: {e}"))
}

fn http_post(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    path: &str,
    body: &Value,
) -> Result<Value, String> {
    let url = format!("{api_url}{path}");
    let headers = runtime_headers(tenant);
    let resp = ctx.http_call("POST", &url, &headers, &body.to_string())?;
    if let Some(denial) = check_cedar_denial(resp.status, &resp.body) {
        return Err(denial);
    }
    if resp.status >= 400 {
        return Err(format!("HTTP POST {path}: {} {}", resp.status, resp.body));
    }
    if resp.body.is_empty() {
        return Ok(json!({"ok": true}));
    }
    serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse response from {path}: {e}"))
}

fn http_patch(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    path: &str,
    body: &Value,
) -> Result<Value, String> {
    let url = format!("{api_url}{path}");
    let headers = runtime_headers(tenant);
    let resp = ctx.http_call("PATCH", &url, &headers, &body.to_string())?;
    if let Some(denial) = check_cedar_denial(resp.status, &resp.body) {
        return Err(denial);
    }
    if resp.status >= 400 {
        return Err(format!("HTTP PATCH {path}: {} {}", resp.status, resp.body));
    }
    if resp.body.is_empty() {
        return Ok(json!({"ok": true}));
    }
    serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse response from {path}: {e}"))
}

fn http_delete(ctx: &Context, api_url: &str, tenant: &str, path: &str) -> Result<Value, String> {
    let url = format!("{api_url}{path}");
    let headers = runtime_headers(tenant);
    let resp = ctx.http_call("DELETE", &url, &headers, "")?;
    if let Some(denial) = check_cedar_denial(resp.status, &resp.body) {
        return Err(denial);
    }
    if resp.status >= 400 {
        return Err(format!("HTTP DELETE {path}: {} {}", resp.status, resp.body));
    }
    if resp.body.is_empty() {
        return Ok(json!({"ok": true}));
    }
    serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse response from {path}: {e}"))
}
