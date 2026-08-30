use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use temper_wasm_sdk::prelude::*;
use wasm_helpers::{entity_field_str, resolve_temper_api_url, runtime_headers_for_workspace};

#[cfg(target_arch = "wasm32")]
const STREAM_CHUNK_BYTES: usize = 256 * 1024;
const DEFAULT_MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_MAX_RESULT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceImage {
    pub(crate) file_version_id: String,
    pub(crate) mime_type: String,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DownloadedImage {
    pub(crate) mime_type: String,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredImage {
    pub(crate) file_id: String,
    pub(crate) file_version_id: String,
    pub(crate) path: String,
    pub(crate) mime_type: String,
}

pub(crate) fn load_source_image(
    ctx: &Context,
    fields: &Value,
    workspace_id: &str,
    file_id: &str,
) -> Result<SourceImage, String> {
    let api_url = resolve_temper_api_url(ctx, fields);
    let head_url = format!("{api_url}/tdata/Files('{}')", escape_key(file_id));
    let headers = runtime_headers_for_workspace(
        ctx,
        &ctx.tenant,
        fields,
        workspace_id,
        None,
        Some("application/json"),
    );
    let response = ctx.http_call("GET", &head_url, &headers, "")?;
    if !(200..300).contains(&response.status) {
        return Err(format!(
            "fal_image_edit: source File read failed (HTTP {}): {}",
            response.status,
            snippet(&response.body)
        ));
    }
    let head: Value = serde_json::from_str(&response.body)
        .map_err(|error| format!("fal_image_edit: parse source File: {error}"))?;
    let source_workspace = entity_field_str(&head, &["WorkspaceId", "workspace_id"]).unwrap_or("");
    if !source_workspace.is_empty() && source_workspace != workspace_id {
        return Err("fal_image_edit: source File belongs to another workspace".to_string());
    }
    let file_version_id = entity_field_str(&head, &["LastVersionId", "last_version_id"])
        .unwrap_or("")
        .to_string();
    let declared_mime = entity_field_str(&head, &["MimeType", "mime_type"]).unwrap_or("");
    let value_url = format!("{api_url}/tdata/Files('{}')/$value", escape_key(file_id));
    let max_bytes = config_usize(ctx, "max_source_bytes", DEFAULT_MAX_SOURCE_BYTES);
    let (_, bytes) =
        streaming_http_call("GET", &value_url, &to_borrowed(&headers), &[], max_bytes)?;
    verify_file_version_bytes(ctx, &api_url, &headers, file_id, &file_version_id, &bytes)?;
    let mime_type = detect_image_mime(&bytes)
        .or_else(|| normalize_image_mime(declared_mime))
        .ok_or_else(|| "fal_image_edit: source File is not a supported raster image".to_string())?;
    Ok(SourceImage {
        file_version_id,
        mime_type,
        bytes,
    })
}

fn verify_file_version_bytes(
    ctx: &Context,
    api_url: &str,
    headers: &[(String, String)],
    file_id: &str,
    file_version_id: &str,
    bytes: &[u8],
) -> Result<(), String> {
    if file_version_id.is_empty() {
        return Err("fal_image_edit: source File has no immutable version id".to_string());
    }
    let version_url = format!(
        "{api_url}/tdata/FileVersions('{}')",
        escape_key(file_version_id)
    );
    let version = ctx.http_call("GET", &version_url, headers, "")?;
    if !(200..300).contains(&version.status) {
        return Err(format!(
            "fal_image_edit: source FileVersion read failed (HTTP {}): {}",
            version.status,
            snippet(&version.body)
        ));
    }
    let version: Value = serde_json::from_str(&version.body)
        .map_err(|error| format!("fal_image_edit: parse source FileVersion: {error}"))?;
    let version_file_id = entity_field_str(&version, &["FileId", "file_id"]).unwrap_or("");
    if version_file_id != file_id {
        return Err("fal_image_edit: source FileVersion belongs to another File".to_string());
    }
    let declared_hash = entity_field_str(&version, &["ContentHash", "content_hash"]).unwrap_or("");
    let actual_hash = format!("{:x}", Sha256::digest(bytes));
    if !normalize_sha256(declared_hash)
        .is_some_and(|declared| declared.eq_ignore_ascii_case(&actual_hash))
    {
        return Err(
            "fal_image_edit: source bytes do not match the immutable FileVersion hash".to_string(),
        );
    }
    Ok(())
}

pub(crate) fn download_image(ctx: &Context, url: &str) -> Result<DownloadedImage, String> {
    if !url.starts_with("https://") {
        return Err("fal_image_edit: result URL must use HTTPS".to_string());
    }
    let max_bytes = config_usize(ctx, "max_result_bytes", DEFAULT_MAX_RESULT_BYTES);
    let (status, bytes) = streaming_http_call("GET", url, &[], &[], max_bytes)?;
    if !(200..300).contains(&status) {
        return Err(format!(
            "fal_image_edit: result download returned HTTP {status}"
        ));
    }
    let mime_type = detect_image_mime(&bytes)
        .ok_or_else(|| "fal_image_edit: result is not a supported raster image".to_string())?;
    Ok(DownloadedImage { mime_type, bytes })
}

pub(crate) fn store_image(
    ctx: &Context,
    fields: &Value,
    workspace_id: &str,
    path: &str,
    mime_type: &str,
    bytes: &[u8],
) -> Result<StoredImage, String> {
    let api_url = resolve_temper_api_url(ctx, fields);
    let name = path.rsplit('/').next().unwrap_or("edited-image.png");
    let headers = runtime_headers_for_workspace(
        ctx,
        &ctx.tenant,
        fields,
        workspace_id,
        Some("application/json"),
        Some("application/json"),
    );
    let create = ctx.http_call(
        "POST",
        &format!("{api_url}/tdata/Files"),
        &headers,
        &json!({
            "Name": name,
            "Path": path,
            "WorkspaceId": workspace_id,
            "MimeType": mime_type,
        })
        .to_string(),
    )?;
    if !(200..300).contains(&create.status) {
        return Err(format!(
            "fal_image_edit: result File create failed (HTTP {}): {}",
            create.status,
            snippet(&create.body)
        ));
    }
    let created: Value = serde_json::from_str(&create.body)
        .map_err(|error| format!("fal_image_edit: parse result File create: {error}"))?;
    let file_id = entity_field_str(&created, &["Id", "id"])
        .or_else(|| created.get("entity_id").and_then(Value::as_str))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "fal_image_edit: result File create returned no id".to_string())?
        .to_string();

    let value_headers = runtime_headers_for_workspace(
        ctx,
        &ctx.tenant,
        fields,
        workspace_id,
        Some(mime_type),
        None,
    );
    let value_url = format!("{api_url}/tdata/Files('{}')/$value", escape_key(&file_id));
    let (put_status, _) =
        streaming_http_call("PUT", &value_url, &to_borrowed(&value_headers), bytes, 1024)?;
    if !(200..300).contains(&put_status) {
        return Err(format!(
            "fal_image_edit: result File upload returned HTTP {put_status}"
        ));
    }

    let head = ctx.http_call(
        "GET",
        &format!("{api_url}/tdata/Files('{}')", escape_key(&file_id)),
        &headers,
        "",
    )?;
    if !(200..300).contains(&head.status) {
        return Err(format!(
            "fal_image_edit: result File read-after-write returned HTTP {}",
            head.status
        ));
    }
    let head_value: Value = serde_json::from_str(&head.body)
        .map_err(|error| format!("fal_image_edit: parse result File head: {error}"))?;
    let file_version_id = entity_field_str(&head_value, &["LastVersionId", "last_version_id"])
        .unwrap_or("")
        .to_string();

    Ok(StoredImage {
        file_id,
        file_version_id,
        path: path.to_string(),
        mime_type: mime_type.to_string(),
    })
}

pub(crate) fn streaming_http_call(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    request_bytes: &[u8],
    max_response_bytes: usize,
) -> Result<(u16, Vec<u8>), String> {
    #[cfg(target_arch = "wasm32")]
    {
        let (mut request_body, mut response_body, response_head) =
            temper_wasm_sdk::http_stream::streaming_call(method, url, headers)
                .map_err(|error| format!("fal_image_edit: open HTTP stream: {error}"))?;
        for chunk in request_bytes.chunks(STREAM_CHUNK_BYTES) {
            request_body
                .write_all_chunk(chunk)
                .map_err(|error| format!("fal_image_edit: write HTTP request: {error}"))?;
        }
        request_body
            .finish()
            .map_err(|error| format!("fal_image_edit: finish HTTP request: {error}"))?;
        let head = response_head()
            .map_err(|error| format!("fal_image_edit: read HTTP response head: {error}"))?;
        let mut bytes = Vec::new();
        let mut buffer = vec![0_u8; STREAM_CHUNK_BYTES];
        loop {
            match response_body.read_next_chunk(&mut buffer) {
                Ok(Some(0)) | Ok(None) => break,
                Ok(Some(count)) => {
                    if bytes.len().saturating_add(count) > max_response_bytes {
                        let _ = response_body.close();
                        return Err(format!(
                            "fal_image_edit: HTTP response exceeds {max_response_bytes} bytes"
                        ));
                    }
                    bytes.extend_from_slice(&buffer[..count]);
                }
                Err(error) => {
                    let _ = response_body.close();
                    return Err(format!("fal_image_edit: read HTTP response: {error}"));
                }
            }
        }
        response_body
            .close()
            .map_err(|error| format!("fal_image_edit: close HTTP response: {error}"))?;
        Ok((head.status, bytes))
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (method, url, headers, request_bytes, max_response_bytes);
        Err("fal_image_edit: streaming HTTP is available in WASM only".to_string())
    }
}

fn config_usize(ctx: &Context, key: &str, fallback: usize) -> usize {
    ctx.config
        .get(key)
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn detect_image_mime(bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png".to_string())
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg".to_string())
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp".to_string())
    } else {
        None
    }
}

fn normalize_image_mime(value: &str) -> Option<String> {
    match value.split(';').next().unwrap_or("").trim() {
        "image/png" => Some("image/png".to_string()),
        "image/jpeg" | "image/jpg" => Some("image/jpeg".to_string()),
        "image/webp" => Some("image/webp".to_string()),
        _ => None,
    }
}

fn to_borrowed(headers: &[(String, String)]) -> Vec<(&str, &str)> {
    headers
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect()
}

fn escape_key(value: &str) -> String {
    value.replace('\'', "''")
}

fn snippet(value: &str) -> String {
    value.chars().take(300).collect()
}

fn normalize_sha256(value: &str) -> Option<&str> {
    let value = value.trim();
    let digest = value.strip_prefix("sha256:").unwrap_or(value);
    (digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_raster_formats() {
        assert_eq!(
            detect_image_mime(b"\x89PNG\r\n\x1a\npayload").as_deref(),
            Some("image/png")
        );
        assert_eq!(
            detect_image_mime(&[0xff, 0xd8, 0xff, 0x00]).as_deref(),
            Some("image/jpeg")
        );
        assert_eq!(
            detect_image_mime(b"RIFFxxxxWEBPpayload").as_deref(),
            Some("image/webp")
        );
        assert!(detect_image_mime(b"<svg>").is_none());
    }

    #[test]
    fn normalizes_content_addressed_sha256_values() {
        let hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        assert_eq!(normalize_sha256(hash), Some(hash));
        assert_eq!(normalize_sha256(&format!("sha256:{hash}")), Some(hash));
        assert_eq!(normalize_sha256("sha256:not-a-digest"), None);
    }
}
