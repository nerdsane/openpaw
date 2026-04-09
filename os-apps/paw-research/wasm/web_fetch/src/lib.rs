//! Web Fetch — WASM module for fetching URLs and converting to markdown.
//!
//! Triggered by WebQuery.ExecuteFetch action. Reads the URL from entity state,
//! fetches the page, converts HTML to structured markdown, and transitions to
//! Complete with content or Failed with error.
//!
//! Results under 30KB are stored inline in the entity's `results` field.
//! Larger results are written to a TemperFS File and the `result_file_id`
//! is stored instead — this avoids the platform's 32KB entity field limit.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Maximum response size to keep (100KB).
const MAX_CONTENT_LEN: usize = 100_000;

/// Inline threshold — results smaller than this go directly in the entity field.
/// Must be under Temper's MAX_FIELD_VALUE_BYTES (32KB) to avoid truncation.
const INLINE_THRESHOLD: usize = 30_000;

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
            html_to_markdown(&resp.body)
        } else {
            resp.body
        };

        // Truncate to max size
        let truncated: String = text.chars().take(MAX_CONTENT_LEN).collect();

        ctx.log(
            "info",
            &format!("web_fetch: got {} chars of text", truncated.len()),
        );

        if truncated.len() <= INLINE_THRESHOLD {
            // Small result — store inline in entity field
            set_success_result(
                "RecordResults",
                &json!({"results": truncated}),
            );
        } else {
            // Large result — write to TemperFS File, store file_id in entity
            let temper_api_url = ctx.config
                .get("temper_api_url")
                .filter(|s| !s.is_empty() && !s.contains("{secret:"))
                .cloned()
                .unwrap_or_else(|| "http://127.0.0.1:3467".to_string());
            let tenant = &ctx.tenant;

            match write_to_temperfs(&ctx, &temper_api_url, tenant, &truncated) {
                Ok(file_id) => {
                    ctx.log("info", &format!("web_fetch: stored {} chars in TemperFS file {file_id}", truncated.len()));
                    // Store a summary inline + file_id for the full content
                    let summary: String = truncated.chars().take(500).collect();
                    set_success_result(
                        "RecordResults",
                        &json!({
                            "results": format!("{summary}\n\n[... {} total chars — full content in file {file_id}]", truncated.len()),
                            "result_file_id": file_id,
                        }),
                    );
                }
                Err(e) => {
                    ctx.log("warn", &format!("web_fetch: TemperFS write failed: {e}, falling back to truncated inline"));
                    // Fallback: store what fits inline
                    let inline: String = truncated.chars().take(INLINE_THRESHOLD).collect();
                    set_success_result(
                        "RecordResults",
                        &json!({"results": format!("{inline}\n\n[... truncated at {} chars, full content was {} chars]", INLINE_THRESHOLD, truncated.len())}),
                    );
                }
            }
        }

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Write content to a TemperFS File and return the file_id.
fn write_to_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    content: &str,
) -> Result<String, String> {
    let headers = vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        ("X-Tenant-Id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "agent".to_string()),
        ("x-temper-agent-type".to_string(), "system".to_string()),
    ];

    // Create File entity
    let entity_id = ctx.entity_state
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let file_body = json!({
        "name": format!("web-fetch-{entity_id}.md"),
        "mime_type": "text/markdown",
    });
    let create_resp = ctx.http_call(
        "POST",
        &format!("{temper_api_url}/tdata/Files"),
        &headers,
        &file_body.to_string(),
    )?;
    if create_resp.status < 200 || create_resp.status >= 300 {
        return Err(format!("File creation failed (HTTP {})", create_resp.status));
    }

    let parsed: serde_json::Value = serde_json::from_str(&create_resp.body)
        .map_err(|e| format!("parse file response: {e}"))?;
    let file_id = parsed
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if file_id.is_empty() {
        return Err("File entity missing entity_id".to_string());
    }

    // Write content via $value endpoint
    let write_headers = vec![
        ("Content-Type".to_string(), "text/plain".to_string()),
        ("X-Tenant-Id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "agent".to_string()),
        ("x-temper-agent-type".to_string(), "system".to_string()),
    ];
    let write_resp = ctx.http_call(
        "PUT",
        &format!("{temper_api_url}/tdata/Files('{file_id}')/$value"),
        &write_headers,
        content,
    )?;
    if write_resp.status < 200 || write_resp.status >= 300 {
        return Err(format!("File write failed (HTTP {})", write_resp.status));
    }

    Ok(file_id)
}

/// Check if the response body looks like HTML.
fn looks_like_html(body: &str) -> bool {
    let lower: String = body.chars().take(500).collect::<String>().to_lowercase();
    lower.contains("<html") || lower.contains("<!doctype html")
}

/// Track ordered vs unordered list nesting for markdown conversion.
#[derive(Clone, Debug)]
enum ListKind {
    Unordered,
    Ordered(usize), // current item counter
}

/// Convert HTML to structured markdown using character-by-character parsing.
///
/// Handles headings, paragraphs, links, lists, inline formatting, code blocks,
/// blockquotes, and common HTML entities. Skips `<script>` and `<style>` blocks
/// and HTML comments entirely. Collapses runs of 3+ newlines to 2.
fn html_to_markdown(html: &str) -> String {
    let mut result = String::with_capacity(html.len() / 2);
    let chars: Vec<char> = html.chars().collect();
    let len = chars.len();
    let mut i = 0;

    // State
    let mut skip_block: Option<String> = None; // "script" or "style"
    let mut list_stack: Vec<ListKind> = Vec::new();
    let mut in_pre = false;

    while i < len {
        // --- skip <script>/<style> blocks ---
        if let Some(ref tag) = skip_block.clone() {
            if chars[i] == '<' && i + 2 < len && chars[i + 1] == '/' {
                let rest: String = chars[i..].iter().take(tag.len() + 3).collect();
                let close = format!("</{}>", tag);
                if rest.to_lowercase().starts_with(&close) {
                    i += close.len();
                    skip_block = None;
                    continue;
                }
            }
            i += 1;
            continue;
        }

        // --- HTML comment <!-- ... --> ---
        if i + 3 < len && &chars[i..i + 4] == &['<', '!', '-', '-'] {
            // skip until -->
            i += 4;
            while i + 2 < len {
                if chars[i] == '-' && chars[i + 1] == '-' && chars[i + 2] == '>' {
                    i += 3;
                    break;
                }
                i += 1;
            }
            continue;
        }

        // --- HTML entity ---
        if chars[i] == '&' && !in_pre {
            if let Some((decoded, advance)) = decode_entity(&chars, i) {
                result.push_str(&decoded);
                i += advance;
                continue;
            }
        }
        // Also decode entities inside <pre> (they're still entities)
        if chars[i] == '&' && in_pre {
            if let Some((decoded, advance)) = decode_entity(&chars, i) {
                result.push_str(&decoded);
                i += advance;
                continue;
            }
        }

        // --- Tag ---
        if chars[i] == '<' {
            // Collect the full tag up to '>'
            let _tag_start = i;
            i += 1; // skip '<'
            let is_closing = i < len && chars[i] == '/';
            if is_closing {
                i += 1;
            }
            // Collect tag name
            let mut tag_name = String::new();
            while i < len && chars[i] != '>' && chars[i] != ' ' && chars[i] != '/' {
                tag_name.push(chars[i].to_ascii_lowercase());
                i += 1;
            }
            // Collect attributes (needed for <a href="...">)
            let mut attrs = String::new();
            while i < len && chars[i] != '>' {
                attrs.push(chars[i]);
                i += 1;
            }
            if i < len && chars[i] == '>' {
                i += 1; // skip '>'
            }
            let attrs = attrs.trim().to_string();

            // Self-closing detection (e.g. <br />, <br/>)
            let _self_closing = attrs.ends_with('/') || tag_name.is_empty();

            // Skip blocks
            if !is_closing && (tag_name == "script" || tag_name == "style") {
                skip_block = Some(tag_name);
                continue;
            }

            // Process by tag name
            match tag_name.as_str() {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    if is_closing {
                        result.push('\n');
                    } else {
                        ensure_newline(&mut result);
                        let level = tag_name[1..].parse::<usize>().unwrap_or(1);
                        for _ in 0..level {
                            result.push('#');
                        }
                        result.push(' ');
                    }
                }
                "p" => {
                    if is_closing {
                        result.push_str("\n\n");
                    } else {
                        ensure_double_newline(&mut result);
                    }
                }
                "br" => {
                    result.push('\n');
                }
                "a" => {
                    if is_closing {
                        // handled inline; the closing tag is consumed
                    } else {
                        // Extract href from attrs
                        let href = extract_attr(&attrs, "href").unwrap_or_default();
                        // Collect inner text until </a>
                        let (link_text, new_i) = collect_until_close(&chars, i, "a");
                        i = new_i;
                        if href.is_empty() {
                            result.push_str(&link_text);
                        } else {
                            result.push('[');
                            result.push_str(&link_text);
                            result.push_str("](");
                            result.push_str(&href);
                            result.push(')');
                        }
                    }
                }
                "ul" => {
                    if is_closing {
                        list_stack.pop();
                        if list_stack.is_empty() {
                            result.push('\n');
                        }
                    } else {
                        ensure_newline(&mut result);
                        list_stack.push(ListKind::Unordered);
                    }
                }
                "ol" => {
                    if is_closing {
                        list_stack.pop();
                        if list_stack.is_empty() {
                            result.push('\n');
                        }
                    } else {
                        ensure_newline(&mut result);
                        list_stack.push(ListKind::Ordered(0));
                    }
                }
                "li" => {
                    if !is_closing {
                        ensure_newline(&mut result);
                        let indent_level = if list_stack.len() > 1 {
                            list_stack.len() - 1
                        } else {
                            0
                        };
                        for _ in 0..indent_level {
                            result.push_str("  ");
                        }
                        if let Some(kind) = list_stack.last_mut() {
                            match kind {
                                ListKind::Unordered => {
                                    result.push_str("- ");
                                }
                                ListKind::Ordered(n) => {
                                    *n += 1;
                                    let num = *n;
                                    result.push_str(&format!("{}. ", num));
                                }
                            }
                        } else {
                            result.push_str("- ");
                        }
                    }
                }
                "strong" | "b" => {
                    result.push_str("**");
                }
                "em" | "i" => {
                    result.push('*');
                }
                "code" => {
                    if !in_pre {
                        result.push('`');
                    }
                }
                "pre" => {
                    if is_closing {
                        in_pre = false;
                        result.push_str("\n```\n");
                    } else {
                        in_pre = true;
                        ensure_newline(&mut result);
                        result.push_str("```\n");
                    }
                }
                "blockquote" => {
                    if is_closing {
                        result.push('\n');
                    } else {
                        ensure_newline(&mut result);
                        result.push_str("> ");
                    }
                }
                "div" | "section" | "article" | "header" | "footer" | "main" | "nav" => {
                    ensure_newline(&mut result);
                }
                _ => {
                    // Unknown tags: strip silently
                }
            }
            continue;
        }

        // --- Normal text ---
        if in_pre {
            result.push(chars[i]);
        } else {
            let ch = chars[i];
            // Collapse whitespace outside <pre>
            if ch.is_whitespace() {
                if !result.is_empty() && !result.ends_with(' ') && !result.ends_with('\n') {
                    result.push(' ');
                }
            } else {
                result.push(ch);
            }
        }
        i += 1;
    }

    // Collapse runs of 3+ newlines to 2
    collapse_newlines(&mut result);
    result.trim().to_string()
}

