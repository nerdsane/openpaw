//! Dispatch `temper.*` and `sandbox.*` method calls to HTTP endpoints.
//!
//! When Monty code calls `temper.create("Issues", {...})`, the Monty
//! interpreter pauses and yields a `FunctionCall`. This module handles
//! that call by making an HTTP request via the WASM host function
//! `ctx.http_call()` and returning the result as JSON.
//!
//! Mirrors the method signatures from `temper-sandbox/src/dispatch.rs`
//! so agents see the exact same Python interface as `mcp__temper__execute`.

use serde_json::{json, Value};
use std::{cell::RefCell, collections::BTreeSet};
use temper_wasm_sdk::context::Context;
use wasm_helpers::runtime_headers;

const DEFAULT_TOOLS_ENABLED: &str = "temper_create,temper_get,temper_list,temper_action,temper_patch,temper_submit_specs,temper_show_spec,temper_specs,temper_upload_wasm,temper_get_trajectories,temper_get_insights,temper_get_decisions,temper_poll_decision,temper_approve_decision,temper_deny_decision,temper_submit_policy,temper_list_policies,temper_get_policy,temper_update_policy,temper_delete_policy,temper_install_app,temper_list_apps,temper_spawn_session,temper_list_sessions,temper_abort_session,temper_steer_session,temper_save_memory,temper_recall_memory,temper_write,temper_read,temper_ls,temper_grep,temper_glob,temper_edit,temper_rename,temper_search_history,temper_run_coding_agent,temper_get_secret,temper_datadog_query,temper_railway,temper_vercel,temper_web_search,temper_web_fetch,read,write,edit,bash";

/// Tools available in plan mode (ADR-004). Blocks sandbox mutation (write, edit)
/// and governance writes. Allows read ops, research, memory, Plan CRUD, and
/// TemperFS writes (for plan documents).
pub const PLAN_MODE_TOOLS: &str = "temper_create,temper_get,temper_list,temper_action,temper_specs,temper_show_spec,temper_save_memory,temper_recall_memory,temper_read,temper_write,temper_ls,temper_grep,temper_glob,temper_search_history,temper_web_search,temper_web_fetch,temper_get_trajectories,temper_get_insights,read,bash";

// Thread-local storage for the done signal. When an agent calls
// temper.done(result), the result is stored here. After all tool
// calls finish, lib.rs checks this and returns RecordResult instead
// of HandleToolResults, completing the session.
thread_local! {
    static DONE_RESULT: RefCell<Option<String>> = RefCell::new(None);
}

// Thread-local storage for lazily provisioned sandbox (ADR-0022).
// When a sandbox tool is called and no sandbox exists, we provision
// one on-demand and cache (sandbox_url, sandbox_id, provider) here.
// lib.rs reads this after tool execution to persist via HandleToolResults.
thread_local! {
    static LAZY_SANDBOX: RefCell<Option<(String, String, String)>> = RefCell::new(None);
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
/// Returns (sandbox_url, sandbox_id, provider).
pub fn take_lazy_sandbox() -> Option<(String, String, String)> {
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
    LAZY_SANDBOX.with(|cell| cell.borrow().as_ref().map(|(url, _, _)| url.clone()))
}

/// Peek at the lazily provisioned sandbox provider without consuming it.
pub fn peek_lazy_sandbox_provider() -> Option<String> {
    LAZY_SANDBOX.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|(_, _, provider)| provider.clone())
    })
}

/// Peek at the lazily provisioned sandbox ID without consuming it.
pub fn peek_lazy_sandbox_id() -> Option<String> {
    LAZY_SANDBOX.with(|cell| cell.borrow().as_ref().map(|(_, id, _)| id.clone()))
}

