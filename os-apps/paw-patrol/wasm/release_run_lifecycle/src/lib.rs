//! release_run_lifecycle — side effects for the ReleaseRun automaton, run on
//! the named Computer's sandbox.
//!
//! One trigger, one side effect, one named callback. The module never
//! dispatches actions on other entities and never loops; the automaton and
//! the kernel `state_timeout` own the orchestration (see release_run.ioa.toml).
//!
//! | trigger action   | side effect on the computer                         | reports              |
//! |------------------|-----------------------------------------------------|----------------------|
//! | Request          | merge the PR via the GitHub API (repo token on box) | MergeSucceeded       |
//! | Check            | one `curl` of health_url                            | CheckHealthy / CheckPending / CheckUnhealthy |
//! | CheckUnhealthy   | `git revert -m 1 <merge_sha>` + push to main        | RollbackPushed       |
//!
//! Any error surfaces through `set_error_result`, which the spec routes to
//! `Fail` (on_failure) so nothing fails silently.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;
use wasm_helpers::sandbox::{self, SandboxHandle, normalize_sandbox_provider};
use wasm_helpers::{bounded_reads, entity_field_str, odata_headers, resolve_temper_api_url};

/// Marker separating the probe body from the HTTP status on the last line.
const HTTP_STATUS_MARKER: &str = "__HTTP_STATUS";

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let release = ReleaseFields::from_state(&ctx, &fields)?;

        ctx.log(
            "info",
            &format!(
                "release_run_lifecycle: {} on {} (repo {} pr {} computer {})",
                ctx.trigger_action, ctx.entity_id, release.repo, release.pr_number, release.computer_id
            ),
        );

        let temper_api_url = resolve_temper_api_url(&ctx, &fields);
        let computer = fetch_computer(&ctx, &temper_api_url, &fields, &release.computer_id)?;
        let handle = sandbox_handle_from_computer(&computer)
            .map_err(|e| format!("release_run_lifecycle: computer {}: {e}", release.computer_id))?;

        let (action, params) = match ctx.trigger_action.as_str() {
            "Request" => merge(&ctx, &handle, &release)?,
            "Check" => check(&ctx, &handle, &release)?,
            "CheckUnhealthy" => rollback(&ctx, &handle, &release)?,
            other => return Err(format!("release_run_lifecycle: unsupported trigger {other}")),
        };
        set_success_result(action, &params);
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// The ReleaseRun fields every step needs, read from trigger params first
/// (the Request action carries them) and entity state otherwise.
struct ReleaseFields {
    repo: String,
    pr_number: String,
    computer_id: String,
    health_url: String,
    max_checks: u64,
    merge_sha: String,
    check_count: u64,
}

impl ReleaseFields {
    fn from_state(ctx: &Context, fields: &Value) -> Result<Self, String> {
        let required = |key: &str| {
            param_or_field(ctx, fields, key)
                .ok_or_else(|| format!("release_run_lifecycle: missing {key}"))
        };
        Ok(Self {
            repo: required("repo")?,
            pr_number: required("pr_number")?,
            computer_id: required("computer_id")?,
            health_url: param_or_field(ctx, fields, "health_url").unwrap_or_default(),
            max_checks: param_or_field(ctx, fields, "max_checks")
                .and_then(|s| s.parse().ok())
                .unwrap_or(20),
            merge_sha: param_or_field(ctx, fields, "merge_sha").unwrap_or_default(),
            check_count: counter_field(fields, "check_count"),
        })
    }
}

// -- Step 1: merge -------------------------------------------------------------

