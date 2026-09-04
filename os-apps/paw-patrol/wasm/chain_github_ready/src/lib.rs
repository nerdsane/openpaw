//! chain_github_ready — one concern: the named path exists on GitHub.
//!
//! Attach* writes a git path (`docs/efforts/ARN-441/spec.md`). This module
//! GETs the GitHub contents API for repo + branch + path. Missing or empty
//! file → set_error_result so on_failure retracts the ready bool.
//!
//! Does not dispatch. Does not write files. Temper Files stay for review/proof.

use temper_wasm_sdk::prelude::*;

#[cfg(target_arch = "wasm32")]
getrandom::register_custom_getrandom!(unsupported_random);

#[cfg(target_arch = "wasm32")]
fn unsupported_random(_buf: &mut [u8]) -> Result<(), getrandom::Error> {
    Err(getrandom::Error::UNSUPPORTED)
}

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
        let mut headers = vec![
            (
                "accept".to_string(),
                "application/vnd.github+json".to_string(),
            ),
            (
                "user-agent".to_string(),
                "temperpaw-chain-github-ready".to_string(),
            ),
        ];
        let cred = bearer_for_repo(&ctx, &repo)?;
        if let Some(token) = cred.token.as_deref() {
            headers.push(("authorization".to_string(), format!("Bearer {token}")));
        }
        // Private repos the token cannot read also return 404 on contents.
        // Probe the repo first so a visibility miss is not reported as a missing file.
        let repo_url = repo_url(&repo);
        let repo_resp = ctx.http_call("GET", &repo_url, &headers, "")?;
        if repo_resp.status == 404 {
            return Err(cannot_see(&repo, cred.label));
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

fn secret(ctx: &Context, key: &str) -> Option<String> {
    ctx.config
        .get(key)
        .filter(|v| !v.is_empty() && !v.contains("{secret:"))
        .cloned()
}

struct GitHubCred {
    token: Option<String>,
    label: &'static str,
}

fn cannot_see(repo: &str, label: &str) -> String {
    match label {
        "app" => format!(
            "chain_github_ready: GitHub App installation cannot see {repo} (no install on this owner, or the install cannot read it)"
        ),
        "github_token" => format!(
            "chain_github_ready: tenant github_token cannot see {repo} (missing, or private and this token has no access)"
        ),
        _ => format!(
            "chain_github_ready: GitHub has no public repo {repo} (no App install on this owner and no github_token)"
        ),
    }
}

fn bearer_for_repo(ctx: &Context, repo: &str) -> Result<GitHubCred, String> {
    let owner = repo.split('/').next().unwrap_or("");
    let app_id = secret(ctx, "github_app_id");
    let pem = secret(ctx, "github_app_private_key");
    match (app_id, pem) {
        (Some(app_id), Some(pem)) => {
            if let Some(token) = installation_token(ctx, &app_id, &pem, owner)? {
                return Ok(GitHubCred {
                    token: Some(token),
                    label: "app",
                });
            }
            // App is the factory credential. This owner has no install — try public.
            return Ok(GitHubCred {
                token: None,
                label: "anonymous",
            });
        }
        _ => {}
    }
    if let Some(token) = secret(ctx, "github_token") {
        return Ok(GitHubCred {
            token: Some(token),
            label: "github_token",
        });
    }
    Err("chain_github_ready: tenant GitHub App (github_app_id + github_app_private_key) is not configured".to_string())
}

fn installation_token(
    ctx: &Context,
    app_id: &str,
    pem: &str,
    owner: &str,
) -> Result<Option<String>, String> {
    let jwt = github_app_jwt(app_id, pem)?;
    let auth = vec![
        (
            "accept".to_string(),
            "application/vnd.github+json".to_string(),
        ),
        (
            "user-agent".to_string(),
            "temperpaw-chain-github-ready".to_string(),
        ),
        ("authorization".to_string(), format!("Bearer {jwt}")),
    ];
    let resp = ctx.http_call(
        "GET",
        "https://api.github.com/app/installations?per_page=100",
        &auth,
        "",
    )?;
    if resp.status >= 400 {
        return Err(format!(
            "chain_github_ready: list installations HTTP {}",
            resp.status
        ));
    }
    let installs: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("chain_github_ready: installations: {e}"))?;
    let Some(id) = install_id_for_owner(&installs, owner) else {
        return Ok(None);
    };
    let url = format!("https://api.github.com/app/installations/{id}/access_tokens");
    let minted = ctx.http_call("POST", &url, &auth, "")?;
    if minted.status >= 400 {
        return Err(format!(
            "chain_github_ready: mint installation token HTTP {}",
            minted.status
        ));
    }
    let body: Value = serde_json::from_str(&minted.body)
        .map_err(|e| format!("chain_github_ready: access_token: {e}"))?;
    let token = body
        .get("token")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "chain_github_ready: installation token missing".to_string())?;
    Ok(Some(token.to_string()))
}

