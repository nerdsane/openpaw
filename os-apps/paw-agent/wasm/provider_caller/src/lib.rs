//! Provider Caller — staged Session-turn WASM for LLM provider I/O.
//!
//! Owns the `CallingProvider` phase:
//! - resolve provider/model/api-key inputs
//! - translate the prepared artifact into provider wire formats
//! - perform outbound LLM HTTP and retries
//! - record usage/observability metadata
//! - write the provider-response artifact
//! - route to `ProviderResponseReady`
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

#[cfg(test)]
use openai_codex_wire::base64_url_no_pad;
use openai_codex_wire::{
    build_openai_headers, extract_chatgpt_account_id_from_jwt, select_openai_responses_url,
};
use session_turn_artifacts::{
    PreparedContextArtifact, ProviderResponseArtifact,
    build_provider_response_ready_params_with_inline, parse_prepared_context_artifact,
};
use temper_wasm_sdk::prelude::*;
use wasm_helpers::{
    read_content_file, resolve_temper_api_url, runtime_headers, runtime_headers_as,
    send_typing_indicator, timestamp_millis_string,
};

const DEFAULT_PROVIDER_CALLER_BUDGET_MS: i64 = 600_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProviderProgressBoundary {
    Start,
    End,
}

fn run_with_provider_progress<T>(
    mut emit_progress: impl FnMut(ProviderProgressBoundary),
    call_provider: impl FnOnce() -> T,
) -> T {
    emit_progress(ProviderProgressBoundary::Start);
    let result = call_provider();
    emit_progress(ProviderProgressBoundary::End);
    result
}

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    if let Err(err) = run_provider_caller() {
        set_error_result(&err);
    }
    0
}

/// Parsed LLM response.
struct LlmResponse {
    content: Value,
    stop_reason: String,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_input_tokens: i64,
    cache_creation_input_tokens: i64,
    request_bytes: usize,
    response_bytes: usize,
}

fn normalize_provider(provider: &str) -> String {
    let norm = provider.trim().to_ascii_lowercase();
    match norm.as_str() {
        "open_router" => "openrouter".to_string(),
        "codex" | "openai-codex" => "openai_codex".to_string(),
        _ => norm,
    }
}

fn is_unresolved_secret_template(value: &str) -> bool {
    value.contains("{secret:")
}

fn first_non_empty(values: &[Option<String>]) -> String {
    for v in values.iter().flatten() {
        if !v.trim().is_empty() {
            return v.trim().to_string();
        }
    }
    String::new()
}

fn resolve_provider_api_key(ctx: &Context, provider: &str) -> Result<String, String> {
    let key = match provider {
        "anthropic" => first_non_empty(&[
            ctx.config.get("anthropic_api_key").cloned(),
            ctx.config.get("anthropic_api_token").cloned(),
            ctx.config.get("api_key").cloned(),
        ]),
        "openai" => first_non_empty(&[
            ctx.config.get("openai_api_key").cloned(),
            ctx.config.get("api_key").cloned(),
        ]),
        "openai_codex" => first_non_empty(&[
            ctx.config.get("openai_codex_access_token").cloned(),
            ctx.config.get("openai_codex_token").cloned(),
        ]),
        "openrouter" => first_non_empty(&[
            ctx.config.get("openrouter_api_key").cloned(),
            ctx.config.get("api_key").cloned(),
        ]),
        other => return Err(format!("unsupported LLM provider: {other}")),
    };
    Ok(key)
}

fn call_mock(
    ctx: &Context,
    messages: &[Value],
    assembled_system_prompt: &str,
    _tools: &[Value],
) -> Result<LlmResponse, String> {
    ctx.log("info", "session_turn: using deterministic mock provider");
    if mock_plan_requests_hang(messages) {
        simulate_mock_hang(ctx)?;
        return Err("mock hang scenario finished without heartbeat".to_string());
    }

    let assistant_turns = messages
        .iter()
        .filter(|message| message.get("role").and_then(Value::as_str) == Some("assistant"))
        .count();

    if let Some(step) =
        extract_mock_plan(messages).and_then(|steps| steps.get(assistant_turns).cloned())
    {
        return build_mock_step_response(messages, assembled_system_prompt, assistant_turns, &step);
    }

    let latest_user = latest_user_text(messages);
    let text = resolve_mock_template(
        latest_user
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("mock provider completed"),
        assembled_system_prompt,
        latest_user.as_deref().unwrap_or(""),
    );
    Ok(mock_text_response(messages, text))
}

// Dead mock-analysis helpers removed (extract_mock_signal_summary, build_mock_analysis,
// collect_existing_dedupe_keys, lookup_string, lookup_u64, lookup_f64, normalize_key,
// humanize_issue_focus). Recoverable from git history if needed.

fn detect_anthropic_oauth_mode(_api_key: &str, auth_mode: &str) -> bool {
    match auth_mode.trim().to_ascii_lowercase().as_str() {
        "oauth" | "token" | "bearer" => true,
        "api_key" => false,
        // Auto-detect: Anthropic accepts all token formats via x-api-key header.
        // Only use Bearer if explicitly configured. Default to x-api-key.
        _ => false,
    }
}

/// Call Anthropic Messages API.
/// Hang-hint threshold for a single LLM HTTP attempt. 60 s is chosen so
/// the hint fires well before the `provider_caller` integration's 600 s
/// WASM-host timeout (see `os-apps/paw-agent/specs/session.ioa.toml`),
/// which is when the WASM host kills the module with no further logs.
const LLM_HANG_HINT_THRESHOLD_MS: i64 = 60_000;

/// True iff a single LLM HTTP attempt has already exceeded the
/// hang-hint threshold. Matches ADR-0037 Fix B.
fn should_emit_hang_hint(attempt_elapsed_ms: i64) -> bool {
    attempt_elapsed_ms >= LLM_HANG_HINT_THRESHOLD_MS
}

/// Log line emitted immediately before an LLM HTTP attempt.
fn format_llm_attempt_start_log(
    provider: &str,
    model: &str,
    attempt: u32,
    total: u32,
    total_elapsed_ms: i64,
) -> String {
    format!(
        "session_turn: {provider} attempt {attempt}/{total} start total_elapsed_ms={total_elapsed_ms} model={model}"
    )
}

/// Log line emitted after an LLM HTTP attempt returns.
fn format_llm_attempt_end_log(
    provider: &str,
    attempt: u32,
    attempt_elapsed_ms: i64,
    http_status: u16,
    body_len: usize,
) -> String {
    format!(
        "session_turn: {provider} attempt {attempt} end elapsed_ms={attempt_elapsed_ms} http_status={http_status} body_len={body_len}"
    )
}

/// Log line emitted when the LLM call completes (success or failure).
fn format_llm_complete_log(
    provider: &str,
    model: &str,
    attempts: u32,
    total_elapsed_ms: i64,
    outcome: &str,
) -> String {
    format!(
        "session_turn: {provider} complete attempts={attempts} total_elapsed_ms={total_elapsed_ms} model={model} outcome={outcome}"
    )
}

/// Warn-level hang-hint line fired when a single attempt crosses
/// `LLM_HANG_HINT_THRESHOLD_MS`. Surfaces in DD so operators see the
/// slow call before the 600 s WASM timeout kills the module silently.
fn format_llm_hang_hint(provider: &str, attempt: u32, attempt_elapsed_ms: i64) -> String {
    format!(
        "session_turn: HANG HINT {provider} attempt {attempt} took {attempt_elapsed_ms} ms (>= {} ms). \
         Upstream provider may be hung; WASM integration timeout_secs will kill the module.",
        LLM_HANG_HINT_THRESHOLD_MS
    )
}

/// Max output tokens passed to both Anthropic and OpenRouter. Kept as a
/// constant so the value is reusable as a `gen_ai.request.max_tokens`
/// span-hint attribute without drifting from the body payload.
const LLM_MAX_TOKENS: u32 = 16384;

/// Upper bound for the `gen_ai.prompt` span-hint value emitted from this
/// module. The host truncates at 20 KB; we cap slightly lower so our
/// `[truncated]` suffix lines up with the guest-level cut rather than the
/// host-level one (easier for operators reading traces to spot). Values
/// are truncated on a UTF-8 boundary.
const LLM_PROMPT_ATTR_MAX_BYTES: usize = 18 * 1024;

/// Serialize `system_prompt` + `messages` as a compact JSON object suitable
/// for the `gen_ai.prompt` span attribute, truncating at a UTF-8 boundary
/// if the payload is larger than [`LLM_PROMPT_ATTR_MAX_BYTES`]. The shape
/// (`{"system": ..., "messages": ...}`) mirrors what DD LLM Obs parsers
/// expect for OpenAI-style payloads and is also readable for Anthropic
/// (whose native shape has system separate).
fn format_gen_ai_prompt_attr(system_prompt: &str, messages: &[Value]) -> String {
    let payload = if system_prompt.is_empty() {
        json!({ "messages": messages })
    } else {
        json!({
            "system": system_prompt,
            "messages": messages,
        })
    };
    let raw = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
    if raw.len() <= LLM_PROMPT_ATTR_MAX_BYTES {
        return raw;
    }
    let mut cut = LLM_PROMPT_ATTR_MAX_BYTES;
    while cut > 0 && !raw.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}…[truncated]", &raw[..cut])
}