fn merge(
    ctx: &Context,
    handle: &SandboxHandle,
    release: &ReleaseFields,
) -> Result<(&'static str, Value), String> {
    validate_repo(&release.repo)?;
    validate_number(&release.pr_number, "pr_number")?;
    let command = merge_command(&release.repo, &release.pr_number);
    let result = sandbox::sandbox_exec(ctx, handle, &command, "/")?;
    let merge_sha = parse_merge_sha(&result.stdout).map_err(|e| {
        format!(
            "release_run_lifecycle: merge of {}#{} did not complete: {e} (exit {}, stderr: {})",
            release.repo,
            release.pr_number,
            result.exit_code,
            excerpt(&result.stderr)
        )
    })?;
    ctx.log(
        "info",
        &format!("release_run_lifecycle: merged {}#{} as {merge_sha}", release.repo, release.pr_number),
    );
    Ok(("MergeSucceeded", json!({ "merge_sha": merge_sha })))
}

/// Merge the PR through the GitHub API from the computer, using the repo
/// token already stored there (`~/.git-credentials`, the same credential the
/// computer pushes with). Prints the API response so the merge sha can be read.
fn merge_command(repo: &str, pr_number: &str) -> String {
    format!(
        "TOK=$(grep -oE '[^/:]+:[^@]+@github.com' ~/.git-credentials | head -1 | cut -d: -f2 | cut -d@ -f1); \
         curl -sS -m 60 -X PUT \"https://api.github.com/repos/{repo}/pulls/{pr_number}/merge\" \
         -H \"Authorization: token $TOK\" -H \"Accept: application/vnd.github+json\" \
         -d '{{\"merge_method\":\"merge\"}}'"
    )
}

/// Read `sha` out of the GitHub merge response; a non-merge response
/// (`merged: false`, an error `message`) becomes an error carrying that text.
fn parse_merge_sha(stdout: &str) -> Result<String, String> {
    let body: Value = serde_json::from_str(stdout.trim())
        .map_err(|_| format!("unexpected GitHub response: {}", excerpt(stdout)))?;
    if body.get("merged").and_then(Value::as_bool) == Some(true) {
        return body
            .get("sha")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .ok_or_else(|| "merge response carried no sha".to_string());
    }
    Err(body
        .get("message")
        .and_then(Value::as_str)
        .map(|m| format!("GitHub refused the merge: {m}"))
        .unwrap_or_else(|| format!("GitHub did not merge: {}", excerpt(stdout))))
}

// -- Step 2: one health probe ----------------------------------------------------

fn check(
    ctx: &Context,
    handle: &SandboxHandle,
    release: &ReleaseFields,
) -> Result<(&'static str, Value), String> {
    if release.health_url.is_empty() {
        return Err("release_run_lifecycle: no health_url to watch".to_string());
    }
    if release.merge_sha.is_empty() {
        return Err("release_run_lifecycle: no merge_sha to watch for".to_string());
    }
    let command = probe_command(&release.health_url);
    let result = sandbox::sandbox_exec(ctx, handle, &command, "/")?;
    let probe = parse_probe(&result.stdout);
    let verdict = evaluate_probe(&probe, &release.merge_sha, release.check_count, release.max_checks);
    ctx.log(
        "info",
        &format!(
            "release_run_lifecycle: check {}/{} -> http {} status {:?} sha {:?}: {:?}",
            release.check_count, release.max_checks, probe.http_status, probe.status, probe.git_sha, verdict
        ),
    );
    Ok(match verdict {
        Verdict::Healthy => (
            "CheckHealthy",
            json!({ "observed_sha": probe.git_sha.unwrap_or_default() }),
        ),
        Verdict::Pending => (
            "CheckPending",
            json!({ "observed_sha": probe.git_sha.unwrap_or_default() }),
        ),
        Verdict::Unhealthy(reason) => (
            "CheckUnhealthy",
            json!({ "reason": reason, "observed_sha": probe.git_sha.unwrap_or_default() }),
        ),
    })
}

/// `curl` the health endpoint from the computer; the HTTP status is appended
/// on its own marker line so a body that is not JSON still yields a verdict.
fn probe_command(health_url: &str) -> String {
    format!("curl -sS -m 15 -w '\\n{HTTP_STATUS_MARKER} %{{http_code}}' '{health_url}'")
}

/// What one probe observed.
#[derive(Debug, Default, PartialEq)]
struct Probe {
    http_status: u16,
    status: Option<String>,
    git_sha: Option<String>,
}

