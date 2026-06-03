//! Provider Auth Gate — ensures subscription OAuth is fresh before LLM calls.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::{resolve_temper_api_url, runtime_headers_as, timestamp_millis_string};

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    if let Err(err) = run_provider_auth_gate() {
        set_error_result(&err);
    }
    0
}

fn run_provider_auth_gate() -> Result<(), String> {
    let ctx = Context::from_host()?;
    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    let provider = selected_provider(&ctx, &fields);
    let ready_action = config_value(&ctx, "ready_action").unwrap_or("ProviderAuthReady");

    if provider != "openai_codex" {
        set_success_result(ready_action, &ready_params(&fields, "skipped", ""));
        return Ok(());
    }

    let auth_action = config_value(&ctx, "auth_action").unwrap_or("EnsureFresh");
    let temper_api_url = resolve_temper_api_url(&ctx, &fields);
    let url = format!("{temper_api_url}{}", auth_endpoint_path(auth_action));
    let headers = runtime_headers_as(
        &ctx,
        &ctx.tenant,
        &fields,
        "system",
        Some("application/json"),
        Some("application/json"),
    );

    ctx.log(
        "info",
        &format!("provider_auth_gate: dispatching OpenAICodexAuth.{auth_action}"),
    );
    let resp = ctx.http_call("POST", &url, &headers, "{}")?;
    if !(200..300).contains(&resp.status) {
        let parsed: Value = serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({}));
        let error = auth_error_from_status(&parsed).unwrap_or_else(|| {
            format!(
                "OpenAI Codex auth {auth_action} failed (HTTP {}): {}",
                resp.status,
                body_snippet(&resp.body)
            )
        });
        if body_mentions_device_code_ready(&resp.body) {
            let prompt = fetch_current_device_login_prompt(&ctx, &temper_api_url, &headers);
            return Err(sign_in_required_message(&error, prompt));
        }
        if sign_in_required_error(&error) {
            let prompt = start_device_login_prompt(&ctx, &temper_api_url, &headers);
            return Err(sign_in_required_message(&error, prompt));
        }
        return Err(format!(
            "OpenAI Codex auth {auth_action} failed (HTTP {}): {}",
            resp.status,
            body_snippet(&resp.body)
        ));
    }

    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({}));
    let status = parsed
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("Ready");
    if auth_status_is_ready(status) {
        set_success_result(ready_action, &ready_params(&fields, status, ""));
        return Ok(());
    }

    if let Some(prompt) = device_login_prompt_from_status(&parsed) {
        return Err(sign_in_required_message(
            "OpenAI Codex sign-in is required",
            Some(prompt),
        ));
    }

    if status.eq_ignore_ascii_case("failed") {
        let error = auth_error_from_status(&parsed)
            .unwrap_or_else(|| "OpenAI Codex auth failed".to_string());
        if sign_in_required_error(&error) {
            let prompt = start_device_login_prompt(&ctx, &temper_api_url, &headers);
            return Err(sign_in_required_message(&error, prompt));
        }
        return Err(error);
    }

    Err(format!(
        "OpenAI Codex auth is {status}; sign-in or authorization may still be pending."
    ))
}

fn selected_provider(ctx: &Context, fields: &Value) -> String {
    fields
        .get("provider")
        .or_else(|| fields.get("Provider"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| config_value(ctx, "default_llm_provider"))
        .map(normalize_provider)
        .unwrap_or_default()
}

fn normalize_provider(provider: &str) -> String {
    match provider.trim().to_ascii_lowercase().as_str() {
        "codex" | "openai-codex" => "openai_codex".to_string(),
        "open_router" => "openrouter".to_string(),
        other => other.to_string(),
    }
}

fn config_value<'a>(ctx: &'a Context, key: &str) -> Option<&'a str> {
    ctx.config
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty() && !value.contains("{secret:"))
}