/// Format a post-response usage log line using OpenTelemetry
/// `gen_ai.*` semconv keys so DD's grok parser indexes them as
/// structured attributes. The `wasm_guest` tracing bridge attaches
/// the current span/trace IDs to the log, giving DD APM a clickable
/// correlation between the `tool.llm_call.*` span and these usage
/// numbers even though they cannot be set as span attributes post-hoc
/// via hint headers (ADR-0037 Fix C4).
fn format_gen_ai_usage_log(
    provider: &str,
    model: &str,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_input_tokens: i64,
    cache_creation_input_tokens: i64,
) -> String {
    format!(
        "session_turn: usage \
         gen_ai.system={provider} \
         gen_ai.request.model={model} \
         gen_ai.usage.input_tokens={input_tokens} \
         gen_ai.usage.output_tokens={output_tokens} \
         gen_ai.usage.cache_read_input_tokens={cache_read_input_tokens} \
         gen_ai.usage.cache_creation_input_tokens={cache_creation_input_tokens}"
    )
}

fn call_anthropic(
    ctx: &Context,
    api_key: &str,
    api_url: &str,
    model: &str,
    system_prompt: &str,
    messages: &[Value],
    tools: &[Value],
    anthropic_auth_mode: &str,
    temperature: f64,
) -> Result<LlmResponse, String> {
    // Detect OAuth token (sk-ant-oat-*) vs standard API key
    let is_oauth = detect_anthropic_oauth_mode(api_key, anthropic_auth_mode);

    // OAuth tokens enforce a fixed system prompt when tools are present.
    // Custom system instructions are prepended to the first user message instead.
    let (effective_system, effective_messages) = if is_oauth {
        let oauth_system = "You are Claude Code, Anthropic's official CLI for Claude.".to_string();
        let mut msgs = messages.to_vec();
        if !system_prompt.is_empty() {
            if let Some(first) = msgs.first_mut() {
                if let Some(content) = first.get("content").and_then(|v| v.as_str()) {
                    let combined = format!("[System instructions: {system_prompt}]\n\n{content}");
                    first["content"] = json!(combined);
                }
            }
        }
        (oauth_system, msgs)
    } else {
        (system_prompt.to_string(), messages.to_vec())
    };

    let mut body = json!({
        "model": model,
        "max_tokens": LLM_MAX_TOKENS,
        "messages": effective_messages,
        "temperature": temperature,
    });

    if !effective_system.is_empty() {
        body["system"] = json!([{
            "type": "text",
            "text": effective_system,
            "cache_control": {"type": "ephemeral"}
        }]);
    }

    // Add cache_control breakpoints to up to 2 recent user messages
    if let Some(msgs_arr) = body.get_mut("messages").and_then(|v| v.as_array_mut()) {
        let mut user_indices: Vec<usize> = Vec::new();
        for (i, m) in msgs_arr.iter().enumerate() {
            if m.get("role").and_then(|v| v.as_str()) == Some("user") {
                user_indices.push(i);
            }
        }
        // Take last 2 user message indices
        let breakpoints: Vec<usize> = user_indices.into_iter().rev().take(2).collect();
        for idx in breakpoints {
            let msg = &mut msgs_arr[idx];
            if let Some(content) = msg.get_mut("content") {
                if let Some(arr) = content.as_array_mut() {
                    // Array content — add cache_control to last block
                    if let Some(last) = arr.last_mut() {
                        last["cache_control"] = json!({"type": "ephemeral"});
                    }
                } else if let Some(text) = content.as_str().map(|s| s.to_string()) {
                    // String content — convert to array format with cache_control
                    msg["content"] = json!([{
                        "type": "text",
                        "text": text,
                        "cache_control": {"type": "ephemeral"}
                    }]);
                }
            }
        }
    }

    if !tools.is_empty() {
        body["tools"] = json!(tools);
    }

    let body_str =
        serde_json::to_string(&body).map_err(|e| format!("JSON serialize error: {e}"))?;

    ctx.log(
        "info",
        &format!(
            "session_turn: calling Anthropic API, model={model}, oauth={is_oauth}, messages={}, url={api_url}",
            messages.len(),
        ),
    );

    // Build auth headers — OAuth tokens use Bearer + beta header
    let mut headers = if is_oauth {
        vec![
            ("authorization".to_string(), format!("Bearer {api_key}")),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
            (
                "anthropic-beta".to_string(),
                "oauth-2025-04-20,computer-use-2025-01-24,prompt-caching-2024-07-31".to_string(),
            ),
            ("content-type".to_string(), "application/json".to_string()),
            ("user-agent".to_string(), "claude-cli/2.1.75".to_string()),
            ("x-app".to_string(), "cli".to_string()),
        ]
    } else {
        vec![
            ("x-api-key".to_string(), api_key.to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
            (
                "anthropic-beta".to_string(),
                "prompt-caching-2024-07-31".to_string(),
            ),
            ("content-type".to_string(), "application/json".to_string()),
        ]
    };
    // Span hint headers — consumed + stripped by the host's split_span_hint_headers
    // (temper-wasm, ADR-0037) so the resulting wasm.host.http_call span is
    // renamed `tool.llm_call.anthropic` and carries gen_ai.* semconv attrs.
    headers.push((
        "X-Temper-Span-Name".to_string(),
        "tool.llm_call.anthropic".to_string(),
    ));
    headers.push((
        "X-Temper-Span-Attr-gen_ai.system".to_string(),
        "anthropic".to_string(),
    ));
    headers.push((
        "X-Temper-Span-Attr-gen_ai.request.model".to_string(),
        model.to_string(),
    ));
    headers.push((
        "X-Temper-Span-Attr-gen_ai.request.temperature".to_string(),
        format!("{temperature}"),
    ));
    headers.push((
        "X-Temper-Span-Attr-gen_ai.request.max_tokens".to_string(),
        LLM_MAX_TOKENS.to_string(),
    ));
    // LLM content capture: prompt is request-side (serialize now), completion
    // is response-side (host resolves pointer against Anthropic's
    // /v1/messages response, which has {content: [{type, text}, ...]}).
    headers.push((
        "X-Temper-Span-Attr-gen_ai.prompt".to_string(),
        format_gen_ai_prompt_attr(&effective_system, &effective_messages),
    ));
    headers.push((
        "X-Temper-Span-Capture-Response-gen_ai.completion".to_string(),
        "/content/0/text".to_string(),
    ));

    // Retry on transient API errors (500, 529, and 400 with vague "Error" message).
    // Per-attempt timing added by ADR-0037 Fix B so a hung upstream surfaces
    // in DD logs long before the 600 s WASM-host timeout kills the module.
    let overall_start_ms = Context::get_time_millis();
    let mut last_err = String::new();
    let mut resp = None;
    let mut attempts_used: u32 = 0;
    for attempt in 0..5u32 {
        let attempt_num = attempt + 1;
        attempts_used = attempt_num;
        if attempt > 0 {
            ctx.log(
                "warn",
                &format!(
                    "session_turn: anthropic retrying (attempt {attempt_num}/5), last error: {last_err}"
                ),
            );
        }
        ctx.log(
            "info",
            &format_llm_attempt_start_log(
                "anthropic",
                model,
                attempt_num,
                5,
                Context::get_time_millis() - overall_start_ms,
            ),
        );
        let attempt_start_ms = Context::get_time_millis();
        match ctx.http_call("POST", api_url, &headers, &body_str) {
            Ok(r) if r.status == 200 => {
                let elapsed = Context::get_time_millis() - attempt_start_ms;
                ctx.log(
                    "info",
                    &format_llm_attempt_end_log(
                        "anthropic",
                        attempt_num,
                        elapsed,
                        r.status as u16,
                        r.body.len(),
                    ),
                );
                if should_emit_hang_hint(elapsed) {
                    ctx.log(
                        "warn",
                        &format_llm_hang_hint("anthropic", attempt_num, elapsed),
                    );
                }
                resp = Some(r);
                break;
            }
            Ok(r) if r.status == 500 || r.status == 529 => {
                let elapsed = Context::get_time_millis() - attempt_start_ms;
                ctx.log(
                    "info",
                    &format_llm_attempt_end_log(
                        "anthropic",
                        attempt_num,
                        elapsed,
                        r.status as u16,
                        r.body.len(),
                    ),
                );
                if should_emit_hang_hint(elapsed) {
                    ctx.log(
                        "warn",
                        &format_llm_hang_hint("anthropic", attempt_num, elapsed),
                    );
                }
                last_err = format!("HTTP {}: {}", r.status, &r.body[..r.body.len().min(200)]);
                continue;
            }
            Ok(r) if r.status == 400 && r.body.contains("\"message\":\"Error\"") => {
                // Transient 400 with vague error message — retry
                let elapsed = Context::get_time_millis() - attempt_start_ms;
                ctx.log(
                    "info",
                    &format_llm_attempt_end_log(
                        "anthropic",
                        attempt_num,
                        elapsed,
                        r.status as u16,
                        r.body.len(),
                    ),
                );
                if should_emit_hang_hint(elapsed) {
                    ctx.log(
                        "warn",
                        &format_llm_hang_hint("anthropic", attempt_num, elapsed),
                    );
                }
                last_err = format!("HTTP 400 (transient): {}", &r.body[..r.body.len().min(200)]);
                continue;
            }
            Ok(r) => {
                let elapsed = Context::get_time_millis() - attempt_start_ms;
                ctx.log(
                    "info",
                    &format_llm_attempt_end_log(
                        "anthropic",
                        attempt_num,
                        elapsed,
                        r.status as u16,
                        r.body.len(),
                    ),
                );
                let total_elapsed = Context::get_time_millis() - overall_start_ms;
                ctx.log(
                    "info",
                    &format_llm_complete_log(
                        "anthropic",
                        model,
                        attempts_used,
                        total_elapsed,
                        "non_retriable_http_error",
                    ),
                );
                return Err(format!(
                    "Anthropic API returned {}: {}",
                    r.status,
                    &r.body[..r.body.len().min(500)]
                ));
            }
            Err(e) => {
                let elapsed = Context::get_time_millis() - attempt_start_ms;
                ctx.log(
                    "warn",
                    &format!(
                        "session_turn: anthropic attempt {attempt_num} transport error elapsed_ms={elapsed} err={e}"
                    ),
                );
                if should_emit_hang_hint(elapsed) {
                    ctx.log(
                        "warn",
                        &format_llm_hang_hint("anthropic", attempt_num, elapsed),
                    );
                }
                last_err = e;
                continue;
            }
        }
    }
    let resp = match resp {
        Some(r) => r,
        None => {
            let total_elapsed = Context::get_time_millis() - overall_start_ms;
            ctx.log(
                "warn",
                &format_llm_complete_log(
                    "anthropic",
                    model,
                    attempts_used,
                    total_elapsed,
                    "exhausted_retries",
                ),
            );
            return Err(format!("Anthropic API failed after 5 attempts: {last_err}"));
        }
    };
    ctx.log(
        "info",
        &format_llm_complete_log(
            "anthropic",
            model,
            attempts_used,
            Context::get_time_millis() - overall_start_ms,
            "success",
        ),
    );

    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse LLM response: {e}"))?;

    let stop_reason = parsed
        .get("stop_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("end_turn")
        .to_string();

    let content = parsed.get("content").cloned().unwrap_or(json!([]));

    // Extract token usage from Anthropic response
    let usage = parsed.get("usage").cloned().unwrap_or(json!({}));
    let input_tokens = usage
        .get("input_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cache_read_input_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cache_creation_input_tokens = usage
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    ctx.log(
        "info",
        &format_gen_ai_usage_log(
            "anthropic",
            model,
            input_tokens,
            output_tokens,
            cache_read_input_tokens,
            cache_creation_input_tokens,
        ),
    );

    Ok(LlmResponse {
        content,
        stop_reason,
        input_tokens,
        output_tokens,
        cache_read_input_tokens,
        cache_creation_input_tokens,
        request_bytes: body_str.len(),
        response_bytes: resp.body.len(),
    })
}

/// Call OpenRouter Chat Completions API (OpenAI-compatible schema).
fn call_openrouter(
    ctx: &Context,
    api_key: &str,
    api_url: &str,
    model: &str,
    system_prompt: &str,
    messages: &[Value],
    tools: &[Value],
    site_url: &str,
    app_name: &str,
    temperature: f64,
) -> Result<LlmResponse, String> {
    let mut or_messages = Vec::<Value>::new();
    if !system_prompt.is_empty() {
        or_messages.push(json!({
            "role": "system",
            "content": system_prompt,
        }));
    }
    or_messages.extend(convert_messages_to_openrouter(messages));

    let openai_tools = convert_tools_to_openrouter(tools);
    let mut body = json!({
        "model": model,
        "messages": or_messages,
        "max_tokens": LLM_MAX_TOKENS,
        "temperature": temperature,
    });
    if !openai_tools.is_empty() {
        body["tools"] = json!(openai_tools);
        body["tool_choice"] = json!("auto");
    }

    let body_str =
        serde_json::to_string(&body).map_err(|e| format!("JSON serialize error: {e}"))?;

    let mut headers = vec![
        ("authorization".to_string(), format!("Bearer {api_key}")),
        ("content-type".to_string(), "application/json".to_string()),
    ];
    if !site_url.trim().is_empty() {
        headers.push(("HTTP-Referer".to_string(), site_url.trim().to_string()));
    }
    if !app_name.trim().is_empty() {
        headers.push(("X-Title".to_string(), app_name.trim().to_string()));
    }
    // Span hints (ADR-0037): stripped by the host before sending upstream.
    headers.push((
        "X-Temper-Span-Name".to_string(),
        "tool.llm_call.openrouter".to_string(),
    ));
    headers.push((
        "X-Temper-Span-Attr-gen_ai.system".to_string(),
        "openrouter".to_string(),
    ));
    headers.push((
        "X-Temper-Span-Attr-gen_ai.request.model".to_string(),
        model.to_string(),
    ));
    headers.push((
        "X-Temper-Span-Attr-gen_ai.request.temperature".to_string(),
        format!("{temperature}"),
    ));
    headers.push((
        "X-Temper-Span-Attr-gen_ai.request.max_tokens".to_string(),
        LLM_MAX_TOKENS.to_string(),
    ));
    // LLM content capture: OpenRouter/OpenAI response shape is
    // {choices: [{message: {content: "..."}}]}. Prompt is serialized
    // request-side; completion is resolved by the host post-response.
    headers.push((
        "X-Temper-Span-Attr-gen_ai.prompt".to_string(),
        format_gen_ai_prompt_attr(system_prompt, &or_messages),
    ));
    headers.push((
        "X-Temper-Span-Capture-Response-gen_ai.completion".to_string(),
        "/choices/0/message/content".to_string(),
    ));

    ctx.log(
        "info",
        &format!(
            "session_turn: calling OpenRouter API, model={model}, messages={}, url={api_url}",
            messages.len(),
        ),
    );

    // Per-attempt timing + hang hint per ADR-0037 Fix B.
    let overall_start_ms = Context::get_time_millis();
    let mut last_err = String::new();
    let mut resp = None;
    let mut attempts_used: u32 = 0;
    for attempt in 0..5u32 {
        let attempt_num = attempt + 1;
        attempts_used = attempt_num;
        if attempt > 0 {
            ctx.log(
                "warn",
                &format!(
                    "session_turn: openrouter retrying (attempt {attempt_num}/5), last error: {last_err}"
                ),
            );
        }
        ctx.log(
            "info",
            &format_llm_attempt_start_log(
                "openrouter",
                model,
                attempt_num,
                5,
                Context::get_time_millis() - overall_start_ms,
            ),
        );
        let attempt_start_ms = Context::get_time_millis();
        match ctx.http_call("POST", api_url, &headers, &body_str) {
            Ok(r) if r.status == 200 => {
                let elapsed = Context::get_time_millis() - attempt_start_ms;
                ctx.log(
                    "info",
                    &format_llm_attempt_end_log(
                        "openrouter",
                        attempt_num,
                        elapsed,
                        r.status as u16,
                        r.body.len(),
                    ),
                );
                if should_emit_hang_hint(elapsed) {
                    ctx.log(
                        "warn",
                        &format_llm_hang_hint("openrouter", attempt_num, elapsed),
                    );
                }
                resp = Some(r);
                break;
            }
            Ok(r) if matches!(r.status, 429 | 500 | 502 | 503 | 504) => {
                let elapsed = Context::get_time_millis() - attempt_start_ms;
                ctx.log(
                    "info",
                    &format_llm_attempt_end_log(
                        "openrouter",
                        attempt_num,
                        elapsed,
                        r.status as u16,
                        r.body.len(),
                    ),
                );
                if should_emit_hang_hint(elapsed) {
                    ctx.log(
                        "warn",
                        &format_llm_hang_hint("openrouter", attempt_num, elapsed),
                    );
                }
                last_err = format!("HTTP {}: {}", r.status, &r.body[..r.body.len().min(200)]);
                continue;
            }
            Ok(r) => {
                let elapsed = Context::get_time_millis() - attempt_start_ms;
                ctx.log(
                    "info",
                    &format_llm_attempt_end_log(
                        "openrouter",
                        attempt_num,
                        elapsed,
                        r.status as u16,
                        r.body.len(),
                    ),
                );
                let total_elapsed = Context::get_time_millis() - overall_start_ms;
                ctx.log(
                    "info",
                    &format_llm_complete_log(
                        "openrouter",
                        model,
                        attempts_used,
                        total_elapsed,
                        "non_retriable_http_error",
                    ),
                );
                return Err(format!(
                    "OpenRouter API returned {}: {}",
                    r.status,
                    &r.body[..r.body.len().min(500)]
                ));
            }
            Err(e) => {
                let elapsed = Context::get_time_millis() - attempt_start_ms;
                ctx.log(
                    "warn",
                    &format!(
                        "session_turn: openrouter attempt {attempt_num} transport error elapsed_ms={elapsed} err={e}"
                    ),
                );
                if should_emit_hang_hint(elapsed) {
                    ctx.log(
                        "warn",
                        &format_llm_hang_hint("openrouter", attempt_num, elapsed),
                    );
                }
                last_err = e;
                continue;
            }
        }
    }
    let resp = match resp {
        Some(r) => r,
        None => {
            let total_elapsed = Context::get_time_millis() - overall_start_ms;
            ctx.log(
                "warn",
                &format_llm_complete_log(
                    "openrouter",
                    model,
                    attempts_used,
                    total_elapsed,
                    "exhausted_retries",
                ),
            );
            return Err(format!(
                "OpenRouter API failed after 5 attempts: {last_err}"
            ));
        }
    };
    ctx.log(
        "info",
        &format_llm_complete_log(
            "openrouter",
            model,
            attempts_used,
            Context::get_time_millis() - overall_start_ms,
            "success",
        ),
    );

    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse OpenRouter response: {e}"))?;
    let choice = parsed
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .cloned()
        .unwrap_or(json!({}));
    let message = choice.get("message").cloned().unwrap_or(json!({}));

    let mut content_blocks = Vec::<Value>::new();
    let text = extract_openrouter_text(&message);
    if !text.is_empty() {
        content_blocks.push(json!({
            "type": "text",
            "text": text,
        }));
    }

    let mut has_tool_calls = false;
    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        for (idx, tc) in tool_calls.iter().enumerate() {
            let fn_name = tc
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("unknown_tool");
            let call_id = tc
                .get("id")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("or_tool_{}", idx + 1));
            let args_str = tc
                .get("function")
                .and_then(|f| f.get("arguments"))
                .and_then(Value::as_str)
                .unwrap_or("{}");
            let input = serde_json::from_str::<Value>(args_str).unwrap_or(json!({}));

            content_blocks.push(json!({
                "type": "tool_use",
                "id": call_id,
                "name": fn_name,
                "input": input,
            }));
            has_tool_calls = true;
        }
    }

    let usage = parsed.get("usage").cloned().unwrap_or(json!({}));
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_i64())
        .or_else(|| usage.get("input_tokens").and_then(|v| v.as_i64()))
        .unwrap_or(0);
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(|v| v.as_i64())
        .or_else(|| usage.get("output_tokens").and_then(|v| v.as_i64()))
        .unwrap_or(0);

    let stop_reason = if has_tool_calls {
        "tool_use".to_string()
    } else {
        "end_turn".to_string()
    };

    ctx.log(
        "info",
        &format_gen_ai_usage_log("openrouter", model, input_tokens, output_tokens, 0, 0),
    );

    Ok(LlmResponse {
        content: Value::Array(content_blocks),
        stop_reason,
        input_tokens,
        output_tokens,
        cache_read_input_tokens: 0,
        cache_creation_input_tokens: 0,
        request_bytes: body_str.len(),
        response_bytes: resp.body.len(),
    })
}

