//! Validate Webhook — WASM module for validating incoming webhook payloads.
//!
//! Triggered by WebhookEvent.Received action. Looks up the WebhookRoute by
//! route_key and, when the route declares a signing secret, verifies the
//! request's `HMAC-SHA256(secret, raw_body)` signature before letting the
//! payload proceed. Fails closed — a route with a configured secret and a
//! missing, unresolvable, or mismatched signature transitions to
//! ValidationFailed and is never dispatched (ARN-168 / Class B). Routes
//! without a secret skip verification and transition to Validated.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "validate_webhook: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // Read route_key from trigger params (set by Received action)
        let route_key = fields
            .get("route_key")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if route_key.is_empty() {
            set_success_result("ValidationFailed", &json!({
                "validation_error": "route_key is empty"
            }));
            return Ok(());
        }

        let raw_payload = fields
            .get("raw_payload")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let raw_headers = fields
            .get("raw_headers")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Resolve Temper API URL from config
        let temper_api_url = resolve_api_url(&ctx);

        let tenant = &ctx.tenant;
        let headers = odata_headers(&ctx, tenant);

        // Query WebhookRoutes by route_key and Active status
        let filter = format!(
            "route_key eq '{}' and Status eq 'Active'",
            route_key.replace('\'', "''")
        );
        let query_url = format!(
            "{}/tdata/WebhookRoutes?$filter={}",
            temper_api_url,
            urlencoded(&filter)
        );

        ctx.log("info", &format!("validate_webhook: querying routes: {query_url}"));

        let resp = ctx.http_call("GET", &query_url, &headers, "")?;
        if resp.status < 200 || resp.status >= 300 {
            set_success_result("ValidationFailed", &json!({
                "validation_error": format!("route lookup failed (HTTP {})", resp.status)
            }));
            return Ok(());
        }

        let body: Value = serde_json::from_str(&resp.body)
            .map_err(|e| format!("parse route response: {e}"))?;

        let routes = body
            .get("value")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if routes.is_empty() {
            set_success_result("ValidationFailed", &json!({
                "validation_error": "no matching route"
            }));
            return Ok(());
        }

        let route = &routes[0];
        let route_fields = route.get("fields").cloned().unwrap_or(json!({}));

        let webhook_secret = route_fields
            .get("webhook_secret")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let source_type = route_fields
            .get("source_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // HMAC verification (ARN-168): compute HMAC-SHA256(secret, raw_body) and
        // constant-time compare it against the signature header. Fail closed —
        // if a route declares a secret we must be able to resolve it, find the
        // signature header, and match it, or the payload is rejected outright.
        let hmac_verified = if webhook_secret.is_empty() {
            // No secret configured on this route: nothing to verify against.
            "skipped"
        } else {
            match resolve_webhook_secret(&ctx, webhook_secret) {
                None => {
                    ctx.log(
                        "warn",
                        "validate_webhook: signing secret configured but unresolvable — rejecting",
                    );
                    set_success_result("ValidationFailed", &json!({
                        "validation_error":
                            "webhook signing secret is configured but could not be resolved"
                    }));
                    return Ok(());
                }
                Some(secret) => {
                    match verify_signature(&secret, raw_headers, source_type, raw_payload) {
                        VerifyOutcome::Verified => "true",
                        VerifyOutcome::Rejected(reason) => {
                            ctx.log(
                                "warn",
                                &format!("validate_webhook: signature rejected — {reason}"),
                            );
                            set_success_result("ValidationFailed", &json!({
                                "validation_error": reason
                            }));
                            return Ok(());
                        }
                    }
                }
            }
        };

        ctx.log(
            "info",
            &format!(
                "validate_webhook: route matched, source_type={}, hmac={}",
                source_type, hmac_verified
            ),
        );

        set_success_result("Validated", &json!({
            "source_type": source_type,
            "hmac_verified": hmac_verified,
            "normalized_payload": raw_payload,
        }));

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Resolve the Temper API URL from integration config or fall back to localhost.
fn resolve_api_url(ctx: &Context) -> String {
    ctx.config
        .get("temper_api_url")
        .filter(|s| !s.is_empty() && !s.contains("{secret:"))
        .cloned()
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string())
}

/// Build standard OData request headers.
fn odata_headers(ctx: &Context, tenant: &str) -> Vec<(String, String)> {
    vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("x-tenant-id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "agent".to_string()),
        ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
        ("x-temper-agent-type".to_string(), "system".to_string()),
    ]
}

/// Minimal URL-encoding for OData filter values.
fn urlencoded(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('\'', "%27")
        .replace('&', "%26")
        .replace('=', "%3D")
}