fn auth_endpoint_path(auth_action: &str) -> &'static str {
    match auth_action {
        "ForceRefresh" => "/paw/setup/openai-codex/force-refresh",
        "Refresh" => "/paw/setup/openai-codex/refresh",
        _ => "/paw/setup/openai-codex/ensure-fresh",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeviceLoginPrompt {
    verification_url: String,
    user_code: String,
}

fn auth_status_is_ready(status: &str) -> bool {
    status.trim().eq_ignore_ascii_case("ready")
}

fn auth_error_from_status(parsed: &Value) -> Option<String> {
    for key in ["last_error", "error_message", "error", "message"] {
        if let Some(value) = non_empty_string_value(parsed.get(key)) {
            return Some(value);
        }
    }
    None
}

fn non_empty_string_value(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(raw) if !raw.trim().is_empty() => Some(raw.trim().to_string()),
        Value::Object(map) => {
            let message = map
                .get("message")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            let code = map
                .get("code")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            match (code, message) {
                (Some(code), Some(message)) => Some(format!("{code}: {message}")),
                (Some(code), None) => Some(code.to_string()),
                (None, Some(message)) => Some(message.to_string()),
                (None, None) => None,
            }
        }
        _ => None,
    }
}

fn sign_in_required_error(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    [
        "refresh token is missing",
        "invalid_grant",
        "refresh_token_reused",
        "refresh token has already been used",
        "invalidated oauth token",
        "oauth token was invalidated",
        "token_revoked",
        "token revoked",
        "token_invalidated",
        "login_required",
        "sign-in is required",
        "sign in is required",
        "start device login first",
        "codex token refresh failed: http 401",
    ]
    .iter()
    .any(|needle| error.contains(needle))
}

fn device_login_prompt_from_status(parsed: &Value) -> Option<DeviceLoginPrompt> {
    let status = parsed.get("status").and_then(Value::as_str)?;
    if !status.eq_ignore_ascii_case("DeviceCodeReady") {
        return None;
    }

    let fields = parsed.get("fields").unwrap_or(parsed);
    let verification_url = field_from_candidates(fields, &["verification_url", "VerificationUrl"])?;
    let user_code = field_from_candidates(fields, &["user_code", "UserCode"])?;
    Some(DeviceLoginPrompt {
        verification_url,
        user_code,
    })
}

fn field_from_candidates(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .filter(|raw| !raw.trim().is_empty())
            .map(|raw| raw.trim().to_string())
    })
}

fn sign_in_required_message(error: &str, prompt: Option<DeviceLoginPrompt>) -> String {
    let error = error.trim();
    match prompt {
        Some(prompt) => format!(
            "{error}. OpenAI Codex sign-in is required. Open {} and enter code {}. After signing in, send your Discord message again.",
            prompt.verification_url, prompt.user_code
        ),
        None => format!(
            "{error}. OpenAI Codex sign-in is required, but I could not start the device login flow automatically. Open the TemperPaw setup page or call /paw/setup/openai-codex/device-login, then send your Discord message again."
        ),
    }
}

fn body_mentions_device_code_ready(body: &str) -> bool {
    body.to_ascii_lowercase().contains("devicecodeready")
}

fn fetch_current_device_login_prompt(
    ctx: &Context,
    temper_api_url: &str,
    headers: &[(String, String)],
) -> Option<DeviceLoginPrompt> {
    let url = format!("{temper_api_url}/paw/setup/openai-codex/status");
    let resp = ctx.http_call("GET", &url, headers, "").ok()?;
    if !(200..300).contains(&resp.status) {
        ctx.log(
            "warn",
            &format!(
                "provider_auth_gate: Codex auth status fetch failed HTTP {}: {}",
                resp.status,
                body_snippet(&resp.body)
            ),
        );
        return None;
    }
    serde_json::from_str::<Value>(&resp.body)
        .ok()
        .and_then(|parsed| device_login_prompt_from_status(&parsed))
}

fn start_device_login_prompt(
    ctx: &Context,
    temper_api_url: &str,
    headers: &[(String, String)],
) -> Option<DeviceLoginPrompt> {
    let url = format!("{temper_api_url}/paw/setup/openai-codex/device-login");
    let resp = ctx.http_call("POST", &url, headers, "{}").ok()?;
    if !(200..300).contains(&resp.status) {
        ctx.log(
            "warn",
            &format!(
                "provider_auth_gate: Codex device login start failed HTTP {}: {}",
                resp.status,
                body_snippet(&resp.body)
            ),
        );
        return None;
    }
    serde_json::from_str::<Value>(&resp.body)
        .ok()
        .and_then(|parsed| device_login_prompt_from_status(&parsed))
}