/// Ensure the result ends with at least one newline.
fn ensure_newline(result: &mut String) {
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
}

/// Ensure the result ends with a double newline (paragraph break).
fn ensure_double_newline(result: &mut String) {
    if result.is_empty() {
        return;
    }
    // Trim trailing spaces
    while result.ends_with(' ') {
        result.pop();
    }
    if !result.ends_with("\n\n") {
        if result.ends_with('\n') {
            result.push('\n');
        } else {
            result.push_str("\n\n");
        }
    }
}

/// Collapse runs of 3+ newlines to exactly 2 newlines.
fn collapse_newlines(s: &mut String) {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut newline_count = 0;
    for &b in bytes {
        if b == b'\n' {
            newline_count += 1;
            if newline_count <= 2 {
                out.push('\n');
            }
        } else {
            newline_count = 0;
            out.push(b as char);
        }
    }
    *s = out;
}

/// Collect text content until a closing tag is found.
/// Returns the collected text and the new index after the closing tag.
fn collect_until_close(chars: &[char], start: usize, tag: &str) -> (String, usize) {
    let mut text = String::new();
    let mut i = start;
    let close_tag = format!("</{}>", tag);
    let close_len = close_tag.len();
    while i < chars.len() {
        if chars[i] == '<' && i + close_len <= chars.len() {
            let candidate: String = chars[i..i + close_len].iter().collect();
            if candidate.to_lowercase() == close_tag {
                return (text, i + close_len);
            }
        }
        // Decode entities inside link text
        if chars[i] == '&' {
            if let Some((decoded, advance)) = decode_entity(chars, i) {
                text.push_str(&decoded);
                i += advance;
                continue;
            }
        }
        // Skip nested tags inside link text
        if chars[i] == '<' {
            // skip the tag
            while i < chars.len() && chars[i] != '>' {
                i += 1;
            }
            if i < chars.len() {
                i += 1; // skip '>'
            }
            continue;
        }
        text.push(chars[i]);
        i += 1;
    }
    (text, i)
}