fn install_id_for_owner(installs: &Value, owner: &str) -> Option<u64> {
    let rows = installs.as_array()?;
    for row in rows {
        let login = row
            .get("account")
            .and_then(|a| a.get("login"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if login.eq_ignore_ascii_case(owner) {
            return row.get("id").and_then(|v| v.as_u64());
        }
    }
    None
}

fn github_app_jwt(app_id: &str, pem: &str) -> Result<String, String> {
    use rsa::RsaPrivateKey;
    use rsa::pkcs1::DecodeRsaPrivateKey;
    use rsa::pkcs1v15::Pkcs1v15Sign;
    use rsa::pkcs8::DecodePrivateKey;
    use rsa::sha2::{Digest, Sha256};

    let now = now_secs()?;
    let header = b64url(br#"{"alg":"RS256","typ":"JWT"}"#);
    let payload = b64url(
        format!(
            r#"{{"iat":{},"exp":{},"iss":"{app_id}"}}"#,
            now.saturating_sub(60),
            now + 540
        )
        .as_bytes(),
    );
    let signing_input = format!("{header}.{payload}");
    let key = RsaPrivateKey::from_pkcs1_pem(pem)
        .or_else(|_| RsaPrivateKey::from_pkcs8_pem(pem))
        .map_err(|e| format!("chain_github_ready: github_app_private_key: {e}"))?;
    let digest = Sha256::digest(signing_input.as_bytes());
    let sig = key
        .sign(Pkcs1v15Sign::new::<Sha256>(), &digest)
        .map_err(|e| format!("chain_github_ready: jwt sign: {e}"))?;
    Ok(format!("{signing_input}.{}", b64url(&sig)))
}

fn b64url(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i];
        let b1 = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
        let b2 = if i + 2 < bytes.len() { bytes[i + 2] } else { 0 };
        out.push(T[(b0 >> 2) as usize] as char);
        out.push(T[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if i + 1 < bytes.len() {
            out.push(T[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        }
        if i + 2 < bytes.len() {
            out.push(T[(b2 & 0x3f) as usize] as char);
        }
        i += 3;
    }
    out
}

fn now_secs() -> Result<u64, String> {
    Ok((Context::get_time_millis() / 1000) as u64)
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
        return Err("chain_github_ready: path must be docs/efforts/<id>/<name>.md".to_string());
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
    if owner
        .chars()
        .any(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
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
        assert_eq!(
            github_repo("nerdsane/temperpaw").unwrap(),
            "nerdsane/temperpaw"
        );
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

    #[test]
    fn b64url_has_no_padding() {
        assert_eq!(b64url(b"f"), "Zg");
        assert_eq!(b64url(b"fo"), "Zm8");
        assert_eq!(b64url(b"foo"), "Zm9v");
    }

    #[test]
    fn picks_install_for_owner() {
        let installs = json!([
            {"id": 11, "account": {"login": "nerdsane"}},
            {"id": 22, "account": {"login": "arni-labs"}}
        ]);
        assert_eq!(install_id_for_owner(&installs, "arni-labs"), Some(22));
        assert_eq!(install_id_for_owner(&installs, "missing"), None);
    }
}