fn parse_probe(stdout: &str) -> Probe {
    let (body, status_line) = match stdout.rfind(HTTP_STATUS_MARKER) {
        Some(idx) => (&stdout[..idx], &stdout[idx + HTTP_STATUS_MARKER.len()..]),
        None => (stdout, ""),
    };
    let http_status = status_line.trim().parse::<u16>().unwrap_or(0);
    let json: Value = serde_json::from_str(body.trim()).unwrap_or(Value::Null);
    Probe {
        http_status,
        status: json.get("status").and_then(Value::as_str).map(str::to_string),
        git_sha: json.get("git_sha").and_then(Value::as_str).map(str::to_string),
    }
}

#[derive(Debug, PartialEq)]
enum Verdict {
    Healthy,
    Pending,
    Unhealthy(String),
}

/// Decide the outcome of one probe.
///
/// - Healthy: HTTP 2xx, `status == healthy`, and the served `git_sha` is the
///   merge commit — the rollout landed and is serving cleanly.
/// - Unhealthy: the new commit is being served but reports degraded, or the
///   probe budget (`max_checks`) is spent without the new commit becoming
///   healthy. Both trigger the rollback.
/// - Pending: anything else while budget remains (old build still serving,
///   deploy in progress, transient non-2xx during the swap).
fn evaluate_probe(probe: &Probe, merge_sha: &str, check_count: u64, max_checks: u64) -> Verdict {
    let serving_new = probe.git_sha.as_deref() == Some(merge_sha);
    let ok = (200..300).contains(&probe.http_status);
    let healthy = probe.status.as_deref() == Some("healthy");
    if serving_new && ok && healthy {
        return Verdict::Healthy;
    }
    if serving_new && ok && !healthy {
        return Verdict::Unhealthy(format!(
            "new commit {merge_sha} is serving but reports status {}",
            probe.status.as_deref().unwrap_or("unknown")
        ));
    }
    if check_count >= max_checks {
        return Verdict::Unhealthy(format!(
            "rollout of {merge_sha} not healthy after {check_count} checks (last: http {}, status {}, sha {})",
            probe.http_status,
            probe.status.as_deref().unwrap_or("none"),
            probe.git_sha.as_deref().unwrap_or("none")
        ));
    }
    Verdict::Pending
}

// -- Step 3: rollback ------------------------------------------------------------

fn rollback(
    ctx: &Context,
    handle: &SandboxHandle,
    release: &ReleaseFields,
) -> Result<(&'static str, Value), String> {
    validate_repo(&release.repo)?;
    validate_sha(&release.merge_sha)?;
    let command = rollback_command(&release.repo, &release.merge_sha);
    let result = sandbox::sandbox_exec(ctx, handle, &command, "/")?;
    if result.exit_code != 0 {
        return Err(format!(
            "release_run_lifecycle: revert of {} in {} failed (exit {}): {}",
            release.merge_sha,
            release.repo,
            result.exit_code,
            excerpt(&result.stderr)
        ));
    }
    let revert_sha = parse_revert_sha(&result.stdout)
        .ok_or_else(|| format!("release_run_lifecycle: revert pushed but no sha printed: {}", excerpt(&result.stdout)))?;
    ctx.log(
        "info",
        &format!("release_run_lifecycle: reverted {} as {revert_sha} and pushed main", release.merge_sha),
    );
    Ok(("RollbackPushed", json!({ "revert_sha": revert_sha })))
}

/// Revert the merge commit on main and push, from the computer's clone of the
/// repo (cloned on first use). The push goes through the same
/// GitHub-connected deploy path as the merge, so the platform redeploys the
/// previous build. Prints the revert commit sha on the last line.
fn rollback_command(repo: &str, merge_sha: &str) -> String {
    let dir = format!("~/workspace/{}", repo_dir_name(repo));
    format!(
        "set -e; [ -d {dir}/.git ] || git clone -q https://github.com/{repo}.git {dir}; \
         cd {dir}; git fetch -q origin main; git checkout -q main; git reset -q --hard origin/main; \
         git revert -m 1 --no-edit {merge_sha} >/dev/null; git push -q origin main; git rev-parse HEAD"
    )
}