/// Extract an attribute value from a tag's attribute string.
/// e.g. extract_attr(r#"href="https://example.com" class="link""#, "href")
fn extract_attr(attrs: &str, name: &str) -> Option<String> {
    let search = format!("{}=", name);
    let lower = attrs.to_lowercase();
    if let Some(pos) = lower.find(&search) {
        let after = &attrs[pos + search.len()..];
        let after = after.trim_start();
        if after.starts_with('"') {
            let inner = &after[1..];
            if let Some(end) = inner.find('"') {
                return Some(inner[..end].to_string());
            }
        } else if after.starts_with('\'') {
            let inner = &after[1..];
            if let Some(end) = inner.find('\'') {
                return Some(inner[..end].to_string());
            }
        } else {
            // Unquoted attribute value
            let end = after.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(after.len());
            return Some(after[..end].to_string());
        }
    }
    None
}

/// Decode an HTML entity starting at position `i` in `chars`.
/// Returns the decoded string and how many chars to advance.
fn decode_entity(chars: &[char], i: usize) -> Option<(String, usize)> {
    if chars[i] != '&' {
        return None;
    }
    // Collect until ';' or max 10 chars
    let mut entity = String::new();
    let mut j = i + 1;
    while j < chars.len() && j - i < 12 && chars[j] != ';' {
        entity.push(chars[j]);
        j += 1;
    }
    if j >= chars.len() || chars[j] != ';' {
        return None;
    }
    let advance = j - i + 1; // include '&' and ';'
    let decoded = match entity.as_str() {
        "amp" => "&".to_string(),
        "lt" => "<".to_string(),
        "gt" => ">".to_string(),
        "quot" => "\"".to_string(),
        "apos" => "'".to_string(),
        "nbsp" => " ".to_string(),
        _ if entity.starts_with('#') => {
            let num_str = &entity[1..];
            let code_point = if num_str.starts_with('x') || num_str.starts_with('X') {
                u32::from_str_radix(&num_str[1..], 16).ok()
            } else {
                num_str.parse::<u32>().ok()
            };
            if let Some(cp) = code_point {
                if let Some(ch) = char::from_u32(cp) {
                    ch.to_string()
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some((decoded, advance))
}