/// Outcome of verifying a webhook signature against a configured secret.
enum VerifyOutcome {
    /// The signature matched `HMAC-SHA256(secret, raw_body)`.
    Verified,
    /// The request must be rejected (fail closed). Carries a human-readable
    /// reason recorded as the WebhookEvent `validation_error`.
    Rejected(String),
}

/// How a route's configured `webhook_secret` value should be resolved.
enum SecretSource<'a> {
    /// No secret is configured (empty or malformed template).
    None,
    /// A literal secret value stored directly on the route.
    Literal(&'a str),
    /// A `{secret:KEY}` template — `KEY` is resolved from the host secret store.
    Template(&'a str),
}

/// Map a webhook source type to the header that carries its HMAC signature.
fn signature_header_name(source_type: &str) -> &'static str {
    match source_type {
        "datadog" => "x-datadog-signature",
        "github" => "x-hub-signature-256",
        _ => "x-hub-signature-256",
    }
}

/// Classify a route's configured secret value without touching the host.
///
/// Kept pure so the parsing rules are unit-testable; the host is only consulted
/// (for `{secret:KEY}` templates) by [`resolve_webhook_secret`].
fn classify_secret(configured: &str) -> SecretSource<'_> {
    let configured = configured.trim();
    if configured.is_empty() {
        return SecretSource::None;
    }
    if let Some(rest) = configured.strip_prefix("{secret:") {
        return match rest.strip_suffix('}').map(str::trim) {
            Some(key) if !key.is_empty() => SecretSource::Template(key),
            _ => SecretSource::None,
        };
    }
    SecretSource::Literal(configured)
}

/// Resolve the webhook signing secret for a route.
///
/// Supports `{secret:KEY}` templates (resolved from the host secret store) and
/// literal secret values. Returns `None` (fail closed) when the value is empty,
/// the template is malformed, or the resolved secret is empty/unresolved.
fn resolve_webhook_secret(ctx: &Context, configured: &str) -> Option<String> {
    match classify_secret(configured) {
        SecretSource::None => None,
        SecretSource::Literal(value) => Some(value.to_string()),
        SecretSource::Template(key) => {
            let resolved = ctx.get_secret(key).ok()?;
            let resolved = resolved.trim().to_string();
            if resolved.is_empty() || resolved.contains("{secret:") {
                None
            } else {
                Some(resolved)
            }
        }
    }
}

/// Extract a header value from the `raw_headers` JSON object.
///
/// HTTP header names are case-insensitive, and the webhook trigger serializes
/// them lowercase, so an exact match is tried first and then a case-insensitive
/// scan.
fn extract_header(raw_headers: &str, header_name: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(raw_headers).ok()?;
    let obj = parsed.as_object()?;
    if let Some(value) = obj.get(header_name).and_then(|v| v.as_str()) {
        return Some(value.to_string());
    }
    obj.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(header_name))
        .and_then(|(_, v)| v.as_str())
        .map(|s| s.to_string())
}

/// Verify the signature header against `HMAC-SHA256(secret, raw_payload)`.
///
/// Fails closed: a missing signature header or a mismatched signature is
/// rejected. The `secret` must already be resolved (non-empty).
fn verify_signature(
    secret: &str,
    raw_headers: &str,
    source_type: &str,
    raw_payload: &str,
) -> VerifyOutcome {
    let header_name = signature_header_name(source_type);
    let Some(provided) = extract_header(raw_headers, header_name) else {
        return VerifyOutcome::Rejected(format!(
            "missing webhook signature header '{header_name}'"
        ));
    };
    if signature_matches(secret, raw_payload.as_bytes(), &provided) {
        VerifyOutcome::Verified
    } else {
        VerifyOutcome::Rejected("webhook signature verification failed".to_string())
    }
}

