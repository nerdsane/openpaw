//! FAL image editing for PawMedia.
//!
//! `MediaGenerationRequest.Edit` invokes this provider-family module. It reads
//! one immutable PawFS source, sends the caller's prompt unchanged to an
//! allow-listed FAL edit model, stores the output in PawFS, and records
//! content-addressed provenance.

mod pawfs;

use base64::{Engine as _, engine::general_purpose};
use pawfs::{SourceImage, StoredImage, download_image, load_source_image, store_image};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use temper_wasm_sdk::prelude::*;
use wasm_helpers::{resolve_temper_api_url, runtime_headers_as};

const DEFAULT_MEDIA_TYPE: &str = "image";
const DEFAULT_OPERATION: &str = "edit";
const DEFAULT_PROVIDER: &str = "fal";
const DEFAULT_OUTPUT_FORMAT: &str = "png";
const GPT_IMAGE_EDIT: &str = "openai/gpt-image-2/edit";
const NANO_BANANA_EDIT: &str = "fal-ai/nano-banana-2/edit";
const FAL_RUN_BASE: &str = "https://fal.run";
const MAX_PROVIDER_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    if let Err(error) = run_fal_image_edit() {
        set_error_result(&error);
    }
    0
}

fn run_fal_image_edit() -> Result<(), String> {
    let ctx = Context::from_host()?;
    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    match edit_and_store(&ctx, &fields) {
        Ok(result) => set_success_result("RecordResult", &record_result_params(&result)),
        Err(error) => set_success_result(
            "RecordError",
            &json!({
                "error": error,
                "last_error": "fal_image_edit failed",
            }),
        ),
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditResult {
    stored: StoredImage,
    source_sha256: String,
    prompt_sha256: String,
    result_sha256: String,
    provider_response_id: String,
    usage_json: String,
}

fn edit_and_store(ctx: &Context, fields: &Value) -> Result<EditResult, String> {
    let request = validated_request(fields)?;
    let source = load_source_image(ctx, fields, &request.workspace_id, &request.source_file_id)?;
    validate_requested_source_version(&request, &source)?;

    let source_sha256 = sha256_hex(&source.bytes);
    let prompt_sha256 = sha256_hex(request.prompt.as_bytes());
    let image_data_uri = format!(
        "data:{};base64,{}",
        source.mime_type,
        general_purpose::STANDARD.encode(&source.bytes)
    );
    let provider_body = provider_request_body(&request, &image_data_uri);
    let provider = call_fal(ctx, &request.model, &provider_body)?;
    let provider_response_id = provider_response_id(&provider.body);
    let usage_json = usage_json(&provider.body);
    record_storing(ctx, fields, &provider_response_id, &usage_json)?;
    let result_url = first_image_url(&provider.body)?;
    let downloaded = download_image(ctx, &result_url)?;
    let result_sha256 = sha256_hex(&downloaded.bytes);
    let output_path = output_path(fields, ctx, extension_for_mime(&downloaded.mime_type));
    let stored = store_image(
        ctx,
        fields,
        &request.workspace_id,
        &output_path,
        &downloaded.mime_type,
        &downloaded.bytes,
    )?;

    Ok(EditResult {
        stored,
        source_sha256,
        prompt_sha256,
        result_sha256,
        provider_response_id,
        usage_json,
    })
}

fn record_result_params(result: &EditResult) -> Value {
    json!({
        "result_file_id": result.stored.file_id,
        "result_file_version_id": result.stored.file_version_id,
        "result_path": result.stored.path,
        "mime_type": result.stored.mime_type,
        "revised_prompt": "",
        "provider_response_id": result.provider_response_id,
        "usage_json": result.usage_json,
        "result_image_base64": "",
        "source_sha256": result.source_sha256,
        "prompt_sha256": result.prompt_sha256,
        "result_sha256": result.result_sha256,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditRequest {
    prompt: String,
    model: String,
    size: String,
    quality: String,
    output_format: String,
    workspace_id: String,
    source_file_id: String,
    source_file_version_id: String,
}

fn validated_request(fields: &Value) -> Result<EditRequest, String> {
    let media_type = field(fields, &["media_type", "MediaType"], DEFAULT_MEDIA_TYPE);
    let operation = field(fields, &["operation", "Operation"], DEFAULT_OPERATION);
    let provider = field(fields, &["provider", "Provider"], DEFAULT_PROVIDER)
        .trim()
        .to_ascii_lowercase();
    if !media_type.eq_ignore_ascii_case(DEFAULT_MEDIA_TYPE) {
        return Err(format!(
            "fal_image_edit: unsupported media_type '{media_type}'"
        ));
    }
    if !operation.eq_ignore_ascii_case(DEFAULT_OPERATION) {
        return Err(format!(
            "fal_image_edit: unsupported operation '{operation}'"
        ));
    }
    if provider != DEFAULT_PROVIDER {
        return Err(format!("fal_image_edit: unsupported provider '{provider}'"));
    }

    let model = field(fields, &["model", "Model"], "");
    if ![GPT_IMAGE_EDIT, NANO_BANANA_EDIT].contains(&model) {
        return Err(format!("fal_image_edit: unsupported model '{model}'"));
    }
    let prompt = field(fields, &["prompt", "Prompt"], "").trim().to_string();
    let workspace_id = field(fields, &["workspace_id", "WorkspaceId"], "")
        .trim()
        .to_string();
    let source_file_id = field(fields, &["source_file_id", "SourceFileId"], "")
        .trim()
        .to_string();
    if prompt.is_empty() {
        return Err("fal_image_edit: prompt is required".to_string());
    }
    if workspace_id.is_empty() {
        return Err("fal_image_edit: workspace_id is required".to_string());
    }
    if source_file_id.is_empty() {
        return Err("fal_image_edit: source_file_id is required".to_string());
    }

    Ok(EditRequest {
        prompt,
        model: model.to_string(),
        size: field(fields, &["size", "Size"], "auto").to_string(),
        quality: field(fields, &["quality", "Quality"], "high").to_string(),
        output_format: normalize_output_format(field(
            fields,
            &["output_format", "OutputFormat"],
            DEFAULT_OUTPUT_FORMAT,
        )),
        workspace_id,
        source_file_id,
        source_file_version_id: field(
            fields,
            &["source_file_version_id", "SourceFileVersionId"],
            "",
        )
        .trim()
        .to_string(),
    })
}

fn validate_requested_source_version(
    request: &EditRequest,
    source: &SourceImage,
) -> Result<(), String> {
    if !request.source_file_version_id.is_empty()
        && request.source_file_version_id != source.file_version_id
    {
        return Err(format!(
            "fal_image_edit: source file version changed (requested '{}', current '{}')",
            request.source_file_version_id, source.file_version_id
        ));
    }
    Ok(())
}

fn provider_request_body(request: &EditRequest, image_data_uri: &str) -> Value {
    if request.model == GPT_IMAGE_EDIT {
        json!({
            "prompt": request.prompt,
            "image_urls": [image_data_uri],
            "image_size": normalize_gpt_size(&request.size),
            "quality": normalize_quality(&request.quality),
            "num_images": 1,
            "output_format": request.output_format,
        })
    } else {
        json!({
            "prompt": request.prompt,
            "image_urls": [image_data_uri],
            "aspect_ratio": "auto",
            "resolution": normalize_nano_resolution(&request.size),
            "num_images": 1,
            "output_format": request.output_format,
            "limit_generations": true,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderResponse {
    body: String,
}

fn call_fal(ctx: &Context, model: &str, body: &Value) -> Result<ProviderResponse, String> {
    let fal_key = ctx
        .config
        .get("fal_key")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty() && !value.contains("{secret:"))
        .ok_or_else(|| "fal_image_edit: fal_key is not configured".to_string())?;
    let url = format!("{FAL_RUN_BASE}/{model}");
    let authorization = format!("Key {fal_key}");
    let headers = [
        ("Authorization", authorization.as_str()),
        ("Content-Type", "application/json"),
        ("Accept", "application/json"),
    ];
    let serialized = body.to_string();
    let (status, response_bytes) = pawfs::streaming_http_call(
        "POST",
        &url,
        &headers,
        serialized.as_bytes(),
        MAX_PROVIDER_RESPONSE_BYTES,
    )?;
    let response_body = String::from_utf8(response_bytes)
        .map_err(|error| format!("fal_image_edit: provider response was not UTF-8: {error}"))?;
    if !(200..300).contains(&status) {
        return Err(format!(
            "fal_image_edit: provider returned HTTP {status}: {}",
            snippet(&response_body)
        ));
    }
    serde_json::from_str::<Value>(&response_body)
        .map_err(|error| format!("fal_image_edit: provider returned invalid JSON: {error}"))?;
    Ok(ProviderResponse {
        body: response_body,
    })
}

fn first_image_url(body: &str) -> Result<String, String> {
    let parsed: Value = serde_json::from_str(body)
        .map_err(|error| format!("fal_image_edit: parse provider response: {error}"))?;
    let data = parsed.get("data").unwrap_or(&parsed);
    let url = data
        .get("images")
        .and_then(Value::as_array)
        .and_then(|images| images.first())
        .and_then(|image| image.get("url"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if !url.starts_with("https://") {
        return Err("fal_image_edit: provider response contained no HTTPS image URL".to_string());
    }
    Ok(url.to_string())
}

fn provider_response_id(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("request_id")
                .or_else(|| value.get("id"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .unwrap_or_default()
}

fn usage_json(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| value.get("usage").cloned())
        .map(|value| value.to_string())
        .unwrap_or_else(|| "{}".to_string())
}

fn output_path(fields: &Value, ctx: &Context, extension: &str) -> String {
    let configured = field(fields, &["output_path", "OutputPath"], "").trim();
    if !configured.is_empty() {
        return with_extension(configured, extension);
    }
    let entity_id = ctx
        .entity_state
        .get("entity_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(ctx.entity_id.as_str());
    format!("/generated/edits/{entity_id}.{extension}")
}

fn field<'a>(fields: &'a Value, names: &[&str], fallback: &'a str) -> &'a str {
    names
        .iter()
        .find_map(|name| fields.get(*name).and_then(Value::as_str))
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
}

fn normalize_output_format(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "jpeg".to_string(),
        "webp" => "webp".to_string(),
        _ => "png".to_string(),
    }
}

fn normalize_quality(value: &str) -> &'static str {
    match value.trim().to_ascii_lowercase().as_str() {
        "low" => "low",
        "medium" => "medium",
        "auto" => "auto",
        _ => "high",
    }
}

fn normalize_gpt_size(value: &str) -> &'static str {
    match value.trim() {
        "square_hd" => "square_hd",
        "square" => "square",
        "portrait_4_3" => "portrait_4_3",
        "portrait_16_9" => "portrait_16_9",
        "landscape_4_3" => "landscape_4_3",
        "landscape_16_9" => "landscape_16_9",
        _ => "auto",
    }
}

fn normalize_nano_resolution(value: &str) -> &'static str {
    match value.trim().to_ascii_uppercase().as_str() {
        "0.5K" => "0.5K",
        "2K" => "2K",
        "4K" => "4K",
        _ => "1K",
    }
}

fn extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        _ => "png",
    }
}

fn with_extension(path: &str, extension: &str) -> String {
    let lower = path.to_ascii_lowercase();
    if [".png", ".jpg", ".jpeg", ".webp"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
    {
        path.to_string()
    } else {
        format!("{path}.{extension}")
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn snippet(value: &str) -> String {
    value.chars().take(300).collect()
}

fn record_storing(
    ctx: &Context,
    fields: &Value,
    provider_response_id: &str,
    usage_json: &str,
) -> Result<(), String> {
    let api_url = resolve_temper_api_url(ctx, fields);
    let entity_id = ctx
        .entity_state
        .get("entity_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(ctx.entity_id.as_str());
    let url = format!(
        "{api_url}/tdata/MediaGenerationRequests('{}')/Temper.RecordStoring",
        entity_id.replace('\'', "''")
    );
    let headers = runtime_headers_as(
        ctx,
        &ctx.tenant,
        fields,
        "system",
        Some("application/json"),
        Some("application/json"),
    );
    let body = json!({
        "provider_response_id": provider_response_id,
        "revised_prompt": "",
        "usage_json": usage_json,
    });
    let response = ctx.http_call("POST", &url, &headers, &body.to_string())?;
    if !(200..300).contains(&response.status) {
        return Err(format!(
            "fal_image_edit: RecordStoring returned HTTP {}: {}",
            response.status,
            snippet(&response.body)
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(model: &str) -> Value {
        json!({
            "Prompt": "Use dry charcoal marks and hard white-paper gaps.",
            "MediaType": "image",
            "Operation": "edit",
            "Provider": "fal",
            "Model": model,
            "Size": "auto",
            "Quality": "high",
            "OutputFormat": "png",
            "WorkspaceId": "workspace-1",
            "SourceFileId": "source-1",
            "SourceFileVersionId": "version-1",
        })
    }

    #[test]
    fn both_models_receive_the_exact_prompt() {
        for model in [GPT_IMAGE_EDIT, NANO_BANANA_EDIT] {
            let request = validated_request(&fields(model)).unwrap();
            let body = provider_request_body(&request, "data:image/png;base64,abc");
            assert_eq!(
                body["prompt"],
                "Use dry charcoal marks and hard white-paper gaps."
            );
        }
    }

    #[test]
    fn arbitrary_fal_endpoints_are_rejected() {
        let error = validated_request(&fields("someone/unknown/edit")).unwrap_err();
        assert!(error.contains("unsupported model"));
    }

    #[test]
    fn parses_wrapped_and_unwrapped_results() {
        for body in [
            r#"{"images":[{"url":"https://files.test/output.png"}]}"#,
            r#"{"data":{"images":[{"url":"https://files.test/output.png"}]}}"#,
        ] {
            assert_eq!(
                first_image_url(body).unwrap(),
                "https://files.test/output.png"
            );
        }
    }
}