fn parse_revert_sha(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| is_sha(l))
        .map(str::to_string)
}

// -- Helpers -----------------------------------------------------------------------

/// Read a value from the trigger params, falling back to entity state.
fn param_or_field(ctx: &Context, fields: &Value, key: &str) -> Option<String> {
    ctx.trigger_params
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            entity_field_str(fields, &[key])
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
}

/// Counters arrive as numbers or numeric strings depending on the read path.
fn counter_field(fields: &Value, key: &str) -> u64 {
    match fields.get(key) {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        _ => 0,
    }
}

/// Fetch the Computer row by id via the Temper API loopback.
fn fetch_computer(
    ctx: &Context,
    temper_api_url: &str,
    fields: &Value,
    computer_id: &str,
) -> Result<Value, String> {
    let headers = odata_headers(ctx, &ctx.tenant, fields);
    let path = format!("/tdata/Computers('{}')", bounded_reads::odata_escape(computer_id));
    bounded_reads::get_json(ctx, temper_api_url, &path, &headers, "release_run_lifecycle")
}

/// Build a SandboxHandle from a Computer row's recorded fields (same contract
/// as computer_exec: the computer must be Ready with a sandbox_url).
fn sandbox_handle_from_computer(computer: &Value) -> Result<SandboxHandle, String> {
    let status = entity_field_str(computer, &["Status", "status"]).unwrap_or("");
    if !status.is_empty() && status != "Ready" {
        return Err(format!("computer is {status}, not Ready"));
    }
    let sandbox_url = entity_field_str(computer, &["SandboxUrl", "sandbox_url"])
        .map(str::trim)
        .unwrap_or("");
    if sandbox_url.is_empty() {
        return Err("no sandbox_url recorded — provision the computer first".to_string());
    }
    let sandbox_id = entity_field_str(computer, &["MachineId", "machine_id"])
        .filter(|s| !s.is_empty())
        .or_else(|| entity_field_str(computer, &["Name", "name"]).filter(|s| !s.is_empty()))
        .unwrap_or("computer-sandbox");
    let provider = entity_field_str(computer, &["Provider", "provider"])
        .filter(|s| !s.is_empty())
        .map(normalize_sandbox_provider)
        .unwrap_or_else(|| "tensorlake".to_string());
    Ok(SandboxHandle {
        sandbox_url: sandbox_url.to_string(),
        sandbox_id: sandbox_id.to_string(),
        provider,
    })
}

/// `owner/name` only — these values are interpolated into shell commands, so
/// anything else is refused rather than quoted.
fn validate_repo(repo: &str) -> Result<(), String> {
    let mut parts = repo.split('/');
    let ok = matches!((parts.next(), parts.next(), parts.next()), (Some(o), Some(n), None)
        if !o.is_empty() && !n.is_empty() && [o, n].iter().all(|p| p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))));
    if ok {
        Ok(())
    } else {
        Err(format!("release_run_lifecycle: repo must be owner/name, got {repo:?}"))
    }
}

fn validate_number(value: &str, what: &str) -> Result<(), String> {
    if !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()) {
        Ok(())
    } else {
        Err(format!("release_run_lifecycle: {what} must be numeric, got {value:?}"))
    }
}

fn validate_sha(sha: &str) -> Result<(), String> {
    if is_sha(sha) {
        Ok(())
    } else {
        Err(format!("release_run_lifecycle: merge_sha must be a git sha, got {sha:?}"))
    }
}