fn sandbox_identity_from_fields(fields: &Value) -> (Option<String>, Option<String>) {
    let lazy = LAZY_SANDBOX.with(|cell| cell.borrow().clone());

    let sandbox_id = fields
        .get("sandbox_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| lazy.as_ref().map(|(_, id, _)| id.clone()));

    let provider = fields
        .get("sandbox_provider")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| lazy.as_ref().map(|(_, _, provider)| provider.clone()));

    (sandbox_id, provider)
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
    let needs_sandbox =
        obj_name == "sandbox" || (obj_name == "temper" && method == "run_coding_agent");

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
            let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
            let (sandbox_id, cached_provider) = sandbox_identity_from_fields(&fields);
            let provider = cached_provider
                .unwrap_or_else(|| {
                    wasm_helpers::sandbox::resolve_sandbox_provider(ctx, &fields)
                        .unwrap_or_else(|_| "tensorlake".to_string())
                });
            dispatch_sandbox(
                ctx,
                &effective_sandbox_url,
                sandbox_id.as_deref().unwrap_or(""),
                &provider,
                workdir,
                method,
                args,
            )
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
            // sandbox_* prefixed aliases → bare tokens (spec canonical form)
            "sandbox_bash" | "sandbox_exec" => "bash",
            "sandbox_read" => "read",
            "sandbox_write" => "write",
            "sandbox_edit" => "edit",
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
        "create_app" => Some("temper_create_app"),
        "list_apps" => Some("temper_list_apps"),
        "spawn_session" => Some("temper_spawn_session"),
        "list_sessions" => Some("temper_list_sessions"),
        "abort_session" => Some("temper_abort_session"),
        "steer_session" => Some("temper_steer_session"),
        "save_memory" => Some("temper_save_memory"),
        "recall_memory" => Some("temper_recall_memory"),
        "write" => Some("temper_write"),
        "read" => Some("temper_read"),
        "ls" => Some("temper_ls"),
        "grep" => Some("temper_grep"),
        "glob" => Some("temper_glob"),
        "edit" => Some("temper_edit"),
        "rename" => Some("temper_rename"),
        "search_history" => Some("temper_search_history"),
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
        "create_app" => temper_create_app(ctx, api_url, tenant, args),
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
            let input = args
                .first()
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default();
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
            let headers = internal_headers();
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
                Err(format!(
                    "SwitchProvider failed (HTTP {}): {}",
                    resp.status,
                    &resp.body[..resp.body.len().min(200)]
                ))
            }
        }

        // Switch session mode between plan and execute (ADR-004)
        "switch_mode" => {
            let input = args
                .first()
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default();
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
            let headers = internal_headers();
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
        "ls" => super::entity_ops::ls(ctx, api_url, tenant, args),
        "grep" => super::entity_ops::grep(ctx, api_url, tenant, args),
        "glob" => super::entity_ops::glob_files(ctx, api_url, tenant, args),
        "edit" => super::entity_ops::edit(ctx, api_url, tenant, args),
        "rename" => super::entity_ops::rename(ctx, api_url, tenant, args),
        "search_history" => super::entity_ops::search_history(ctx, api_url, tenant, args),
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
             save_memory, recall_memory, write, read, ls, grep, glob, edit, rename, \
             search_history, run_coding_agent, datadog_query, railway, vercel, \
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
    let spec_names: Vec<String> = specs
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    let has_model = has_model_csdl(&spec_names);
    if !has_model {
        return Err(
            "temper.submit_specs requires a spec bundle containing model.csdl.xml. \
             Include model.csdl.xml plus one or more *.ioa.toml files; nested paths are allowed."
                .to_string(),
        );
    }
    // Validate all spec values are strings (LoadInlineRequest expects BTreeMap<String, String>).
    if let Some(obj) = specs.as_object() {
        for (key, val) in obj {
            if !val.is_string() {
                let vtype = if val.is_object() {
                    "object"
                } else if val.is_array() {
                    "array"
                } else if val.is_number() {
                    "number"
                } else if val.is_boolean() {
                    "boolean"
                } else {
                    "non-string"
                };
                return Err(format!(
                    "temper.submit_specs(): spec value for '{}' must be a TOML/XML/JSON string, \
                     not a {} value. Wrap the content in a string.",
                    key, vtype
                ));
            }
        }
    }
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

fn has_model_csdl(spec_names: &[String]) -> bool {
    spec_names
        .iter()
        .any(|name| name == "model.csdl.xml" || name.ends_with("/model.csdl.xml"))
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
    let primary_result = web_query_dispatch(ctx, api_url, tenant, "search", &query, "")?;
    if !web_search_results_empty(&primary_result) {
        return Ok(primary_result);
    }

    if let Some(retry_query) =
        fallback_web_search_query(&query, &recent_user_messages(ctx, api_url, tenant, 8))
    {
        ctx.log(
            "info",
            &format!("web_search: retrying vague zero-result query '{query}' as '{retry_query}'"),
        );

        let retry_result = web_query_dispatch(ctx, api_url, tenant, "search", &retry_query, "")?;
        if !web_search_results_empty(&retry_result) {
            return Ok(retry_result);
        }
    }

    Ok(primary_result)
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
    let result = web_query_dispatch(ctx, api_url, tenant, "fetch", "", &url)?;
    if web_search_results_empty(&result) {
        return Err(format!(
            "web_fetch: fetched no readable content from {url}; try a more specific page or search first"
        ));
    }
    Ok(result)
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
    http_post(
        ctx,
        api_url,
        tenant,
        &format!("/tdata/WebQueries('{key}')/Temper.{action_name}?await_integration=true"),
        &action_params,
    )?;

    // 3. Read the entity back — WASM integration has run by this point.
    // OData GET hydrates blob refs transparently (temper ADR-0040), so the
    // `results` field arrives fully resolved regardless of size. The prior
    // TemperFS File workaround (result_file_id + delete-after-read) is
    // retired — see temper ADR-0045 / ADR-0046 and openpaw ADR-0033.
    let result = http_get(ctx, api_url, tenant, &format!("/tdata/WebQueries('{key}')"))?;
    let result_fields = result.get("fields").cloned().unwrap_or(result.clone());
    let (status, results_raw) = interpret_web_query_entity_result(query_type, &result_fields)?;

    // Try to parse as JSON array; if not, return as plain text
    match serde_json::from_str::<Value>(results_raw) {
        Ok(parsed) => {
            ctx.log(
                "info",
                &format!("web_query: {query_type} completed with status {status}"),
            );
            Ok(parsed)
        }
        Err(_) => {
            ctx.log(
                "info",
                &format!("web_query: {query_type} completed with status {status}"),
            );
            Ok(json!(results_raw))
        }
    }
}

const GENERIC_SEARCH_TOKENS: &[&str] = &[
    "a",
    "an",
    "and",
    "as",
    "before",
    "can",
    "could",
    "do",
    "does",
    "easily",
    "famous",
    "find",
    "for",
    "from",
    "github",
    "it",
    "just",
    "look",
    "of",
    "open",
    "please",
    "repo",
    "repository",
    "same",
    "search",
    "super",
    "that",
    "the",
    "this",
    "up",
    "use",
    "web",
    "you",
    "your",
];

const SUBJECT_CONTEXT_STOPWORDS: &[&str] = &[
    "asked", "clone", "cloned", "don't", "dont", "find", "look", "remember", "search", "searched",
    "use", "using", "web",
];

fn web_search_results_empty(result: &Value) -> bool {
    match result {
        Value::Array(items) => items.is_empty(),
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return true;
            }
            serde_json::from_str::<Value>(trimmed)
                .ok()
                .is_some_and(|parsed| matches!(parsed, Value::Array(ref items) if items.is_empty()))
        }
        _ => false,
    }
}

