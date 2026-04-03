//! Web Fetch — WASM module for fetching URLs and extracting text content.
//!
//! Triggered by WebQuery.ExecuteFetch action. Reads the URL from entity state,
//! fetches the page, strips HTML tags, and transitions to Complete with text
//! or Failed with error.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Maximum response size to keep (100KB).
const MAX_CONTENT_LEN: usize = 100_000;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "web_fetch: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let url = fields
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if url.is_empty() {
            set_success_result(
                "RecordError",
                &json!({"error": "web_fetch: url is empty"}),
            );
            return Ok(());
        }

        // Validate URL scheme
        if !url.starts_with("http://") && !url.starts_with("https://") {
            set_success_result(
                "RecordError",
                &json!({"error": "web_fetch: only http:// and https:// URLs are supported"}),
            );
            return Ok(());
        }

        let headers = vec![
            ("User-Agent".to_string(), "OpenPaw/1.0".to_string()),
            ("Accept".to_string(), "text/html, text/plain, */*".to_string()),
        ];

        ctx.log("info", &format!("web_fetch: fetching {url}"));

        let resp = ctx.http_call("GET", url, &headers, "")?;

        if resp.status < 200 || resp.status >= 300 {
            let err_body: String = resp.body.chars().take(300).collect();
            set_success_result(
                "RecordError",
                &json!({"error": format!("web_fetch: HTTP {} from {}: {}", resp.status, url, err_body)}),
            );
            return Ok(());
        }

        // Strip HTML if the response looks like HTML
        let text = if looks_like_html(&resp.body) {
            strip_html(&resp.body)
        } else {
            resp.body
        };

        // Truncate to max size
        let truncated: String = text.chars().take(MAX_CONTENT_LEN).collect();

        ctx.log(
            "info",
            &format!("web_fetch: got {} chars of text", truncated.len()),
        );

        set_success_result(
            "RecordResults",
            &json!({"results": truncated}),
        );

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Check if the response body looks like HTML.
fn looks_like_html(body: &str) -> bool {
    let lower: String = body.chars().take(500).collect::<String>().to_lowercase();
    lower.contains("<html") || lower.contains("<!doctype html")
}

/// Strip HTML tags from a string using character-by-character parsing.
/// Removes <script> and <style> blocks entirely, strips all other tags,
/// and collapses whitespace runs.
fn strip_html(html: &str) -> String {
    let mut result = String::with_capacity(html.len() / 2);
    let chars: Vec<char> = html.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_tag = false;
    let mut skip_block = false;
    let mut skip_tag = String::new();
    let mut last_was_space = false;

    while i < len {
        if skip_block {
            // Look for closing tag of the block we're skipping
            if chars[i] == '<' && i + 2 < len && chars[i + 1] == '/' {
                let rest: String = chars[i..].iter().take(20).collect();
                let rest_lower = rest.to_lowercase();
                let close = format!("</{}>", skip_tag);
                if rest_lower.starts_with(&close) {
                    i += close.len();
                    skip_block = false;
                    skip_tag.clear();
                    continue;
                }
            }
            i += 1;
            continue;
        }

        if chars[i] == '<' {
            // Check for script/style opening tags
            let rest: String = chars[i..].iter().take(20).collect();
            let rest_lower = rest.to_lowercase();
            if rest_lower.starts_with("<script") {
                skip_block = true;
                skip_tag = "script".to_string();
                i += 1;
                continue;
            }
            if rest_lower.starts_with("<style") {
                skip_block = true;
                skip_tag = "style".to_string();
                i += 1;
                continue;
            }
            in_tag = true;
            i += 1;
            continue;
        }

        if chars[i] == '>' && in_tag {
            in_tag = false;
            i += 1;
            continue;
        }

        if !in_tag {
            let ch = chars[i];
            if ch.is_whitespace() {
                if !last_was_space && !result.is_empty() {
                    result.push(' ');
                    last_was_space = true;
                }
            } else {
                result.push(ch);
                last_was_space = false;
            }
        }

        i += 1;
    }

    result.trim().to_string()
}
