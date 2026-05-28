//! Path utilities — pure string operations for filesystem path handling.

/// Normalize a Unix-style path: ensure leading `/`, collapse `//`, strip trailing `/`.
pub fn normalize(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("path must not be empty".into());
    }

    let with_slash = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };

    // Collapse consecutive slashes and strip trailing slash.
    let mut result = String::with_capacity(with_slash.len());
    let mut prev_slash = false;
    for ch in with_slash.chars() {
        if ch == '/' {
            if !prev_slash {
                result.push('/');
            }
            prev_slash = true;
        } else {
            result.push(ch);
            prev_slash = false;
        }
    }

    // Strip trailing slash (unless root).
    if result.len() > 1 && result.ends_with('/') {
        result.pop();
    }

    Ok(result)
}

/// Split a normalized path into `(dir_path, filename)`.
///
/// - `"/wiki/grammar/particles.md"` → `("/wiki/grammar", "particles.md")`
/// - `"/README.md"` → `("/", "README.md")`
pub fn parse(path: &str) -> Result<(&str, &str), String> {
    match path.rsplit_once('/') {
        Some(("", filename)) if !filename.is_empty() => Ok(("/", filename)),
        Some((dir, filename)) if !filename.is_empty() => Ok((dir, filename)),
        _ => Err(format!("invalid path (no filename): {path}")),
    }
}

/// Infer MIME type from file extension.
pub fn mime_from_ext(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "md" | "markdown" => "text/markdown",
        "txt" => "text/plain",
        "json" => "application/json",
        "yaml" | "yml" => "application/yaml",
        "toml" => "application/toml",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "ts" => "application/typescript",
        "rs" => "text/x-rust",
        "py" => "text/x-python",
        "sh" => "text/x-shellscript",
        "xml" => "application/xml",
        "csv" => "text/csv",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// Split a directory path into individual segment names.
///
/// - `"/"` → `vec![]`
/// - `"/wiki/grammar"` → `vec!["wiki", "grammar"]`
pub fn dir_segments(dir_path: &str) -> Vec<&str> {
    dir_path.split('/').filter(|s| !s.is_empty()).collect()
}