/// Constant-time comparison of a provided signature against the expected
/// `HMAC-SHA256(secret, body)`.
///
/// Accepts an optional `sha256=` prefix and is case-insensitive over the hex
/// digest. Uses [`subtle::ConstantTimeEq`] rather than `==` to avoid a timing
/// side channel on the digest.
fn signature_matches(secret: &str, body: &[u8], provided: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use subtle::ConstantTimeEq;

    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    let expected_hex = hex::encode(mac.finalize().into_bytes());

    // Normalize (lowercase) so the `sha256=` prefix matches case-insensitively,
    // then strip it before comparing the hex digest.
    let normalized = provided.trim().to_ascii_lowercase();
    let provided_hex = normalized
        .strip_prefix("sha256=")
        .unwrap_or(normalized.as_str())
        .trim();

    provided_hex
        .as_bytes()
        .ct_eq(expected_hex.as_bytes())
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compute the reference `HMAC-SHA256(secret, body)` hex digest.
    fn sign(secret: &str, body: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    fn github_headers(signature: &str) -> String {
        json!({
            "x-hub-signature-256": signature,
            "content-type": "application/json",
        })
        .to_string()
    }

    #[test]
    fn correctly_signed_payload_is_accepted() {
        let secret = "s3cr3t";
        let body = r#"{"action":"opened"}"#;
        let headers = github_headers(&format!("sha256={}", sign(secret, body)));
        assert!(matches!(
            verify_signature(secret, &headers, "github", body),
            VerifyOutcome::Verified
        ));
    }

    /// ARN-168 exploit: a present-but-forged signature must be rejected.
    /// Against the old code this was accepted as `"true"` on header presence.
    #[test]
    fn forged_signature_is_rejected() {
        let secret = "s3cr3t";
        let body = r#"{"action":"opened"}"#;
        let headers = github_headers(
            "sha256=deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        );
        assert!(matches!(
            verify_signature(secret, &headers, "github", body),
            VerifyOutcome::Rejected(_)
        ));
    }

    #[test]
    fn missing_signature_header_is_rejected() {
        let secret = "s3cr3t";
        let body = r#"{"action":"opened"}"#;
        let headers = json!({ "content-type": "application/json" }).to_string();
        assert!(matches!(
            verify_signature(secret, &headers, "github", body),
            VerifyOutcome::Rejected(_)
        ));
    }

    /// A signature valid for one body must not validate a different body —
    /// proves the HMAC covers the payload, not merely header presence.
    #[test]
    fn tampered_body_is_rejected() {
        let secret = "s3cr3t";
        let signed_body = r#"{"action":"opened"}"#;
        let attacker_body = r#"{"action":"deleted"}"#;
        let headers = github_headers(&format!("sha256={}", sign(secret, signed_body)));
        assert!(matches!(
            verify_signature(secret, &headers, "github", attacker_body),
            VerifyOutcome::Rejected(_)
        ));
    }

    #[test]
    fn wrong_secret_is_rejected() {
        let body = r#"{"action":"opened"}"#;
        let headers = github_headers(&format!("sha256={}", sign("attacker-guess", body)));
        assert!(matches!(
            verify_signature("real-secret", &headers, "github", body),
            VerifyOutcome::Rejected(_)
        ));
    }

    #[test]
    fn bare_hex_signature_without_prefix_is_accepted() {
        // Sources that send a bare hex digest (no `sha256=` prefix) also verify.
        let secret = "s3cr3t";
        let body = "payload";
        let headers = json!({ "x-datadog-signature": sign(secret, body) }).to_string();
        assert!(matches!(
            verify_signature(secret, &headers, "datadog", body),
            VerifyOutcome::Verified
        ));
    }

    #[test]
    fn uppercase_prefixed_signature_is_accepted() {
        let secret = "s3cr3t";
        let body = "payload";
        let headers = github_headers(&format!("SHA256={}", sign(secret, body).to_uppercase()));
        assert!(matches!(
            verify_signature(secret, &headers, "github", body),
            VerifyOutcome::Verified
        ));
    }

    #[test]
    fn header_lookup_is_case_insensitive() {
        let secret = "s3cr3t";
        let body = "payload";
        let headers = json!({
            "X-Hub-Signature-256": format!("sha256={}", sign(secret, body)),
        })
        .to_string();
        assert!(matches!(
            verify_signature(secret, &headers, "github", body),
            VerifyOutcome::Verified
        ));
    }

    #[test]
    fn signature_matches_only_the_correct_digest() {
        let secret = "key";
        let body = b"body-bytes";
        let good = sign(secret, "body-bytes");
        assert!(signature_matches(secret, body, &good));
        assert!(signature_matches(secret, body, &format!("sha256={good}")));
        assert!(!signature_matches(secret, body, "sha256=00"));
        assert!(!signature_matches(secret, body, ""));
        assert!(!signature_matches(secret, body, "not-hex"));
    }

    #[test]
    fn classify_secret_variants() {
        assert!(matches!(classify_secret(""), SecretSource::None));
        assert!(matches!(classify_secret("   "), SecretSource::None));
        assert!(matches!(classify_secret("{secret:}"), SecretSource::None));
        assert!(matches!(classify_secret("{secret: }"), SecretSource::None));
        assert!(matches!(
            classify_secret("literal-secret"),
            SecretSource::Literal("literal-secret")
        ));
        match classify_secret("{secret:GITHUB_WEBHOOK}") {
            SecretSource::Template(key) => assert_eq!(key, "GITHUB_WEBHOOK"),
            _ => panic!("expected a {{secret:KEY}} template"),
        }
    }
}