fn ready_params(fields: &Value, status: &str, error: &str) -> Value {
    ready_params_with_checked_at(fields, status, error, timestamp_millis_string())
}

fn ready_params_with_checked_at(
    fields: &Value,
    status: &str,
    error: &str,
    checked_at_ms: String,
) -> Value {
    json!({
        "provider_auth_status": status,
        "provider_auth_checked_at_ms": checked_at_ms,
        "provider_auth_error": error,
        "provider_auth_retry_count": retry_count(fields, "provider_auth_retry_count", "ProviderAuthRetryCount"),
        "compaction_auth_retry_count": retry_count(fields, "compaction_auth_retry_count", "CompactionAuthRetryCount"),
    })
}

fn retry_count(fields: &Value, snake_case: &str, pascal_case: &str) -> i64 {
    fields
        .get(snake_case)
        .or_else(|| fields.get(pascal_case))
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|raw| raw.parse::<i64>().ok()))
        })
        .unwrap_or(0)
}

fn body_snippet(body: &str) -> String {
    body.chars().take(300).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_provider_recognizes_codex_aliases() {
        assert_eq!(normalize_provider("codex"), "openai_codex");
        assert_eq!(normalize_provider("openai-codex"), "openai_codex");
        assert_eq!(normalize_provider("open_router"), "openrouter");
    }

    #[test]
    fn auth_endpoint_path_maps_actions_to_setup_triggers() {
        assert_eq!(
            auth_endpoint_path("EnsureFresh"),
            "/paw/setup/openai-codex/ensure-fresh"
        );
        assert_eq!(
            auth_endpoint_path("ForceRefresh"),
            "/paw/setup/openai-codex/force-refresh"
        );
        assert_eq!(
            auth_endpoint_path("Refresh"),
            "/paw/setup/openai-codex/refresh"
        );
    }

    #[test]
    fn ready_params_preserves_existing_retry_counts() {
        let fields = json!({
            "provider_auth_retry_count": 1,
            "compaction_auth_retry_count": "2"
        });
        let params = ready_params_with_checked_at(&fields, "Ready", "", "123".to_string());

        assert_eq!(
            params
                .get("provider_auth_retry_count")
                .and_then(Value::as_i64),
            Some(1)
        );
        assert_eq!(
            params
                .get("compaction_auth_retry_count")
                .and_then(Value::as_i64),
            Some(2)
        );
    }

    #[test]
    fn refresh_token_reuse_requires_human_sign_in() {
        assert!(sign_in_required_error(
            r#"OpenAI Codex token refresh failed: HTTP 401 {"error":{"code":"refresh_token_reused"}}"#
        ));
        assert!(sign_in_required_error(
            "Encountered invalidated oauth token for user"
        ));
        assert!(sign_in_required_error("token_revoked"));
    }

    #[test]
    fn device_code_ready_is_not_provider_ready() {
        assert!(auth_status_is_ready("Ready"));
        assert!(!auth_status_is_ready("DeviceCodeReady"));
        assert!(!auth_status_is_ready("Failed"));
    }

    #[test]
    fn sign_in_message_includes_device_login_details() {
        let prompt = device_login_prompt_from_status(&json!({
            "status": "DeviceCodeReady",
            "verification_url": "https://auth.openai.com/codex/device",
            "user_code": "ABCD-EFGH"
        }))
        .expect("device prompt");

        let message = sign_in_required_message("refresh_token_reused", Some(prompt));

        assert!(message.contains("https://auth.openai.com/codex/device"));
        assert!(message.contains("ABCD-EFGH"));
        assert!(!message.contains("start the Codex device login again"));
    }

    #[test]
    fn auth_error_skips_empty_last_error() {
        let parsed = json!({
            "status": "Failed",
            "last_error": "",
            "error_message": "OpenAI Codex token refresh failed"
        });

        assert_eq!(
            auth_error_from_status(&parsed).as_deref(),
            Some("OpenAI Codex token refresh failed")
        );
    }
}