/// Extract text and image content from a tool_result content field.
/// Returns (text_output, Vec<(media_type, base64_data)>).
fn extract_text_and_images_from_tool_content(
    content: Option<&Value>,
) -> (String, Vec<(String, String)>) {
    let mut text_parts = Vec::new();
    let mut images = Vec::new();

    match content {
        Some(Value::Array(blocks)) => {
            for block in blocks {
                match block.get("type").and_then(Value::as_str).unwrap_or("") {
                    "text" => {
                        if let Some(t) = block.get("text").and_then(Value::as_str) {
                            text_parts.push(t.to_string());
                        }
                    }
                    "image" => {
                        if let Some(source) = block.get("source") {
                            let media_type = source
                                .get("media_type")
                                .and_then(Value::as_str)
                                .unwrap_or("image/png")
                                .to_string();
                            let data = source
                                .get("data")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            if !data.is_empty() {
                                images.push((media_type, data));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Some(Value::String(text)) => {
            text_parts.push(text.clone());
        }
        _ => {}
    }

    (text_parts.join("\n"), images)
}

/// Call OpenAI Codex Responses API (chatgpt.com/backend-api/codex/responses).
///
/// Uses the Responses API format (not Chat Completions): instructions, input, stream=true.
/// The WASM http_call buffers the full SSE stream — we parse the response.completed event.
fn call_openai(
    ctx: &Context,
    api_key: &str,
    api_url: &str,
    codex_account_id: Option<&str>,
    model: &str,
    system_prompt: &str,
    messages: &[Value],
    tools: &[Value],
    temperature: f64,
    provider: &str,
) -> Result<LlmResponse, String> {
    // Convert Anthropic-format messages to Responses API input format
    let pre_convert_types: Vec<String> = messages
        .iter()
        .map(|m| {
            let role = m.get("role").and_then(Value::as_str).unwrap_or("?");
            let ct = if m.get("content").and_then(Value::as_str).is_some() {
                "str".to_string()
            } else if let Some(arr) = m.get("content").and_then(Value::as_array) {
                let block_types: Vec<&str> = arr
                    .iter()
                    .filter_map(|b| b.get("type").and_then(Value::as_str))
                    .collect();
                format!("arr[{}]", block_types.join(","))
            } else {
                "?".to_string()
            };
            format!("{}:{}", role, ct)
        })
        .collect();
    ctx.log(
        "info",
        &format!(
            "session_turn: openai pre-convert messages={} types={:?}",
            messages.len(),
            pre_convert_types
        ),
    );

    let mut input = Vec::<Value>::new();
    for msg in messages {
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("user");
        match role {
            "user" => {
                if let Some(content) = msg.get("content").and_then(Value::as_str) {
                    input.push(json!({"role": "user", "content": content}));
                } else if let Some(blocks) = msg.get("content").and_then(Value::as_array) {
                    // Handle array content blocks — may contain text AND tool_result blocks
                    let mut has_tool_results = false;
                    for block in blocks {
                        let block_type = block.get("type").and_then(Value::as_str).unwrap_or("");
                        if block_type == "tool_result" {
                            // Anthropic tool_result → Responses API function_call_output
                            let call_id = block
                                .get("tool_use_id")
                                .and_then(Value::as_str)
                                .unwrap_or("");
                            let (output, images) =
                                extract_text_and_images_from_tool_content(block.get("content"));
                            input.push(json!({
                                "type": "function_call_output",
                                "call_id": call_id,
                                "output": output
                            }));
                            // Emit input_image items for each image block
                            for (media_type, data) in &images {
                                input.push(json!({
                                    "type": "input_image",
                                    "image_url": format!("data:{media_type};base64,{data}")
                                }));
                            }
                            has_tool_results = true;
                        }
                    }
                    // Also extract any text blocks (non-tool-result content)
                    if !has_tool_results {
                        let text: String = blocks
                            .iter()
                            .filter_map(|b| b.get("text").and_then(Value::as_str))
                            .collect::<Vec<_>>()
                            .join("\n");
                        if !text.is_empty() {
                            input.push(json!({"role": "user", "content": text}));
                        }
                    }
                }
            }
            "assistant" => {
                if let Some(blocks) = msg.get("content").and_then(Value::as_array) {
                    for block in blocks {
                        let block_type = block.get("type").and_then(Value::as_str).unwrap_or("");
                        match block_type {
                            "text" => {
                                if let Some(text) = block.get("text").and_then(Value::as_str) {
                                    input.push(json!({
                                        "type": "message",
                                        "role": "assistant",
                                        "content": [{"type": "output_text", "text": text}]
                                    }));
                                }
                            }
                            "tool_use" => {
                                let call_id = block
                                    .get("id")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_string();
                                let name = block
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_string();
                                let arguments =
                                    serde_json::to_string(block.get("input").unwrap_or(&json!({})))
                                        .unwrap_or_else(|_| "{}".to_string());
                                input.push(json!({
                                    "type": "function_call",
                                    "call_id": call_id,
                                    "name": name,
                                    "arguments": arguments
                                }));
                            }
                            _ => {}
                        }
                    }
                }
            }
            "tool_result" => {
                // Anthropic tool_result → Responses API function_call_output
                let tool_use_id = msg.get("tool_use_id").and_then(Value::as_str).unwrap_or("");
                let (output, images) =
                    extract_text_and_images_from_tool_content(msg.get("content"));
                input.push(json!({
                    "type": "function_call_output",
                    "call_id": tool_use_id,
                    "output": output
                }));
                for (media_type, data) in &images {
                    input.push(json!({
                        "type": "input_image",
                        "image_url": format!("data:{media_type};base64,{data}")
                    }));
                }
            }
            _ => {}
        }
    }

    // Convert tools to Responses API format
    let codex_tools: Vec<Value> = tools
        .iter()
        .map(|t| {
            let name = t.get("name").and_then(Value::as_str).unwrap_or("");
            let desc = t.get("description").and_then(Value::as_str).unwrap_or("");
            let schema = t.get("input_schema").cloned().unwrap_or(json!({}));
            json!({
                "type": "function",
                "name": name,
                "description": desc,
                "parameters": schema,
                "strict": false
            })
        })
        .collect();

    let mut body = json!({
        "model": model,
        "instructions": system_prompt,
        "input": input,
        "stream": true,
        "store": false,
        "reasoning": {
            "effort": "medium",
            "summary": "auto",
        },
    });
    // Codex API does not support the temperature parameter
    if provider != "openai_codex" {
        body["temperature"] = json!(temperature);
    }
    if !codex_tools.is_empty() {
        body["tools"] = json!(codex_tools);
        // "auto" lets the model choose text or tool calls. "required" forces
        // a tool call every turn, which creates an infinite loop when the model
        // wants to respond with text (e.g., "hello").
        body["tool_choice"] = json!("auto");
    }

    let body_str =
        serde_json::to_string(&body).map_err(|e| format!("JSON serialize error: {e}"))?;

    let headers = build_openai_headers(provider, api_key, codex_account_id);

    // Log input types for debugging conversation format issues
    let input_types: Vec<String> = input
        .iter()
        .map(|i| {
            let t = i
                .get("type")
                .and_then(Value::as_str)
                .or_else(|| i.get("role").and_then(Value::as_str))
                .unwrap_or("?");
            t.to_string()
        })
        .collect();
    // Log top-level body keys (not full body — system prompt can be huge)
    let body_keys: Vec<&str> = body
        .as_object()
        .map(|m| m.keys().map(|k| k.as_str()).collect())
        .unwrap_or_default();
    ctx.log(
        "info",
        &format!(
            "session_turn: calling OpenAI API, model={model}, input={}, types={:?}, url={api_url}, body_keys={body_keys:?}",
            input.len(),
            input_types,
        ),
    );

    let mut last_err = String::new();
    let mut output_items = Vec::<Value>::new();
    let mut usage = json!({});

    for attempt in 0..5 {
        if attempt > 0 {
            ctx.log(
                "warn",
                &format!("session_turn: OpenAI Codex retry {}/{}", attempt + 1, 5),
            );
        }
        let resp = match ctx.http_call("POST", api_url, &headers, &body_str) {
            Ok(r) if r.status >= 200 && r.status < 300 => r,
            Ok(r) if r.status == 429 => {
                last_err = format!("OpenAI Codex API rate limited (429)");
                continue;
            }
            Ok(r) => {
                let snippet = &r.body[..r.body.len().min(300)];
                ctx.log(
                    "error",
                    &format!(
                        "session_turn: OpenAI Codex API error status={} body={snippet}",
                        r.status
                    ),
                );
                return Err(format!("OpenAI Codex API returned {}: {snippet}", r.status));
            }
            Err(e) => {
                last_err = e;
                continue;
            }
        };

        // Parse SSE data payloads (newline-separated JSON lines from host).
        // The Codex endpoint streams individual events — output_item.done events
        // contain the actual tool calls and messages. response.completed may have
        // empty output (Codex strips it for bandwidth). So we accumulate output
        // items from output_item.done events and usage from response.completed.
        let body = &resp.body;
        output_items.clear();
        usage = json!({});
        let mut streamed_text = String::new();
        let mut saw_completed = false;

        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() || line == "[DONE]" {
                continue;
            }
            let json_str = line.strip_prefix("data: ").unwrap_or(line);
            if let Ok(event) = serde_json::from_str::<Value>(json_str) {
                let event_type = event.get("type").and_then(Value::as_str).unwrap_or("");
                match event_type {
                    "response.output_item.done" => {
                        if let Some(item) = event.get("item") {
                            output_items.push(item.clone());
                        }
                    }
                    "response.output_text.delta" => {
                        if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                            streamed_text.push_str(delta);
                        } else if let Some(text) = event.get("text").and_then(Value::as_str) {
                            streamed_text.push_str(text);
                        }
                    }
                    "response.output_text.done" => {
                        if let Some(text) = event.get("text").and_then(Value::as_str) {
                            if streamed_text.is_empty() {
                                streamed_text.push_str(text);
                            }
                        }
                    }
                    "response.completed" => {
                        saw_completed = true;
                        if let Some(resp) = event.get("response") {
                            if let Some(u) = resp.get("usage") {
                                usage = u.clone();
                            }
                            if let Some(out) = resp.get("output").and_then(Value::as_array) {
                                if !out.is_empty() {
                                    output_items = out.clone();
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if output_items.is_empty() {
            let trimmed = streamed_text.trim();
            if !trimmed.is_empty() {
                output_items.push(json!({
                    "type": "message",
                    "content": [{
                        "type": "output_text",
                        "text": trimmed,
                    }],
                }));
            }
        }

        if !output_items.is_empty() {
            break;
        }

        // No output items — either the stream was truncated (no response.completed
        // event) or Codex returned response.completed with empty output. Both
        // happen transiently on the Codex backend; retry up to the attempt budget
        // before giving up so a single flaky response doesn't fail the turn.
        last_err = if saw_completed {
            format!(
                "OpenAI: no output items found in {} lines ({}B) despite response.completed",
                body.lines().count(),
                body.len()
            )
        } else {
            format!(
                "SSE stream truncated: {} lines ({}B) but no response.completed event",
                body.lines().count(),
                body.len()
            )
        };
        ctx.log("warn", &format!("session_turn: {last_err}, will retry"));
        continue;
    }

    if output_items.is_empty() {
        return Err(format!(
            "OpenAI Codex API failed after 5 attempts: {last_err}"
        ));
    }

    // Build a synthetic response object for the existing parsing code
    let response = json!({
        "output": output_items,
        "usage": usage,
    });

    // Extract content and tool calls from response.output
    let mut content_blocks = Vec::<Value>::new();
    let mut has_tool_calls = false;

    if let Some(output) = response.get("output").and_then(Value::as_array) {
        for item in output {
            let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
            match item_type {
                "message" => {
                    if let Some(content) = item.get("content").and_then(Value::as_array) {
                        for part in content {
                            let part_type = part.get("type").and_then(Value::as_str).unwrap_or("");
                            if part_type == "output_text" {
                                if let Some(text) = part.get("text").and_then(Value::as_str) {
                                    if !text.is_empty() {
                                        content_blocks.push(json!({
                                            "type": "text",
                                            "text": text,
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
                "function_call" => {
                    let call_id = item
                        .get("call_id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let arguments = item
                        .get("arguments")
                        .and_then(Value::as_str)
                        .unwrap_or("{}");
                    let input = serde_json::from_str::<Value>(arguments).unwrap_or(json!({}));
                    content_blocks.push(json!({
                        "type": "tool_use",
                        "id": call_id,
                        "name": name,
                        "input": input,
                    }));
                    has_tool_calls = true;
                }
                _ => {} // reasoning, etc. — skip
            }
        }
    }

    let usage = response.get("usage").cloned().unwrap_or(json!({}));
    let input_tokens = usage
        .get("input_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let stop_reason = if has_tool_calls {
        "tool_use".to_string()
    } else {
        "end_turn".to_string()
    };

    ctx.log(
        "info",
        &format!("session_turn: OpenAI Codex response: blocks={}, stop={stop_reason}, in={input_tokens}, out={output_tokens}",
            content_blocks.len()),
    );

    Ok(LlmResponse {
        content: Value::Array(content_blocks),
        stop_reason,
        input_tokens,
        output_tokens,
        cache_read_input_tokens: 0,
        cache_creation_input_tokens: 0,
        request_bytes: body_str.len(),
        response_bytes: serde_json::to_string(&response).unwrap_or_default().len(),
    })
}

fn extract_openrouter_text(message: &Value) -> String {
    if let Some(text) = message.get("content").and_then(Value::as_str) {
        return text.to_string();
    }
    if let Some(arr) = message.get("content").and_then(Value::as_array) {
        let mut chunks = Vec::<String>::new();
        for item in arr {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                chunks.push(text.to_string());
            } else if let Some(text) = item.get("content").and_then(Value::as_str) {
                chunks.push(text.to_string());
            }
        }
        return chunks.join("\n");
    }
    String::new()
}

fn stringify_content(value: &Value) -> String {
    if let Some(s) = value.as_str() {
        s.to_string()
    } else {
        value.to_string()
    }
}

#[allow(dead_code)]
fn emit_progress_ignore(ctx: &Context, payload: Value) {
    let _ = (ctx, payload);
}

fn agent_headers(
    ctx: &Context,
    tenant: &str,
    content_type: Option<&str>,
    accept: Option<&str>,
) -> Vec<(String, String)> {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    runtime_headers(ctx, tenant, &fields, content_type, accept)
}

fn send_heartbeat(ctx: &Context, temper_api_url: &str, tenant: &str) -> Result<(), String> {
    let url = format!(
        "{temper_api_url}/tdata/Sessions('{}')/TemperPaw.Heartbeat",
        ctx.entity_id
    );
    let body = json!({ "last_heartbeat_at": timestamp_millis_string() });
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let headers = runtime_headers_as(
        ctx,
        tenant,
        &fields,
        "system",
        Some("application/json"),
        None,
    );
    let _ = ctx.http_call("POST", &url, &headers, &body.to_string())?;
    Ok(())
}

fn send_progress(ctx: &Context, temper_api_url: &str, tenant: &str) -> Result<(), String> {
    let url = format!(
        "{temper_api_url}/tdata/Sessions('{}')/TemperPaw.ProgressMade",
        ctx.entity_id
    );
    let body = json!({ "last_progress_at": timestamp_millis_string() });
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let headers = runtime_headers_as(
        ctx,
        tenant,
        &fields,
        "system",
        Some("application/json"),
        None,
    );
    let _ = ctx.http_call("POST", &url, &headers, &body.to_string())?;
    Ok(())
}

fn provider_progress_dispatch_enabled(ctx: &Context) -> bool {
    ctx.config
        .get("provider_progress_dispatch_enabled")
        .or_else(|| ctx.config.get("session_provider_progress_enabled"))
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn mock_plan_requests_hang(messages: &[Value]) -> bool {
    if let Some(steps) = extract_mock_plan(messages)
        && steps
            .iter()
            .any(|step| step.get("mode").and_then(Value::as_str) == Some("hang"))
    {
        return true;
    }
    latest_user_text(messages)
        .map(|text| text.contains("[mock-hang]"))
        .unwrap_or(false)
}

fn simulate_mock_hang(ctx: &Context) -> Result<(), String> {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let base_url = resolve_temper_api_url(ctx, &fields);
    let url = format!(
        "{base_url}/observe/entities/{}/{}/wait?statuses=__never__&timeout_ms=10000&poll_ms=250",
        ctx.entity_type, ctx.entity_id
    );
    let headers = agent_headers(ctx, &ctx.tenant, None, Some("application/json"));
    let _ = ctx.http_call("GET", &url, &headers, "")?;
    Ok(())
}

fn extract_mock_plan(messages: &[Value]) -> Option<Vec<Value>> {
    for message in messages {
        if message.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }
        let raw = stringify_content(message.get("content").unwrap_or(&Value::Null));
        let Ok(parsed) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        if let Some(steps) = parsed.get("steps").and_then(Value::as_array) {
            return Some(steps.clone());
        }
        if let Some(steps) = parsed
            .get("mock_plan")
            .and_then(|value| value.get("steps"))
            .and_then(Value::as_array)
        {
            return Some(steps.clone());
        }
    }
    None
}

fn build_mock_step_response(
    messages: &[Value],
    assembled_system_prompt: &str,
    assistant_turns: usize,
    step: &Value,
) -> Result<LlmResponse, String> {
    if step.get("mode").and_then(Value::as_str) == Some("hang") {
        return Ok(mock_text_response(
            messages,
            "mock hang placeholder".to_string(),
        ));
    }

    let mut content = Vec::<Value>::new();
    if let Some(text) = step.get("text").and_then(Value::as_str) {
        let resolved = resolve_mock_template(
            text,
            assembled_system_prompt,
            latest_user_text(messages).as_deref().unwrap_or(""),
        );
        if !resolved.is_empty() {
            content.push(json!({ "type": "text", "text": resolved }));
        }
    }

    if let Some(tool_calls) = step.get("tool_calls").and_then(Value::as_array) {
        for (index, tool_call) in tool_calls.iter().enumerate() {
            let name = tool_call
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("unknown_tool");
            let input = tool_call.get("input").cloned().unwrap_or_else(|| json!({}));
            let id = tool_call
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("mock-tool-{assistant_turns}-{index}"));
            content.push(json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input,
            }));
        }
    }

    if content
        .iter()
        .any(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
    {
        let serialized_content = serde_json::to_string(&content).unwrap_or_default();
        let output_len = serialized_content.len() as i64;
        return Ok(LlmResponse {
            content: Value::Array(content),
            stop_reason: "tool_use".to_string(),
            input_tokens: estimate_message_tokens(messages),
            output_tokens: output_len,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
            request_bytes: 0,
            response_bytes: serialized_content.len(),
        });
    }

    let final_text = step
        .get("final_text")
        .or_else(|| step.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("mock provider completed");
    Ok(mock_text_response(
        messages,
        resolve_mock_template(
            final_text,
            assembled_system_prompt,
            latest_user_text(messages).as_deref().unwrap_or(""),
        ),
    ))
}

fn mock_text_response(messages: &[Value], text: String) -> LlmResponse {
    LlmResponse {
        content: json!([{ "type": "text", "text": text.clone() }]),
        stop_reason: "end_turn".to_string(),
        input_tokens: estimate_message_tokens(messages),
        output_tokens: text.len() as i64,
        cache_read_input_tokens: 0,
        cache_creation_input_tokens: 0,
        request_bytes: 0,
        response_bytes: text.len(),
    }
}

fn estimate_message_tokens(messages: &[Value]) -> i64 {
    messages
        .iter()
        .map(|message| {
            message
                .get("content")
                .map(stringify_content)
                .unwrap_or_default()
                .len() as i64
        })
        .sum::<i64>()
}

fn latest_user_text(messages: &[Value]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find(|message| message.get("role").and_then(Value::as_str) == Some("user"))
        .map(|message| stringify_content(message.get("content").unwrap_or(&Value::Null)))
}

fn resolve_mock_template(
    template: &str,
    assembled_system_prompt: &str,
    latest_user: &str,
) -> String {
    let mut text = template.to_string();
    text = text.replace("{{latest_user}}", latest_user);
    text = text.replace(
        "{{memory_block}}",
        &extract_tag_block(assembled_system_prompt, "agent_memory"),
    );
    text = text.replace(
        "{{memory_keys}}",
        &extract_memory_keys(assembled_system_prompt).join(", "),
    );
    text = text.replace(
        "{{memory_count}}",
        &extract_memory_keys(assembled_system_prompt)
            .len()
            .to_string(),
    );
    text = text.replace(
        "{{skills_block}}",
        &extract_tag_block(assembled_system_prompt, "available_skills"),
    );
    text
}

fn extract_tag_block(text: &str, tag: &str) -> String {
    let start_tag = format!("<{tag}>");
    let end_tag = format!("</{tag}>");
    let Some(start) = text.find(&start_tag) else {
        return String::new();
    };
    let Some(end) = text[start..].find(&end_tag) else {
        return String::new();
    };
    text[start..start + end + end_tag.len()].to_string()
}

fn extract_memory_keys(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let marker = "key=\"";
            let start = line.find(marker)? + marker.len();
            let rest = &line[start..];
            let end = rest.find('"')?;
            Some(rest[..end].to_string())
        })
        .collect()
}

fn convert_messages_to_openrouter(messages: &[Value]) -> Vec<Value> {
    let mut out = Vec::<Value>::new();
    for msg in messages {
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("user");
        let content = msg.get("content").cloned().unwrap_or(json!(""));

        match content {
            Value::String(text) => {
                out.push(json!({
                    "role": role,
                    "content": text,
                }));
            }
            Value::Array(blocks) => {
                if role == "assistant" {
                    let mut text_chunks = Vec::<String>::new();
                    let mut tool_calls = Vec::<Value>::new();
                    for (idx, block) in blocks.iter().enumerate() {
                        match block.get("type").and_then(Value::as_str).unwrap_or("") {
                            "text" => {
                                if let Some(t) = block.get("text").and_then(Value::as_str) {
                                    text_chunks.push(t.to_string());
                                }
                            }
                            "tool_use" => {
                                let id = block
                                    .get("id")
                                    .and_then(Value::as_str)
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| format!("tool_{}", idx + 1));
                                let name = block
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .unwrap_or("unknown_tool");
                                let input = block.get("input").cloned().unwrap_or(json!({}));
                                tool_calls.push(json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": input.to_string(),
                                    }
                                }));
                            }
                            _ => {}
                        }
                    }

                    let mut assistant = json!({
                        "role": "assistant",
                        "content": text_chunks.join("\n"),
                    });
                    if !tool_calls.is_empty() {
                        assistant["tool_calls"] = json!(tool_calls);
                    }
                    out.push(assistant);
                } else if role == "user" {
                    let mut user_text = Vec::<String>::new();
                    for block in &blocks {
                        match block.get("type").and_then(Value::as_str).unwrap_or("") {
                            "tool_result" => {
                                let tool_call_id = block
                                    .get("tool_use_id")
                                    .and_then(Value::as_str)
                                    .unwrap_or("unknown_tool_call");
                                let (text_output, images) =
                                    extract_text_and_images_from_tool_content(block.get("content"));
                                let content = if text_output.is_empty() {
                                    stringify_content(
                                        block
                                            .get("content")
                                            .unwrap_or(&Value::String(String::new())),
                                    )
                                } else {
                                    text_output
                                };
                                out.push(json!({
                                    "role": "tool",
                                    "tool_call_id": tool_call_id,
                                    "content": content,
                                }));
                                // Inject user message with images for visual context
                                for (media_type, data) in &images {
                                    out.push(json!({
                                        "role": "user",
                                        "content": [{
                                            "type": "image_url",
                                            "image_url": {
                                                "url": format!("data:{media_type};base64,{data}")
                                            }
                                        }]
                                    }));
                                }
                            }
                            "text" => {
                                if let Some(t) = block.get("text").and_then(Value::as_str) {
                                    user_text.push(t.to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                    if !user_text.is_empty() {
                        out.push(json!({
                            "role": "user",
                            "content": user_text.join("\n"),
                        }));
                    }
                } else {
                    out.push(json!({
                        "role": role,
                        "content": Value::Array(blocks),
                    }));
                }
            }
            other => {
                out.push(json!({
                    "role": role,
                    "content": other,
                }));
            }
        }
    }
    out
}

fn convert_tools_to_openrouter(tools: &[Value]) -> Vec<Value> {
    let mut out = Vec::<Value>::new();
    for tool in tools {
        let Some(name) = tool.get("name").and_then(Value::as_str) else {
            continue;
        };
        let description = tool
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        let parameters = tool
            .get("input_schema")
            .cloned()
            .unwrap_or(json!({"type": "object", "properties": {}}));
        out.push(json!({
            "type": "function",
            "function": {
                "name": name,
                "description": description,
                "parameters": parameters,
            }
        }));
    }
    out
}

/// Build tool definitions for the LLM.
///
/// Returns a single `execute` tool for the Monty REPL. Agents write Python
/// code using `temper.*` and `sandbox.*` objects. The method listing is
/// inline in the tool description so agents see it immediately.
pub fn run_provider_caller() -> Result<(), String> {
    let started_at = Context::get_time_millis();
    let ctx = Context::from_host()?;
    ctx.log("info", "provider_caller: starting");

    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let prepared_context_file_id = fields
        .get("prepared_context_file_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let prepared_context_inline_json =
        read_state_string_field(&ctx, &fields, "prepared_context_inline_json");
    if prepared_context_file_id.is_empty() && prepared_context_inline_json.is_empty() {
        return Err(
            "provider_caller: missing prepared_context_inline_json or prepared_context_file_id"
                .to_string(),
        );
    }
    let temper_api_url = resolve_temper_api_url(&ctx, &fields);
    let tenant = &ctx.tenant;
    let provider_caller_budget_ms = configured_budget_ms(
        &ctx,
        &fields,
        "provider_caller_budget_ms",
        DEFAULT_PROVIDER_CALLER_BUDGET_MS,
    );
    let read_started_at = Context::get_time_millis();
    let prepared_result = read_prepared_context_artifact(
        &ctx,
        &temper_api_url,
        tenant,
        &fields,
        prepared_context_file_id,
        &prepared_context_inline_json,
    );
    emit_phase_step_duration(
        &ctx,
        "provider_caller",
        "read_prepared_artifact",
        read_started_at,
        if prepared_result.is_ok() {
            "ok"
        } else {
            "error"
        },
    );
    let prepared = prepared_result?;
    check_phase_budget(
        &ctx,
        "provider_caller",
        started_at,
        provider_caller_budget_ms,
        "read_prepared_artifact",
    )?;
    // Read provider/model via the blob-aware reader so we transparently
    // dereference any host-side $blob_ref that hydration left in
    // entity_state.fields. Direct `fields.get(...).as_str()` returns None on
    // blob_ref objects, which is why long-running sessions trip
    // "Session model is required" once accumulated state pushes the entity
    // past the inline ceiling.
    let provider_raw = read_state_string_field(&ctx, &fields, "provider");
    let model_raw = read_state_string_field(&ctx, &fields, "model");
    let temperature: f64 = fields
        .get("temperature")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0);
    let (provider, model, api_key) = resolve_provider_and_model(&ctx, &provider_raw, &model_raw)?;

    let anthropic_api_url = ctx
        .config
        .get("anthropic_api_url")
        .cloned()
        .unwrap_or_else(|| "https://api.anthropic.com/v1/messages".to_string());
    let openrouter_api_url = ctx
        .config
        .get("openrouter_api_url")
        .cloned()
        .unwrap_or_else(|| "https://openrouter.ai/api/v1/chat/completions".to_string());
    let openai_api_url = select_openai_responses_url(&ctx.config, &provider);
    let openai_codex_account_id = if provider == "openai_codex" {
        ctx.config
            .get("openai_codex_account_id")
            .filter(|value| !value.trim().is_empty() && !is_unresolved_secret_template(value))
            .cloned()
            .or_else(|| extract_chatgpt_account_id_from_jwt(&api_key))
    } else {
        None
    };
    if provider == "openai_codex" && openai_codex_account_id.is_none() {
        return Err(
            "openai_codex requires openai_codex_account_id or a ChatGPT OAuth token containing chatgpt_account_id"
                .to_string(),
        );
    }
    let anthropic_auth_mode = ctx
        .config
        .get("anthropic_auth_mode")
        .cloned()
        .unwrap_or_else(|| "auto".to_string());
    let openrouter_site_url = ctx
        .config
        .get("openrouter_site_url")
        .cloned()
        .unwrap_or_default();
    let openrouter_app_name = ctx
        .config
        .get("openrouter_app_name")
        .cloned()
        .unwrap_or_else(|| "temperpaw-agent".to_string());

    let mock_hang = provider == "mock" && mock_plan_requests_hang(&prepared.messages);
    if !mock_hang {
        let _ = send_heartbeat(&ctx, &temper_api_url, tenant);
    }
    let typing_agent_id = fields
        .get("agent_id")
        .or_else(|| fields.get("AgentId"))
        .and_then(|v| v.as_str())
        .unwrap_or(&ctx.entity_id);
    send_typing_indicator(&ctx, &temper_api_url, tenant, typing_agent_id);

    let provider_call_started_at = Context::get_time_millis();
    let provider_progress_enabled = provider_progress_dispatch_enabled(&ctx);
    let response_result = run_with_provider_progress(
        |boundary| {
            ctx.log(
                "debug",
                &format!(
                    "provider_caller: provider progress boundary={boundary:?} provider={provider} model={model}"
                ),
            );
            if provider_progress_enabled {
                let _ = send_progress(&ctx, &temper_api_url, tenant);
            }
        },
        || match provider.as_str() {
            "mock" => call_mock(
                &ctx,
                &prepared.messages,
                &prepared.system_prompt,
                &prepared.tools,
            ),
            "anthropic" => call_anthropic(
                &ctx,
                &api_key,
                &anthropic_api_url,
                &model,
                &prepared.system_prompt,
                &prepared.messages,
                &prepared.tools,
                &anthropic_auth_mode,
                temperature,
            ),
            "openrouter" => call_openrouter(
                &ctx,
                &api_key,
                &openrouter_api_url,
                &model,
                &prepared.system_prompt,
                &prepared.messages,
                &prepared.tools,
                &openrouter_site_url,
                &openrouter_app_name,
                temperature,
            ),
            "openai" | "openai_codex" => call_openai(
                &ctx,
                &api_key,
                &openai_api_url,
                openai_codex_account_id.as_deref(),
                &model,
                &prepared.system_prompt,
                &prepared.messages,
                &prepared.tools,
                temperature,
                &provider,
            ),
            other => Err(format!("unsupported LLM provider: {other}")),
        },
    );
    emit_phase_step_duration(
        &ctx,
        "provider_caller",
        "provider_http",
        provider_call_started_at,
        if response_result.is_ok() {
            "ok"
        } else {
            "error"
        },
    );
    let response = response_result?;
    check_phase_budget(
        &ctx,
        "provider_caller",
        started_at,
        provider_caller_budget_ms,
        "provider_http",
    )?;

    let metric_tags = session_metric_tags(&provider, &model);
    emit_metric_ignore(
        &ctx,
        "temper_session_provider_request_bytes",
        response.request_bytes as f64,
        &metric_tags,
        Some("gauge"),
    );
    emit_metric_ignore(
        &ctx,
        "temper_session_provider_response_bytes",
        response.response_bytes as f64,
        &metric_tags,
        Some("gauge"),
    );

    let artifact = ProviderResponseArtifact {
        version: 1,
        provider: provider.clone(),
        model: model.clone(),
        content: response.content,
        stop_reason: response.stop_reason,
        input_tokens: response.input_tokens,
        output_tokens: response.output_tokens,
        cache_read_input_tokens: response.cache_read_input_tokens,
        cache_creation_input_tokens: response.cache_creation_input_tokens,
        request_bytes: response.request_bytes,
        response_bytes: response.response_bytes,
    };
    let artifact_json = serde_json::to_string(&artifact)
        .map_err(|e| format!("provider response artifact serialize: {e}"))?;
    let stage_started_at = Context::get_time_millis();
    emit_phase_step_duration(
        &ctx,
        "provider_caller",
        "write_provider_response_artifact",
        stage_started_at,
        "ok",
    );
    check_phase_budget(
        &ctx,
        "provider_caller",
        started_at,
        provider_caller_budget_ms,
        "write_provider_response_artifact",
    )?;

    let params =
        build_provider_response_ready_params_with_inline("", &artifact_json, &prepared, &artifact);
    set_success_result("ProviderResponseReady", &params);
    emit_phase_total_duration(
        &ctx,
        "provider_caller",
        started_at,
        "provider_response_ready",
    );
    Ok(())
}

fn resolve_provider_and_model(
    ctx: &Context,
    provider_raw: &str,
    model_raw: &str,
) -> Result<(String, String, String), String> {
    if model_raw.trim().is_empty() {
        return Err(
            "Session model is required; configure the Agent or pass an explicit override"
                .to_string(),
        );
    }
    if provider_raw.trim().is_empty() {
        return Err(
            "Session provider is required; configure the Agent or pass an explicit override"
                .to_string(),
        );
    }
    let provider = normalize_provider(provider_raw);
    let api_key = if provider == "mock" {
        String::new()
    } else {
        let key = resolve_provider_api_key(ctx, &provider)?;
        if is_unresolved_secret_template(&key) {
            return Err(format!(
                "provider={provider} api key is unresolved secret template: '{key}'. set tenant secret and retry"
            ));
        } else {
            key
        }
    };

    let model = model_raw.trim().to_string();

    if provider != "mock" && api_key.is_empty() {
        return Err(format!("missing API key for provider={provider}"));
    }

    Ok((provider, model, api_key))
}

fn read_prepared_context_artifact(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    file_id: &str,
    inline_json: &str,
) -> Result<PreparedContextArtifact, String> {
    let raw = if inline_json.is_empty() {
        read_content_file(ctx, temper_api_url, tenant, fields, file_id)?
    } else {
        inline_json.to_string()
    };
    parse_prepared_context_artifact(&raw)
}

fn read_state_string_field(ctx: &Context, fields: &Value, field_name: &str) -> String {
    match ctx.read_field_string(field_name) {
        Ok(value) if !value.is_empty() => value,
        _ => fields
            .get(field_name)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    }
}

fn session_metric_tags(provider: &str, model: &str) -> Value {
    json!({
        "provider": provider,
        "model": model,
    })
}

fn emit_metric_ignore(ctx: &Context, name: &str, value: f64, tags: &Value, kind: Option<&str>) {
    let _ = ctx.emit_metric(name, value, tags, kind);
}

fn elapsed_ms_since(started_at: i64) -> i64 {
    Context::get_time_millis().saturating_sub(started_at)
}

fn configured_budget_ms(ctx: &Context, fields: &Value, key: &str, default_value: i64) -> i64 {
    fields
        .get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .or_else(|| ctx.config.get(key).and_then(|s| s.parse::<i64>().ok()))
        .filter(|value| *value > 0)
        .unwrap_or(default_value)
}

fn emit_phase_step_duration(
    ctx: &Context,
    phase: &str,
    step: &str,
    started_at: i64,
    result: &str,
) -> i64 {
    let elapsed_ms = elapsed_ms_since(started_at);
    emit_metric_ignore(
        ctx,
        "temper_session_phase_step_duration_ms",
        elapsed_ms as f64,
        &json!({
            "phase": phase,
            "step": step,
            "result": result,
        }),
        Some("histogram"),
    );
    ctx.log(
        "info",
        &format!("session_phase phase={phase} step={step} result={result} elapsed_ms={elapsed_ms}"),
    );
    elapsed_ms
}

fn emit_phase_total_duration(ctx: &Context, phase: &str, started_at: i64, result: &str) -> i64 {
    let elapsed_ms = elapsed_ms_since(started_at);
    emit_metric_ignore(
        ctx,
        "temper_session_phase_duration_ms",
        elapsed_ms as f64,
        &json!({
            "phase": phase,
            "result": result,
        }),
        Some("histogram"),
    );
    elapsed_ms
}

fn check_phase_budget(
    ctx: &Context,
    phase: &str,
    started_at: i64,
    budget_ms: i64,
    last_step: &str,
) -> Result<(), String> {
    let elapsed_ms = elapsed_ms_since(started_at);
    if elapsed_ms <= budget_ms {
        return Ok(());
    }

    emit_metric_ignore(
        ctx,
        "temper_session_phase_budget_exceeded_total",
        1.0,
        &json!({
            "phase": phase,
            "last_step": last_step,
        }),
        Some("count"),
    );
    Err(format!(
        "{phase}: exceeded local budget after {last_step} (elapsed_ms={elapsed_ms}, budget_ms={budget_ms})"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_progress_wrapper_emits_start_and_end_on_success() {
        let mut events = Vec::new();

        let result = run_with_provider_progress(|event| events.push(event), || Ok::<_, String>(42));

        assert_eq!(result, Ok(42));
        assert_eq!(
            events,
            vec![
                ProviderProgressBoundary::Start,
                ProviderProgressBoundary::End
            ]
        );
    }

    #[test]
    fn provider_progress_wrapper_emits_end_on_error() {
        let mut events = Vec::new();

        let result = run_with_provider_progress(
            |event| events.push(event),
            || Err::<(), _>("provider failed".to_string()),
        );

        assert_eq!(result, Err("provider failed".to_string()));
        assert_eq!(
            events,
            vec![
                ProviderProgressBoundary::Start,
                ProviderProgressBoundary::End
            ]
        );
    }

    #[test]
    fn llm_attempt_start_log_includes_provider_model_attempt_and_elapsed() {
        let msg = format_llm_attempt_start_log("anthropic", "claude-sonnet-4.6", 2, 5, 1234);
        assert!(msg.contains("anthropic"));
        assert!(msg.contains("attempt 2/5"));
        assert!(msg.contains("total_elapsed_ms=1234"));
        assert!(msg.contains("model=claude-sonnet-4.6"));
        assert!(msg.contains("start"));
    }

    #[test]
    fn llm_attempt_end_log_includes_http_status_and_body_len() {
        let msg = format_llm_attempt_end_log("openrouter", 1, 850, 200, 4096);
        assert!(msg.contains("openrouter"));
        assert!(msg.contains("attempt 1"));
        assert!(msg.contains("elapsed_ms=850"));
        assert!(msg.contains("http_status=200"));
        assert!(msg.contains("body_len=4096"));
        assert!(msg.contains("end"));
    }

    #[test]
    fn llm_complete_log_summarises_total_and_outcome() {
        let msg = format_llm_complete_log("anthropic", "claude-sonnet-4.6", 3, 9500, "success");
        assert!(msg.contains("complete"));
        assert!(msg.contains("attempts=3"));
        assert!(msg.contains("total_elapsed_ms=9500"));
        assert!(msg.contains("outcome=success"));
    }

    #[test]
    fn hang_hint_triggers_at_sixty_seconds() {
        // Below threshold — no hint.
        assert!(!should_emit_hang_hint(59_999));
        // At threshold — hint fires.
        assert!(should_emit_hang_hint(60_000));
        // Well over threshold — hint fires.
        assert!(should_emit_hang_hint(180_000));
    }

    #[test]
    fn hang_hint_message_mentions_elapsed_and_provider() {
        let msg = format_llm_hang_hint("anthropic", 1, 70_123);
        assert!(msg.to_lowercase().contains("hang"));
        assert!(msg.contains("anthropic"));
        assert!(msg.contains("attempt 1"));
        assert!(msg.contains("70123"));
    }

    #[test]
    fn gen_ai_prompt_attr_includes_system_and_messages() {
        let msgs = vec![
            json!({"role": "user", "content": "hello"}),
            json!({"role": "assistant", "content": "hi"}),
        ];
        let out = format_gen_ai_prompt_attr("you are helpful", &msgs);
        let v: Value = serde_json::from_str(&out).expect("must be valid JSON");
        assert_eq!(v["system"], "you are helpful");
        assert_eq!(v["messages"].as_array().unwrap().len(), 2);
        assert_eq!(v["messages"][0]["content"], "hello");
    }

    #[test]
    fn gen_ai_prompt_attr_omits_system_when_empty() {
        let msgs = vec![json!({"role": "user", "content": "hi"})];
        let out = format_gen_ai_prompt_attr("", &msgs);
        let v: Value = serde_json::from_str(&out).expect("must be valid JSON");
        assert!(v.get("system").is_none());
        assert_eq!(v["messages"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn gen_ai_prompt_attr_truncates_on_utf8_boundary_with_suffix() {
        // Build a huge message payload to force truncation.
        let big = "🎉".repeat(10 * 1024); // 40 KB of 4-byte chars.
        let msgs = vec![json!({"role": "user", "content": big})];
        let out = format_gen_ai_prompt_attr("", &msgs);
        assert!(out.ends_with("…[truncated]"));
        assert!(
            out.len() <= LLM_PROMPT_ATTR_MAX_BYTES + "…[truncated]".len(),
            "attr length {} exceeded expected cap",
            out.len()
        );
        // Truncation must keep the prefix valid UTF-8.
        let _ = std::str::from_utf8(out.as_bytes()).expect("must remain valid utf-8");
    }

    #[test]
    fn gen_ai_usage_log_emits_semconv_keys() {
        let msg = format_gen_ai_usage_log("anthropic", "claude-sonnet-4.6", 120, 480, 40, 80);
        // Required gen_ai semconv attributes are visible as key=value pairs
        // so DD's grok ingestion tags them on the log event.
        assert!(msg.contains("gen_ai.system=anthropic"));
        assert!(msg.contains("gen_ai.request.model=claude-sonnet-4.6"));
        assert!(msg.contains("gen_ai.usage.input_tokens=120"));
        assert!(msg.contains("gen_ai.usage.output_tokens=480"));
        assert!(msg.contains("gen_ai.usage.cache_read_input_tokens=40"));
        assert!(msg.contains("gen_ai.usage.cache_creation_input_tokens=80"));
    }

    #[test]
    fn gen_ai_usage_log_includes_human_prefix() {
        let msg = format_gen_ai_usage_log("openrouter", "anthropic/claude-sonnet-4.6", 0, 0, 0, 0);
        assert!(msg.starts_with("session_turn: usage "));
    }

    #[test]
    fn openai_codex_uses_subscription_endpoint_not_public_responses_api() {
        let mut config = std::collections::BTreeMap::new();
        config.insert(
            "openai_api_url".to_string(),
            "https://api.openai.com/v1/responses".to_string(),
        );
        config.insert(
            "openai_codex_api_url".to_string(),
            "https://chatgpt.com/backend-api/codex/responses".to_string(),
        );

        assert_eq!(
            select_openai_responses_url(&config, "openai"),
            "https://api.openai.com/v1/responses"
        );
        assert_eq!(
            select_openai_responses_url(&config, "openai_codex"),
            "https://chatgpt.com/backend-api/codex/responses"
        );
    }

    #[test]
    fn openai_codex_headers_include_chatgpt_account_and_sse_contract() {
        let headers = build_openai_headers("openai_codex", "access-token", Some("acct_123"));

        assert!(headers.contains(&(
            "authorization".to_string(),
            "Bearer access-token".to_string()
        )));
        assert!(headers.contains(&("chatgpt-account-id".to_string(), "acct_123".to_string())));
        assert!(headers.contains(&(
            "OpenAI-Beta".to_string(),
            "responses=experimental".to_string()
        )));
        assert!(headers.contains(&("accept".to_string(), "text/event-stream".to_string())));
    }

    #[test]
    fn public_openai_headers_do_not_include_codex_subscription_headers() {
        let headers = build_openai_headers("openai", "sk-test", Some("acct_123"));

        assert!(!headers.iter().any(|(name, _)| name == "chatgpt-account-id"));
        assert!(!headers.iter().any(|(name, _)| name == "OpenAI-Beta"));
        assert!(headers.contains(&("authorization".to_string(), "Bearer sk-test".to_string())));
    }

    #[test]
    fn chatgpt_account_id_is_extracted_from_codex_jwt() {
        let payload = r#"{"https://api.openai.com/auth":{"chatgpt_account_id":"acct_456"}}"#;
        let token = format!("header.{}.sig", base64_url_no_pad(payload.as_bytes()));

        assert_eq!(
            extract_chatgpt_account_id_from_jwt(&token).as_deref(),
            Some("acct_456")
        );
    }
}