fn interpret_web_query_entity_result<'a>(
    query_type: &str,
    fields: &'a Value,
) -> Result<(&'a str, &'a str), String> {
    let status = fields
        .get("Status")
        .or_else(|| fields.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match status {
        "Complete" => {}
        "Failed" => {
            let error = fields
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(format!("web_{query_type}: {error}"));
        }
        other => {
            return Err(format!(
                "web_{query_type}: query never completed (status={})",
                if other.is_empty() { "unknown" } else { other }
            ));
        }
    }

    let results_raw = fields
        .get("results")
        .and_then(|v| v.as_str())
        .unwrap_or("[]");

    Ok((status, results_raw))
}

fn fallback_web_search_query(query: &str, recent_user_messages: &[String]) -> Option<String> {
    if !is_vague_web_search_query(query) {
        return None;
    }

    let query_lower = query.to_lowercase();
    let explicit_subject = recent_user_messages
        .iter()
        .rev()
        .find_map(|message| extract_explicit_repo_subject(message));

    let fallback_subject = explicit_subject.or_else(|| {
        recent_user_messages
            .iter()
            .rev()
            .find_map(|message| extract_search_subject(message))
    })?;

    if query_lower.contains(&fallback_subject.to_lowercase()) {
        return None;
    }

    Some(format!("{fallback_subject} github repo"))
}

fn is_vague_web_search_query(query: &str) -> bool {
    let tokens = search_tokens(query);
    !tokens.iter().any(|token| !is_generic_search_token(token))
}

fn extract_search_subject(message: &str) -> Option<String> {
    extract_explicit_repo_subject(message).or_else(|| {
        search_tokens(message).into_iter().rev().find(|token| {
            !is_generic_search_token(token) && !SUBJECT_CONTEXT_STOPWORDS.contains(&token.as_str())
        })
    })
}

fn extract_explicit_repo_subject(message: &str) -> Option<String> {
    if let Some(repo) = extract_github_repo(message) {
        return Some(repo);
    }

    let tokens = search_tokens(message);
    if tokens.is_empty() {
        return None;
    }

    for window in tokens.windows(2) {
        if matches!(window[1].as_str(), "repo" | "repository")
            && !is_generic_search_token(&window[0])
            && !SUBJECT_CONTEXT_STOPWORDS.contains(&window[0].as_str())
        {
            return Some(window[0].clone());
        }
    }

    None
}

fn extract_github_repo(message: &str) -> Option<String> {
    let marker = "github.com/";
    let start = message.find(marker)?;
    let remainder = &message[start + marker.len()..];
    let repo = remainder
        .split(|ch: char| ch.is_whitespace() || matches!(ch, '?' | '#' | ')' | ']' | '"' | '\''))
        .next()?
        .trim_matches('/');
    if repo.is_empty() {
        return None;
    }

    let repo_name = repo.rsplit('/').next()?.trim();
    if repo_name.is_empty() {
        return None;
    }
    Some(repo_name.to_string())
}

fn search_tokens(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(clean_search_token)
        .filter(|token| !token.is_empty())
        .collect()
}

fn clean_search_token(raw: &str) -> String {
    raw.trim_matches(|ch: char| {
        !(ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-' | '_' | '.'))
    })
    .to_lowercase()
}

fn is_generic_search_token(token: &str) -> bool {
    token.len() < 3 || GENERIC_SEARCH_TOKENS.contains(&token)
}

fn recent_user_messages(ctx: &Context, api_url: &str, tenant: &str, limit: usize) -> Vec<String> {
    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    let session_file_id = fields
        .get("session_file_id")
        .and_then(|value| value.as_str())
        .unwrap_or("");

    let mut messages = Vec::new();
    if let Some(current_user_message) = fields.get("user_message").and_then(Value::as_str) {
        if !current_user_message.trim().is_empty() {
            messages.push(current_user_message.trim().to_string());
        }
    }

    if session_file_id.is_empty() {
        return messages;
    }

    let headers = runtime_headers(ctx, tenant, &fields, None, Some("text/plain"));
    let file_url = format!("{api_url}/tdata/Files('{session_file_id}')/$value");
    let Ok(resp) = ctx.http_call("GET", &file_url, &headers, "") else {
        return messages;
    };
    if resp.status >= 400 {
        return messages;
    }

    let tree = session_tree_lib::SessionTree::from_jsonl(&resp.body);
    for entry_id in tree.entry_ids() {
        let Some(entry) = tree.get(entry_id) else {
            continue;
        };
        if entry.data.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }
        let content = extract_search_entry_text(&entry.data);
        if !content.trim().is_empty() {
            messages.push(content.trim().to_string());
        }
    }

    if messages.len() > limit {
        messages.drain(0..messages.len().saturating_sub(limit));
    }
    messages
}

fn extract_search_entry_text(data: &Value) -> String {
    if let Some(text) = data.get("content").and_then(Value::as_str) {
        return text.to_string();
    }
    if let Some(arr) = data.get("content").and_then(Value::as_array) {
        let parts: Vec<String> = arr
            .iter()
            .filter_map(|block| {
                block
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect();
        if !parts.is_empty() {
            return parts.join("\n");
        }
    }
    if let Some(summary) = data.get("summary").and_then(Value::as_str) {
        return summary.to_string();
    }
    String::new()
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

/// Create an app at runtime by bundling specs, policies, and WASM modules inline.
/// This bypasses the on-disk catalog — the capability_installer processes the bundle
/// using its `cap_type="app"` handler.
///
/// Input (single object arg):
///   { name, specs: {filename: content, ...}, policies?: {filename: content, ...},
///     wasm_modules?: {name: base64_bytes, ...}, reason? }
fn temper_create_app(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let input = args.first().ok_or_else(|| {
        "temper.create_app() requires an object argument with at least 'name' and 'specs'"
            .to_string()
    })?;

    let app_name = input
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "temper.create_app(): 'name' (string) is required".to_string())?;
    let specs = input.get("specs").ok_or_else(|| {
        "temper.create_app(): 'specs' (object of filename→content strings) is required".to_string()
    })?;
    let policies = input.get("policies").cloned().unwrap_or(json!({}));
    let wasm_modules = input.get("wasm_modules").cloned().unwrap_or(json!({}));
    let reason = input.get("reason").and_then(|v| v.as_str()).unwrap_or("");

    // Validate specs values are strings (same check as temper_submit_specs).
    if let Some(obj) = specs.as_object() {
        for (key, val) in obj {
            if !val.is_string() {
                let vtype = if val.is_object() {
                    "object"
                } else if val.is_array() {
                    "array"
                } else if val.is_number() {
                    "number"
                } else if val.is_boolean() {
                    "boolean"
                } else {
                    "non-string"
                };
                return Err(format!(
                    "temper.create_app(): spec value for '{}' must be a string, not a {} value.",
                    key, vtype
                ));
            }
        }
    }

    // Build the bundle that capability_installer's app handler expects.
    let bundle = json!({
        "specs": specs,
        "policies": policies,
        "wasm_modules": wasm_modules,
    });
    let payload = serde_json::to_string(&bundle)
        .map_err(|e| format!("temper.create_app(): failed to serialize bundle: {e}"))?;

    let agent_id = ctx
        .entity_state
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let body = json!({
        "CapabilityType": "app",
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

/// Provision a sandbox on-demand via the provider abstraction. Called when a
/// sandbox tool is invoked but no sandbox_url is set on the session.
///
/// Returns the sandbox_url on success. Caches (sandbox_url, sandbox_id, provider)
/// in the LAZY_SANDBOX thread-local so subsequent tool calls in the same
/// invocation reuse it, and lib.rs persists it via HandleToolResults params.
fn lazy_provision_sandbox(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
) -> Result<String, String> {
    use wasm_helpers::sandbox::{self, sandbox_config_from_fields};

    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    let provider = sandbox::resolve_sandbox_provider(ctx, &fields)?;

    ctx.log(
        "info",
        &format!("lazy_provision_sandbox: provisioning via {provider} provider"),
    );

    // Send heartbeat so the user sees a typing indicator
    super::session::send_heartbeat(ctx, temper_api_url, tenant);

    // Create sandbox
    let config = sandbox_config_from_fields(&fields);
    let handle = sandbox::sandbox_create(ctx, &provider, &config)?;

    // Poll for readiness (max 12 retries = ~60s)
    let max_checks = 12;
    for attempt in 0..max_checks {
        match sandbox::sandbox_health_check(ctx, &handle) {
            Ok(true) => {
                ctx.log(
                    "info",
                    &format!(
                        "lazy_provision_sandbox: sandbox ready after {} checks: id={}, provider={}",
                        attempt + 1,
                        handle.sandbox_id,
                        handle.provider
                    ),
                );

                // Run post-provisioning setup (gh CLI etc.) — non-fatal
                sandbox::sandbox_setup(ctx, &handle);

                // Cache in thread-local
                LAZY_SANDBOX.with(|cell| {
                    *cell.borrow_mut() = Some((
                        handle.sandbox_url.clone(),
                        handle.sandbox_id.clone(),
                        handle.provider.clone(),
                    ));
                });

                return Ok(handle.sandbox_url);
            }
            Ok(false) => {
                ctx.log(
                    "info",
                    &format!(
                        "lazy_provision_sandbox: sandbox not ready, check {}/{}",
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
        "sandbox {} did not become ready within {max_checks} readiness checks. \
         The sandbox may still be booting — try again in a moment.",
        handle.sandbox_id
    ))
}

// ---------------------------------------------------------------------------
// Sandbox dispatch (via provider abstraction)
// ---------------------------------------------------------------------------

fn dispatch_sandbox(
    ctx: &Context,
    sandbox_url: &str,
    sandbox_id: &str,
    provider: &str,
    workdir: &str,
    method: &str,
    args: &[Value],
) -> Result<Value, String> {
    use wasm_helpers::sandbox::{self, SandboxHandle};

    if sandbox_url.is_empty() {
        return Err(format!("sandbox.{method}(): no sandbox attached"));
    }

    let handle = SandboxHandle {
        sandbox_url: sandbox_url.to_string(),
        sandbox_id: sandbox_id.to_string(),
        provider: provider.to_string(),
    };

    match method {
        "read" => {
            let path = str_arg(args, 0, "path", "read")?;
            if is_image_extension(&path) {
                // Binary-safe read: base64-encode in sandbox to avoid UTF-8 corruption
                let b64_cmd = format!(
                    "base64 -w0 {} 2>/dev/null || base64 {}",
                    shell_quote(&path),
                    shell_quote(&path)
                );
                let result = sandbox::sandbox_exec(ctx, &handle, &b64_cmd, "/")?;
                if result.exit_code != 0 {
                    return Err(format!(
                        "sandbox.read({path}): failed to read image (exit {}): {}",
                        result.exit_code, result.stderr
                    ));
                }
                let b64_data = result.stdout.trim().to_string();
                if b64_data.is_empty() {
                    return Err(format!("sandbox.read({path}): image file is empty"));
                }
                let media_type = media_type_from_extension(&path);
                Ok(json!({
                    "__openpaw_image": true,
                    "media_type": media_type,
                    "base64_data": b64_data,
                    "source_path": path
                }))
            } else {
                let content = sandbox::sandbox_file_read(ctx, &handle, &path)?;
                Ok(json!(content))
            }
        }
        "write" => {
            let path = str_arg(args, 0, "path", "write")?;
            let content = str_arg(args, 1, "content", "write")?;
            sandbox::sandbox_file_write(ctx, &handle, &path, &content)?;
            Ok(json!({"ok": true}))
        }
        "edit" => sandbox_edit(ctx, &handle, args),
        "bash" => {
            let command = str_arg(args, 0, "command", "bash")?;
            let result = sandbox::sandbox_exec(ctx, &handle, &command, workdir)?;
            let mut output = String::new();
            if !result.stdout.is_empty() {
                output.push_str(&result.stdout);
            }
            if !result.stderr.is_empty() {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str("STDERR: ");
                output.push_str(&result.stderr);
            }
            output.push_str(&format!("\n[exit code: {}]", result.exit_code));
            Ok(json!(output))
        }
        _ => Err(format!(
            "unknown sandbox method '{method}'. Available: read, write, edit, bash"
        )),
    }
}

/// Edit a file via read-modify-write (consumer-level operation, not provider-level).
fn sandbox_edit(
    ctx: &Context,
    handle: &wasm_helpers::sandbox::SandboxHandle,
    args: &[Value],
) -> Result<Value, String> {
    let path = str_arg(args, 0, "path", "edit")?;
    let old_string = str_arg(args, 1, "old_string", "edit")?;
    let new_string = str_arg(args, 2, "new_string", "edit")?;

    let content = wasm_helpers::sandbox::sandbox_file_read(ctx, handle, &path)?;
    if !content.contains(&old_string) {
        return Err(format!(
            "sandbox.edit({path}): old_string not found in file"
        ));
    }
    let new_content = content.replacen(&old_string, &new_string, 1);
    wasm_helpers::sandbox::sandbox_file_write(ctx, handle, &path, &new_content)?;
    Ok(json!({"ok": true}))
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

/// Minimal headers for internal Temper API calls.
/// Auth headers are injected by the WASM host — see ADR-0043.
fn internal_headers() -> Vec<(String, String)> {
    vec![("Content-Type".to_string(), "application/json".to_string())]
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

fn http_get(ctx: &Context, api_url: &str, _tenant: &str, path: &str) -> Result<Value, String> {
    let url = format!("{api_url}{path}");
    let headers = internal_headers();
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
    _tenant: &str,
    path: &str,
    body: &Value,
) -> Result<Value, String> {
    let url = format!("{api_url}{path}");
    let headers = internal_headers();
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
    _tenant: &str,
    path: &str,
    body: &Value,
) -> Result<Value, String> {
    let url = format!("{api_url}{path}");
    let headers = internal_headers();
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

fn http_delete(ctx: &Context, api_url: &str, _tenant: &str, path: &str) -> Result<Value, String> {
    let url = format!("{api_url}{path}");
    let headers = internal_headers();
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

// ---------------------------------------------------------------------------
// Image file helpers
// ---------------------------------------------------------------------------

fn is_image_extension(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
}

fn media_type_from_extension(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::{
        fallback_web_search_query, has_model_csdl, interpret_web_query_entity_result,
        is_image_extension, media_type_from_extension, is_vague_web_search_query,
        sandbox_identity_from_fields, web_search_results_empty, LAZY_SANDBOX,
    };
    use serde_json::json;

    #[test]
    fn detects_model_csdl_at_root() {
        assert!(has_model_csdl(&["model.csdl.xml".to_string()]));
    }

    #[test]
    fn detects_model_csdl_in_nested_bundle() {
        assert!(has_model_csdl(&[
            "InlineProbe/model.csdl.xml".to_string(),
            "InlineProbe/order.ioa.toml".to_string(),
        ]));
    }

    #[test]
    fn rejects_bundle_without_model_csdl() {
        assert!(!has_model_csdl(&["bookmark.ioa.toml".to_string()]));
    }

    #[test]
    fn vague_web_search_query_detects_meta_follow_ups() {
        assert!(is_vague_web_search_query("super famous"));
        assert!(is_vague_web_search_query("that repo"));
        assert!(!is_vague_web_search_query("openclaw github repo"));
    }

    #[test]
    fn fallback_web_search_query_uses_recent_repo_subject() {
        let retry = fallback_web_search_query(
            "super famous",
            &[
                "Can you clone openclaw repo".to_string(),
                "I asked you to clone openclaw you don't remember?".to_string(),
            ],
        );

        assert_eq!(retry.as_deref(), Some("openclaw github repo"));
    }

    #[test]
    fn fallback_web_search_query_ignores_specific_queries() {
        let retry = fallback_web_search_query(
            "openclaw github repo",
            &["Can you clone openclaw repo".to_string()],
        );

        assert_eq!(retry, None);
    }

    #[test]
    fn interpret_web_query_entity_result_requires_complete_status() {
        let err = interpret_web_query_entity_result(
            "search",
            &json!({
                "status": "Created",
                "results": "[]",
            }),
        )
        .unwrap_err();

        assert_eq!(err, "web_search: query never completed (status=Created)");
    }

    #[test]
    fn interpret_web_query_entity_result_surfaces_recorded_errors() {
        let err = interpret_web_query_entity_result(
            "fetch",
            &json!({
                "status": "Failed",
                "error": "missing exa_api_key",
            }),
        )
        .unwrap_err();

        assert_eq!(err, "web_fetch: missing exa_api_key");
    }

    #[test]
    fn web_search_results_empty_treats_blank_strings_as_empty() {
        assert!(web_search_results_empty(&json!("")));
        assert!(web_search_results_empty(&json!("[]")));
        assert!(!web_search_results_empty(&json!("headline text")));
    }

    #[test]
    fn test_is_image_extension() {
        assert!(is_image_extension("/tmp/screenshot.png"));
        assert!(is_image_extension("/tmp/photo.JPG"));
        assert!(is_image_extension("file.jpeg"));
        assert!(is_image_extension("file.gif"));
        assert!(is_image_extension("file.webp"));
        assert!(!is_image_extension("/tmp/data.txt"));
        assert!(!is_image_extension("/tmp/code.rs"));
        assert!(!is_image_extension("/tmp/doc.pdf"));
        assert!(!is_image_extension("no_extension"));
    }

    #[test]
    fn test_media_type_from_extension() {
        assert_eq!(media_type_from_extension("file.png"), "image/png");
        assert_eq!(media_type_from_extension("file.jpg"), "image/jpeg");
        assert_eq!(media_type_from_extension("file.jpeg"), "image/jpeg");
        assert_eq!(media_type_from_extension("file.gif"), "image/gif");
        assert_eq!(media_type_from_extension("file.webp"), "image/webp");
        assert_eq!(
            media_type_from_extension("file.bmp"),
            "application/octet-stream"
        );
    }

    #[test]
    fn sandbox_identity_uses_lazy_cache_when_entity_state_is_empty() {
        LAZY_SANDBOX.with(|cell| {
            *cell.borrow_mut() = Some((
                "https://sandbox.example".to_string(),
                "sb-lazy".to_string(),
                "modal".to_string(),
            ));
        });

        let (sandbox_id, provider) = sandbox_identity_from_fields(&json!({}));

        assert_eq!(sandbox_id.as_deref(), Some("sb-lazy"));
        assert_eq!(provider.as_deref(), Some("modal"));

        LAZY_SANDBOX.with(|cell| *cell.borrow_mut() = None);
    }

    #[test]
    fn sandbox_identity_prefers_persisted_entity_state_over_lazy_cache() {
        LAZY_SANDBOX.with(|cell| {
            *cell.borrow_mut() = Some((
                "https://sandbox.example".to_string(),
                "sb-lazy".to_string(),
                "modal".to_string(),
            ));
        });

        let (sandbox_id, provider) = sandbox_identity_from_fields(&json!({
            "sandbox_id": "sb-persisted",
            "sandbox_provider": "tensorlake"
        }));

        assert_eq!(sandbox_id.as_deref(), Some("sb-persisted"));
        assert_eq!(provider.as_deref(), Some("tensorlake"));

        LAZY_SANDBOX.with(|cell| *cell.borrow_mut() = None);
    }
}
