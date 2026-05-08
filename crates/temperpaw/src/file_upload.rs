use axum::Router;
use axum::extract::{Json, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use base64::Engine;
use serde::Deserialize;
use temper_platform::PlatformState;
use temper_runtime::tenant::TenantId;
use temper_server::request_context::AgentContext;

#[derive(Debug, Deserialize)]
struct Base64FileUploadRequest {
    #[serde(alias = "content_type")]
    mime_type: String,
    base64_data: String,
}

pub(crate) fn router(state: PlatformState) -> Router {
    Router::new()
        .route(
            "/paw/fs/files/{file_id}/value-base64",
            post(upload_file_value_base64),
        )
        .with_state(state)
}

async fn upload_file_value_base64(
    State(state): State<PlatformState>,
    Path(file_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<Base64FileUploadRequest>,
) -> Response {
    let tenant = tenant_from_headers(&headers);
    let (bytes, mime_type) =
        match decode_browser_image_base64(request.base64_data.trim(), request.mime_type.trim()) {
            Ok(decoded) => decoded,
            Err(message) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "InvalidImageBase64",
                        "message": message,
                    })),
                )
                    .into_response();
            }
        };

    let agent_ctx = agent_context_from_headers(&headers);
    match state
        .server
        .put_file_stream_content(&tenant, &file_id, &bytes, mime_type, &agent_ctx)
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "file_id": file_id,
                "mime_type": mime_type,
                "size_bytes": bytes.len(),
            })),
        )
            .into_response(),
        Err(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "FileUploadFailed",
                "message": message,
            })),
        )
            .into_response(),
    }
}

fn tenant_from_headers(headers: &HeaderMap) -> TenantId {
    headers
        .get("x-tenant-id")
        .and_then(|value| value.to_str().ok())
        .map(TenantId::new)
        .unwrap_or_default()
}

fn agent_context_from_headers(headers: &HeaderMap) -> AgentContext {
    fn header_string(headers: &HeaderMap, name: &str) -> Option<String> {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }

    AgentContext {
        agent_id: header_string(headers, "x-temper-principal-id"),
        agent_type: header_string(headers, "x-temper-agent-type"),
        session_id: header_string(headers, "x-session-id"),
        intent: header_string(headers, "x-intent"),
        trace_id: traceparent_ids(headers).map(|(trace_id, _)| trace_id),
        parent_span_id: traceparent_ids(headers).map(|(_, span_id)| span_id),
        workflow_root_entity_type: header_string(headers, "x-temper-workflow-root-entity-type"),
        workflow_root_entity_id: header_string(headers, "x-temper-workflow-root-entity-id"),
        workflow_run_id: header_string(headers, "x-temper-workflow-run-id"),
        idempotency_key: header_string(headers, "idempotency-key"),
        ..AgentContext::default()
    }
}

fn traceparent_ids(headers: &HeaderMap) -> Option<(String, String)> {
    let traceparent = headers.get("traceparent")?.to_str().ok()?;
    let mut parts = traceparent.split('-');
    let _version = parts.next()?;
    let trace_id = parts.next()?;
    let span_id = parts.next()?;
    if trace_id.len() == 32 && span_id.len() == 16 {
        Some((trace_id.to_string(), span_id.to_string()))
    } else {
        None
    }
}

fn decode_browser_image_base64(
    raw: &str,
    declared_mime: &str,
) -> Result<(Vec<u8>, &'static str), String> {
    let normalized_declared = normalize_image_mime(declared_mime)
        .ok_or_else(|| format!("unsupported image MIME type '{declared_mime}'"))?;
    let (base64_text, data_url_mime) = split_data_url_base64(raw)?;
    if let Some(data_url_mime) = data_url_mime {
        let normalized_data_url_mime = normalize_image_mime(data_url_mime)
            .ok_or_else(|| format!("unsupported data URL image MIME type '{data_url_mime}'"))?;
        if normalized_data_url_mime != normalized_declared {
            return Err(format!(
                "declared MIME type '{normalized_declared}' does not match data URL MIME type '{normalized_data_url_mime}'"
            ));
        }
    }

    let compact: String = base64_text
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect();
    if compact.is_empty() {
        return Err("image payload is empty".to_string());
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(compact.as_bytes())
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(compact.as_bytes()))
        .map_err(|error| format!("image payload is not valid base64: {error}"))?;

    let detected_mime = detect_browser_image_mime(&bytes)
        .ok_or_else(|| "decoded payload is not a supported browser image".to_string())?;
    if detected_mime != normalized_declared {
        return Err(format!(
            "declared MIME type '{normalized_declared}' does not match decoded image bytes '{detected_mime}'"
        ));
    }

    Ok((bytes, detected_mime))
}

fn split_data_url_base64(raw: &str) -> Result<(&str, Option<&str>), String> {
    let Some(rest) = raw.strip_prefix("data:") else {
        return Ok((raw, None));
    };
    let Some((metadata, data)) = rest.split_once(',') else {
        return Err("data URL is missing ',' separator".to_string());
    };
    let mut parts = metadata.split(';');
    let mime_type = parts.next().unwrap_or("");
    if !parts.any(|part| part.eq_ignore_ascii_case("base64")) {
        return Err("data URL image payload must be base64 encoded".to_string());
    }
    Ok((data, Some(mime_type)))
}

fn normalize_image_mime(mime_type: &str) -> Option<&'static str> {
    match mime_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "image/jpeg" | "image/jpg" => Some("image/jpeg"),
        "image/png" => Some("image/png"),
        "image/gif" => Some("image/gif"),
        "image/webp" => Some("image/webp"),
        "image/svg+xml" => Some("image/svg+xml"),
        _ => None,
    }
}

fn detect_browser_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }

    let text = std::str::from_utf8(bytes).ok()?.trim_start();
    if text.starts_with("<svg") || (text.starts_with("<?xml") && text.contains("<svg")) {
        return Some("image/svg+xml");
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=";

    #[test]
    fn decodes_plain_png_base64_to_image_bytes() {
        let (bytes, mime_type) = decode_browser_image_base64(PNG_1X1, "image/png").unwrap();

        assert_eq!(mime_type, "image/png");
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn decodes_data_url_png_base64_to_image_bytes() {
        let (bytes, mime_type) =
            decode_browser_image_base64(&format!("data:image/png;base64,{PNG_1X1}"), "image/png")
                .unwrap();

        assert_eq!(mime_type, "image/png");
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn rejects_base64_text_that_is_not_an_image() {
        let error = decode_browser_image_base64("aGVsbG8=", "image/png").unwrap_err();

        assert!(error.contains("not a supported browser image"));
    }

    #[test]
    fn rejects_mime_mismatch() {
        let error = decode_browser_image_base64(PNG_1X1, "image/jpeg").unwrap_err();

        assert!(error.contains("does not match decoded image bytes"));
    }
}