fn is_sha(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn repo_dir_name(repo: &str) -> &str {
    repo.rsplit('/').next().unwrap_or(repo)
}

fn excerpt(text: &str) -> String {
    let t = text.trim();
    if t.len() <= 300 {
        t.to_string()
    } else {
        let mut end = 300;
        while !t.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &t[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "3d9284a1b2c3d4e5f60718293a4b5c6d7e8f9012";

    // -- merge ----------------------------------------------------------------

    #[test]
    fn merge_command_targets_the_pr_with_the_stored_token() {
        let cmd = merge_command("arni-labs/deep-sci-fi", "106");
        assert!(cmd.contains("https://api.github.com/repos/arni-labs/deep-sci-fi/pulls/106/merge"));
        assert!(cmd.contains("~/.git-credentials"));
        assert!(cmd.contains("\"merge_method\":\"merge\""));
        assert!(cmd.contains("-X PUT"));
    }

    #[test]
    fn merge_sha_is_read_from_a_merged_response() {
        let out = format!(r#"{{"sha":"{SHA}","merged":true,"message":"Pull Request successfully merged"}}"#);
        assert_eq!(parse_merge_sha(&out).unwrap(), SHA);
    }

    #[test]
    fn refused_merge_surfaces_githubs_message() {
        let out = r#"{"message":"Pull Request is not mergeable","documentation_url":"x"}"#;
        let err = parse_merge_sha(out).unwrap_err();
        assert!(err.contains("not mergeable"), "{err}");
    }

    #[test]
    fn non_json_merge_output_is_an_error_not_a_sha() {
        let err = parse_merge_sha("curl: (7) Failed to connect").unwrap_err();
        assert!(err.contains("unexpected GitHub response"), "{err}");
    }

    #[test]
    fn merge_refuses_shell_unsafe_repo_or_pr() {
        assert!(validate_repo("arni-labs/deep-sci-fi").is_ok());
        assert!(validate_repo("a/b; rm -rf /").is_err());
        assert!(validate_repo("no-slash").is_err());
        assert!(validate_repo("a/b/c").is_err());
        assert!(validate_number("106", "pr_number").is_ok());
        assert!(validate_number("106 || true", "pr_number").is_err());
        assert!(validate_number("", "pr_number").is_err());
    }

    // -- probe ----------------------------------------------------------------

    #[test]
    fn probe_command_curls_health_and_appends_http_status() {
        let cmd = probe_command("https://deep-sci-fi-production.up.railway.app/health");
        assert!(cmd.starts_with("curl -sS -m 15"));
        assert!(cmd.contains("__HTTP_STATUS %{http_code}"));
        assert!(cmd.ends_with("/health'"));
    }

    #[test]
    fn probe_parses_body_and_status_line() {
        let out = format!("{{\"status\":\"healthy\",\"git_sha\":\"{SHA}\"}}\n__HTTP_STATUS 200");
        let p = parse_probe(&out);
        assert_eq!(p.http_status, 200);
        assert_eq!(p.status.as_deref(), Some("healthy"));
        assert_eq!(p.git_sha.as_deref(), Some(SHA));
    }

    #[test]
    fn probe_without_json_body_still_yields_http_status() {
        let p = parse_probe("<html>502 Bad Gateway</html>\n__HTTP_STATUS 502");
        assert_eq!(p.http_status, 502);
        assert_eq!(p, Probe { http_status: 502, status: None, git_sha: None });
    }

    #[test]
    fn probe_with_no_marker_is_status_zero() {
        assert_eq!(parse_probe("curl: (28) timed out").http_status, 0);
    }

    fn probe(http: u16, status: Option<&str>, sha: Option<&str>) -> Probe {
        Probe { http_status: http, status: status.map(String::from), git_sha: sha.map(String::from) }
    }

    #[test]
    fn healthy_when_new_commit_serves_healthy() {
        assert_eq!(evaluate_probe(&probe(200, Some("healthy"), Some(SHA)), SHA, 1, 20), Verdict::Healthy);
    }

    #[test]
    fn pending_while_old_commit_still_serves() {
        assert_eq!(evaluate_probe(&probe(200, Some("healthy"), Some("oldsha00")), SHA, 1, 20), Verdict::Pending);
    }

    #[test]
    fn pending_when_health_has_no_sha_yet() {
        // e.g. the previous build predates the git_sha field.
        assert_eq!(evaluate_probe(&probe(200, Some("healthy"), None), SHA, 3, 20), Verdict::Pending);
    }

    #[test]
    fn pending_on_transient_502_during_the_swap() {
        assert_eq!(evaluate_probe(&probe(502, None, None), SHA, 2, 20), Verdict::Pending);
    }

    #[test]
    fn unhealthy_when_new_commit_serves_degraded() {
        match evaluate_probe(&probe(200, Some("degraded"), Some(SHA)), SHA, 1, 20) {
            Verdict::Unhealthy(reason) => assert!(reason.contains("degraded"), "{reason}"),
            other => panic!("expected Unhealthy, got {other:?}"),
        }
    }

    #[test]
    fn unhealthy_once_the_check_budget_is_spent() {
        match evaluate_probe(&probe(200, Some("healthy"), Some("oldsha00")), SHA, 20, 20) {
            Verdict::Unhealthy(reason) => {
                assert!(reason.contains("after 20 checks"), "{reason}");
                assert!(reason.contains("oldsha00"), "{reason}");
            }
            other => panic!("expected Unhealthy, got {other:?}"),
        }
    }

    #[test]
    fn budget_is_not_spent_one_check_early() {
        assert_eq!(evaluate_probe(&probe(200, Some("healthy"), Some("oldsha00")), SHA, 19, 20), Verdict::Pending);
    }

    // -- rollback -------------------------------------------------------------

    #[test]
    fn rollback_reverts_the_merge_commit_and_pushes_main() {
        let cmd = rollback_command("arni-labs/deep-sci-fi", SHA);
        assert!(cmd.starts_with("set -e;"));
        assert!(cmd.contains("git clone -q https://github.com/arni-labs/deep-sci-fi.git ~/workspace/deep-sci-fi"));
        assert!(cmd.contains("git reset -q --hard origin/main"));
        assert!(cmd.contains(&format!("git revert -m 1 --no-edit {SHA}")));
        assert!(cmd.contains("git push -q origin main"));
        assert!(cmd.trim_end().ends_with("git rev-parse HEAD"));
    }

    #[test]
    fn revert_sha_is_the_last_sha_line() {
        let out = format!("Switched to branch 'main'\n{SHA}\n");
        assert_eq!(parse_revert_sha(&out).as_deref(), Some(SHA));
        assert_eq!(parse_revert_sha("nothing here"), None);
    }

    #[test]
    fn rollback_refuses_a_non_sha() {
        assert!(validate_sha(SHA).is_ok());
        assert!(validate_sha("main; rm -rf ~").is_err());
        assert!(validate_sha("").is_err());
    }

    // -- helpers --------------------------------------------------------------

    #[test]
    fn counter_reads_number_or_string() {
        assert_eq!(counter_field(&json!({"check_count": 3}), "check_count"), 3);
        assert_eq!(counter_field(&json!({"check_count": "7"}), "check_count"), 7);
        assert_eq!(counter_field(&json!({}), "check_count"), 0);
    }

    #[test]
    fn handle_requires_a_ready_computer_with_sandbox_url() {
        let ready = json!({"Status":"Ready","fields":{"sandbox_url":"https://s.example","machine_id":"m1","provider":"tl"}});
        let h = sandbox_handle_from_computer(&ready).unwrap();
        assert_eq!(h.sandbox_url, "https://s.example");
        assert_eq!(h.sandbox_id, "m1");
        assert_eq!(h.provider, "tensorlake");
        let sleeping = json!({"Status":"Sleeping","fields":{"sandbox_url":"https://s.example"}});
        assert!(sandbox_handle_from_computer(&sleeping).err().unwrap().contains("Sleeping"));
        let bare = json!({"Status":"Ready","fields":{"sandbox_url":""}});
        assert!(sandbox_handle_from_computer(&bare).err().unwrap().contains("no sandbox_url"));
    }
}
