//! chain_github_ready — one concern: the named path exists on GitHub.
//!
//! Attach* writes a git path (`docs/efforts/ARN-441/spec.md`). This module
//! GETs the GitHub contents API for repo + branch + path. Missing or empty
//! file → set_error_result so on_failure retracts the ready bool.
//!
//! Does not dispatch. Does not write files. Temper Files stay for review/proof.

use temper_wasm_sdk::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let path_field = cfg(&ctx, "path_field")?;
        let repo_field = ctx
            .config
            .get("repo_field")
            .map(String::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or("repo");
        let branch_field = ctx
            .config
            .get("branch_field")
            .map(String::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or("branch");
        let path = field_or_param(&ctx, &fields, path_field)?;
        let repo = field_or_param(&ctx, &fields, repo_field)?;
        let branch = field_or_param(&ctx, &fields, branch_field)?;
        let path = git_path(&path)?;
        let repo = github_repo(&repo)?;
        let token = ctx
            .config
            .get("github_token")
            .filter(|v| !v.is_empty() && !v.contains("{secret:"))
            .cloned()
            .unwrap_or_default();
        let mut headers = vec![
            (
                "accept".to_string(),
                "application/vnd.github+json".to_string(),
            ),
            ("user-agent".to_string(), "temperpaw-chain-github-ready".to_string()),
        ];
        if token.is_empty() {
            return Err(
                "chain_github_ready: tenant secret github_token is not configured"
                    .to_string(),
            );
        }
        headers.push(("authorization".to_string(), format!("Bearer {token}")));
        // Private repos the token cannot read also return 404 on contents.
        // Probe the repo first so a visibility miss is not reported as a missing file.
        let repo_url = repo_url(&repo);
        let repo_resp = ctx.http_call("GET", &repo_url, &headers, "")?;
        if repo_resp.status == 404 {
            return Err(format!(
                "chain_github_ready: tenant github_token cannot see {repo} (missing, or private and this token has no access)"
            ));
        }
        if repo_resp.status >= 400 {
            return Err(format!(
                "chain_github_ready: GET {repo_url} HTTP {}",
                repo_resp.status
            ));
        }
        let url = contents_url(&repo, &path, &branch);
        let resp = ctx.http_call("GET", &url, &headers, "")?;
        if resp.status == 404 {
            return Err(format!(
                "chain_github_ready: {repo}@{branch}:{path} is not on GitHub"
            ));
        }
        if resp.status >= 400 {
            return Err(format!(
                "chain_github_ready: GET {url} HTTP {}",
                resp.status
            ));
        }
        let body: Value = serde_json::from_str(&resp.body)
            .map_err(|e| format!("chain_github_ready: body: {e}"))?;
        if !github_file_present(&body) {
            return Err(format!(
                "chain_github_ready: {repo}@{branch}:{path} is not a file"
            ));
        }
        ctx.log(
            "info",
            &format!("chain_github_ready: {repo}@{branch}:{path} is on GitHub"),
        );
        set_success_result("", &json!({ "repo": repo, "branch": branch, "path": path }));
        Ok(())
    })();
    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}

fn cfg<'a>(ctx: &'a Context, key: &str) -> Result<&'a str, String> {
    ctx.config
        .get(key)
        .map(String::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("chain_github_ready: missing config {key}"))
}

fn field_or_param(ctx: &Context, fields: &Value, field: &str) -> Result<String, String> {
    let from_param = ctx
        .trigger_params
        .get(field)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let from_field = fields
        .get(field)
        .and_then(|v| v.as_str())
        .or_else(|| fields.get(&pascal(field)).and_then(|v| v.as_str()))
        .unwrap_or("")
        .trim();
    let value = if !from_param.is_empty() {
        from_param
    } else {
        from_field
    };
    if value.is_empty() {
        return Err(format!("chain_github_ready: empty {field}"));
    }
    Ok(value.to_string())
}

fn pascal(field: &str) -> String {
    field
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn git_path(path: &str) -> Result<String, String> {
    if path.starts_with('/') || path.contains("..") || path.contains('\\') {
        return Err("chain_github_ready: path is not a repo-relative git path".to_string());
    }
    if !path.starts_with("docs/efforts/") || !path.ends_with(".md") {
        return Err(
            "chain_github_ready: path must be docs/efforts/<id>/<name>.md".to_string(),
        );
    }
    Ok(path.to_string())
}

fn github_repo(repo: &str) -> Result<String, String> {
    let mut parts = repo.split('/');
    let owner = parts.next().unwrap_or("");
    let name = parts.next().unwrap_or("");
    if owner.is_empty() || name.is_empty() || parts.next().is_some() {
        return Err("chain_github_ready: repo must be owner/name".to_string());
    }
    if owner.chars().any(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
        || name
            .chars()
            .any(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != '.')
    {
        return Err("chain_github_ready: repo must be owner/name".to_string());
    }
    Ok(format!("{owner}/{name}"))
}

fn repo_url(repo: &str) -> String {
    format!("https://api.github.com/repos/{repo}")
}

fn contents_url(repo: &str, path: &str, branch: &str) -> String {
    let encoded_path = path
        .split('/')
        .map(urlencode_segment)
        .collect::<Vec<_>>()
        .join("/");
    format!(
        "https://api.github.com/repos/{repo}/contents/{encoded_path}?ref={}",
        urlencode_segment(branch)
    )
}

fn urlencode_segment(segment: &str) -> String {
    let mut out = String::new();
    for b in segment.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn github_file_present(body: &Value) -> bool {
    body.get("type").and_then(|v| v.as_str()) == Some("file")
        && body
            .get("sha")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_effort_markdown_paths() {
        assert!(git_path("docs/efforts/ARN-441/spec.md").is_ok());
        assert!(git_path("/docs/efforts/ARN-441/spec.md").is_err());
        assert!(git_path("docs/efforts/../secret.md").is_err());
        assert!(git_path("os-apps/foo.md").is_err());
    }

    #[test]
    fn accepts_owner_repo() {
        assert_eq!(github_repo("nerdsane/temperpaw").unwrap(), "nerdsane/temperpaw");
        assert!(github_repo("temperpaw").is_err());
        assert!(github_repo("https://github.com/nerdsane/temperpaw").is_err());
    }

    #[test]
    fn contents_url_encodes_path() {
        let url = contents_url("nerdsane/temperpaw", "docs/efforts/ARN-441/spec.md", "main");
        assert_eq!(
            url,
            "https://api.github.com/repos/nerdsane/temperpaw/contents/docs/efforts/ARN-441/spec.md?ref=main"
        );
        assert_eq!(
            repo_url("arni-labs/aya"),
            "https://api.github.com/repos/arni-labs/aya"
        );
    }

    #[test]
    fn file_payload_passes() {
        assert!(github_file_present(&json!({"type": "file", "sha": "abc"})));
        assert!(!github_file_present(&json!({"type": "dir", "sha": "abc"})));
        assert!(!github_file_present(&json!({"type": "file", "sha": ""})));
    }
}
