//! Web Fetch — WASM module for fetching URLs and converting to markdown.
//!
//! Triggered by WebQuery.ExecuteFetch action. Reads the URL from entity state,
//! fetches the page, converts HTML to structured markdown, and transitions to
//! Complete with content or Failed with error.
//!
//! Results are stored inline in the entity's `results` field. Temper's
//! field-overflow primitive (temper ADR-0040 / ADR-0045) automatically
//! writes oversize values to the blob store and resolves them on read
//! via ctx.read_field_string (ADR-0046), so modules no longer need the
//! hand-rolled TemperFS File workaround.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Maximum response size to keep (100KB of chars; ~400KB bytes worst-case UTF-8).
/// Worst-case still fits within typical invocation heap budgets (WASM
/// max_memory >= 256MB in paw-agent specs) and is routed through Temper's
/// field-overflow blob path for any value above the 128KB inline ceiling.
const MAX_CONTENT_LEN: usize = 100_000;

/// Hard cap on the raw response body before HTML stripping. Pathological
/// responses (HTML bombs, CDN dumps) that exceed this are truncated with a
/// marker and a `WebFetchTruncated` event is dispatched for observability.
/// The result ALSO still goes through `MAX_CONTENT_LEN` after stripping.
/// See temperpaw ADR-0033.
const WEB_FETCH_MAX_BYTES: usize = 10 * 1024 * 1024; // 10 MB

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "web_fetch: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let url = fields.get("url").and_then(|v| v.as_str()).unwrap_or("");

        if url.is_empty() {
            set_success_result("RecordError", &json!({"error": "web_fetch: url is empty"}));
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
            ("User-Agent".to_string(), "TemperPaw/1.0".to_string()),
            (
                "Accept".to_string(),
                "text/html, text/plain, */*".to_string(),
            ),
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

        // Enforce the 10MB upstream cap before any parsing/allocation.
        // Oversize bodies are truncated with a marker and truncated_bytes is
        // recorded on the entity for observability. See temperpaw ADR-0033.
        let (body_text, truncated_upstream) = apply_upstream_cap(resp.body, WEB_FETCH_MAX_BYTES);

        // Strip HTML if the response looks like HTML
        let text = if looks_like_html(&body_text) {
            html_to_markdown(&body_text)
        } else {
            body_text
        };

        // Truncate to max size
        let truncated: String = text.chars().take(MAX_CONTENT_LEN).collect();

        if !has_readable_web_content(&truncated) {
            set_success_result(
                "RecordError",
                &json!({"error": format!("web_fetch: fetched {url} but no readable text was extracted")}),
            );
            return Ok(());
        }

        ctx.log(
            "info",
            &format!("web_fetch: got {} chars of text", truncated.len()),
        );

        // Store inline regardless of size — Temper's field-overflow path
        // (ADR-0040 / ADR-0045) handles values above the 128KB inline ceiling
        // by writing to the content-addressed blob store, and consumers
        // resolve via ctx.read_field_string (ADR-0046).
        //
        // `truncated_bytes` carries the original upstream size when the 10MB
        // cap fired (temperpaw ADR-0033); empty string means no truncation.
        let truncated_bytes_param = truncated_upstream
            .map(|n| n.to_string())
            .unwrap_or_default();
        if !truncated_bytes_param.is_empty() {
            ctx.log(
                "warn",
                &format!(
                    "web_fetch: upstream truncated at {WEB_FETCH_MAX_BYTES} bytes; original was {truncated_bytes_param} bytes"
                ),
            );
        }
        set_success_result(
            "RecordResults",
            &json!({
                "results": truncated,
                "truncated_bytes": truncated_bytes_param,
            }),
        );

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Check if the response body looks like HTML.
/// Enforce an upstream byte cap on the response body before any parsing.
///
/// Returns `(body_or_truncated, Some(original_size))` when the body exceeded
/// `max_bytes`; returns `(body, None)` otherwise. When truncated, a trailing
/// marker is appended so downstream consumers can see the boundary. The
/// `Some(original_size)` value is written to `WebQuery.truncated_bytes`.
///
/// Char-boundary safe: truncation walks back to the nearest valid UTF-8 char
/// boundary so the resulting string is well-formed. See temperpaw ADR-0033.
fn apply_upstream_cap(body: String, max_bytes: usize) -> (String, Option<usize>) {
    let body_len = body.len();
    if body_len <= max_bytes {
        return (body, None);
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !body.is_char_boundary(boundary) {
        boundary -= 1;
    }
    let mut cut = body[..boundary].to_string();
    cut.push_str(&format!(
        "\n\n[web_fetch: truncated upstream at {max_bytes} bytes; response was {body_len} bytes]"
    ));
    (cut, Some(body_len))
}

fn looks_like_html(body: &str) -> bool {
    let lower: String = body.chars().take(500).collect::<String>().to_lowercase();
    lower.contains("<html") || lower.contains("<!doctype html")
}

fn has_readable_web_content(text: &str) -> bool {
    text.chars().any(|ch| !ch.is_whitespace())
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
            let end = after
                .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .unwrap_or(after.len());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings() {
        let html = "<h1>Title</h1><h2>Subtitle</h2><h3>Section</h3>";
        let md = html_to_markdown(html);
        assert!(md.contains("# Title"), "h1: {md}");
        assert!(md.contains("## Subtitle"), "h2: {md}");
        assert!(md.contains("### Section"), "h3: {md}");
    }

    #[test]
    fn links() {
        let html = r#"<a href="https://example.com">Example</a>"#;
        let md = html_to_markdown(html);
        assert!(md.contains("[Example](https://example.com)"), "link: {md}");
    }

    #[test]
    fn unordered_list() {
        let html = "<ul><li>One</li><li>Two</li><li>Three</li></ul>";
        let md = html_to_markdown(html);
        assert!(md.contains("- One"), "ul item: {md}");
        assert!(md.contains("- Two"), "ul item: {md}");
    }

    #[test]
    fn ordered_list() {
        let html = "<ol><li>First</li><li>Second</li></ol>";
        let md = html_to_markdown(html);
        assert!(md.contains("1. First"), "ol item: {md}");
        assert!(md.contains("2. Second"), "ol item: {md}");
    }

    #[test]
    fn bold_and_italic() {
        let html = "<strong>bold</strong> and <em>italic</em>";
        let md = html_to_markdown(html);
        assert!(md.contains("**bold**"), "bold: {md}");
        assert!(md.contains("*italic*"), "italic: {md}");
    }

    #[test]
    fn inline_code() {
        let html = "Use <code>println!</code> to print";
        let md = html_to_markdown(html);
        assert!(md.contains("`println!`"), "code: {md}");
    }

    #[test]
    fn pre_block() {
        let html = "<pre>fn main() {\n  println!(\"hi\");\n}</pre>";
        let md = html_to_markdown(html);
        assert!(md.contains("```"), "fenced: {md}");
        assert!(md.contains("fn main()"), "content: {md}");
    }

    #[test]
    fn blockquote() {
        let html = "<blockquote>A wise saying</blockquote>";
        let md = html_to_markdown(html);
        assert!(md.contains("> A wise saying"), "bq: {md}");
    }

    #[test]
    fn script_and_style_stripped() {
        let html = "<p>Hello</p><script>alert('x')</script><style>.x{}</style><p>World</p>";
        let md = html_to_markdown(html);
        assert!(!md.contains("alert"), "script: {md}");
        assert!(!md.contains(".x{"), "style: {md}");
        assert!(md.contains("Hello"), "p1: {md}");
        assert!(md.contains("World"), "p2: {md}");
    }

    #[test]
    fn html_entities() {
        let html = "&amp; &lt; &gt; &quot;";
        let md = html_to_markdown(html);
        assert!(md.contains("& < > \""), "entities: {md}");
    }

    #[test]
    fn unknown_tags_stripped() {
        let html = "<div><span>text</span></div>";
        let md = html_to_markdown(html);
        assert!(md.contains("text"), "content preserved: {md}");
        assert!(!md.contains("<div>"), "div stripped: {md}");
        assert!(!md.contains("<span>"), "span stripped: {md}");
    }

    #[test]
    fn full_page() {
        let html = r#"<html><head><title>Test</title></head><body>
            <h2>API Reference</h2>
            <ul>
                <li><a href="/docs">Documentation</a></li>
                <li><a href="/api">API</a></li>
            </ul>
            <p>Read the <strong>docs</strong> for more.</p>
        </body></html>"#;
        let md = html_to_markdown(html);
        assert!(md.contains("## API Reference"), "heading: {md}");
        assert!(md.contains("[Documentation](/docs)"), "link: {md}");
        assert!(md.contains("**docs**"), "bold: {md}");
    }

    #[test]
    fn readable_content_rejects_blank_text() {
        assert!(!has_readable_web_content(""));
        assert!(!has_readable_web_content("   \n\t"));
        assert!(has_readable_web_content("headline"));
    }

    // --- Red-green tests for temperpaw ADR-0033 (10MB upstream cap) ---
    //
    // Red phase would fail before the cap was introduced because the module
    // would allocate the entire oversized body and attempt to parse it as
    // HTML, spending an unbounded amount of fuel + heap. With the cap the
    // oversize path short-circuits and the original size is surfaced to the
    // caller as `truncated_bytes`.

    #[test]
    fn apply_upstream_cap_under_cap_returns_untouched() {
        let body = "hello world".repeat(10);
        let (out, original) = apply_upstream_cap(body.clone(), 1024);
        assert_eq!(out, body, "small body passes through unmodified");
        assert!(original.is_none(), "small body is not flagged as truncated");
    }

    #[test]
    fn apply_upstream_cap_over_cap_truncates_and_marks() {
        let body = "a".repeat(2048);
        let (out, original) = apply_upstream_cap(body, 512);
        assert_eq!(original, Some(2048), "original size reported");
        assert!(out.len() < 2048, "output is shorter than input");
        assert!(
            out.contains("truncated upstream at 512 bytes"),
            "truncation marker present: {out}"
        );
        assert!(
            out.contains("response was 2048 bytes"),
            "original size in marker: {out}"
        );
    }

    #[test]
    fn apply_upstream_cap_respects_utf8_char_boundaries() {
        // Build a body where the nominal byte cap falls in the middle of a
        // multi-byte UTF-8 sequence (4-byte emoji 🦀 = 0xF0 0x9F 0xA6 0x80).
        // apply_upstream_cap must walk back to a char boundary so the
        // resulting string is well-formed.
        let mut body = String::new();
        for _ in 0..100 {
            body.push('🦀');
        }
        // 🦀 is 4 bytes; 100 crabs = 400 bytes. Cap at 101 falls mid-crab.
        let (out, original) = apply_upstream_cap(body, 101);
        assert_eq!(original, Some(400));
        // The cut portion should be all complete crabs (100/4 = 25 crabs = 100 bytes).
        assert!(out.starts_with("🦀🦀🦀🦀🦀"));
        // Must be valid UTF-8 (String guarantees, but cross-check).
        assert_eq!(out.chars().next(), Some('🦀'));
    }

    #[test]
    fn apply_upstream_cap_at_exact_cap_returns_untouched() {
        let body = "x".repeat(512);
        let (out, original) = apply_upstream_cap(body.clone(), 512);
        assert_eq!(out, body, "body at exactly cap passes through");
        assert!(original.is_none());
    }
}
