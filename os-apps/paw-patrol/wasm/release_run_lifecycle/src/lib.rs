//! release_run_lifecycle — side effects for the ReleaseRun automaton, run on
//! the named Computer's sandbox.
//!
//! One trigger, one side effect, one named callback. The module never
//! dispatches actions on other entities and never loops; the automaton and
//! the kernel `state_timeout` own the orchestration (see release_run.ioa.toml).
//!
//! | trigger action | side effect on the computer                              | reports |
//! |----------------|----------------------------------------------------------|---------|
//! | Request        | preflight the PR (base==main), merge via GitHub API      | MergeSucceeded |
//! | Check          | one `curl` of health_url                                 | CheckHealthy / CheckPending / CheckUnhealthy |
//! | CheckUnhealthy | `git revert -m 1 <merge_sha>` + push to the base branch  | RollbackPushed |
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

/// Hard ceiling on the watch budget: 240 checks × 30s = 2 hours. Bounds the
/// `max_checks` config so a caller cannot request an effectively permanent
/// watch that defeats automatic rollback.
const MAX_CHECKS_CEILING: u64 = 240;

/// Consecutive degraded-on-new-commit probes required before rollback. A
/// single degraded 30s window right after a swap (cache warmup) must not
/// revert a healthy release; three in a row is a real regression.
const DEGRADED_STREAK_THRESHOLD: u64 = 3;

/// The only base branch a release may target. Enforced at merge time so the
/// rollback (which reverts on this branch) can never push to the wrong one.
const RELEASE_BASE_BRANCH: &str = "main";

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
            "Request" => merge(&ctx, &handle, &release, &temper_api_url, &fields)?,
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
    degraded_streak: u64,
    /// Optional reviewed PR head to bind the merge to (empty = unbound).
    expected_head_sha: String,
}

impl ReleaseFields {
    fn from_state(ctx: &Context, fields: &Value) -> Result<Self, String> {
        let required = |key: &str| {
            param_or_field(ctx, fields, key)
                .ok_or_else(|| format!("release_run_lifecycle: missing {key}"))
        };
        let max_checks_raw = param_or_field(ctx, fields, "max_checks")
            .ok_or_else(|| "release_run_lifecycle: missing max_checks".to_string())?;
        Ok(Self {
            repo: required("repo")?,
            pr_number: required("pr_number")?,
            computer_id: required("computer_id")?,
            health_url: param_or_field(ctx, fields, "health_url").unwrap_or_default(),
            max_checks: parse_max_checks(&max_checks_raw)?,
            merge_sha: param_or_field(ctx, fields, "merge_sha").unwrap_or_default(),
            check_count: counter_field(fields, "check_count"),
            degraded_streak: counter_field(fields, "degraded_streak"),
            expected_head_sha: param_or_field(ctx, fields, "expected_head_sha").unwrap_or_default(),
        })
    }
}

/// Parse and bound the watch budget. Rejects non-numeric input and 0 (which
/// would make the first check exceed budget → instant rollback) and clamps to
/// [`MAX_CHECKS_CEILING`] so a huge value cannot create a permanent watch.
fn parse_max_checks(raw: &str) -> Result<u64, String> {
    let n: u64 = raw
        .trim()
        .parse()
        .map_err(|_| format!("release_run_lifecycle: max_checks must be a number, got {raw:?}"))?;
    if n == 0 {
        return Err("release_run_lifecycle: max_checks must be at least 1".to_string());
    }
    if n > MAX_CHECKS_CEILING {
        return Err(format!(
            "release_run_lifecycle: max_checks {n} exceeds the ceiling {MAX_CHECKS_CEILING} (2h)"
        ));
    }
    Ok(n)
}

// -- Step 1: merge -------------------------------------------------------------

fn merge(
    ctx: &Context,
    handle: &SandboxHandle,
    release: &ReleaseFields,
    temper_api_url: &str,
    fields: &Value,
) -> Result<(&'static str, Value), String> {
    validate_repo(&release.repo)?;
    validate_number(&release.pr_number, "pr_number")?;
    // Validate the watch target BEFORE merging: a bad health_url must abort
    // the release rather than leave a merged-but-unwatchable rollout.
    validate_url(&release.health_url)?;

    // ARN-397: per-repo serialization. Refuse to start a merge while another
    // ReleaseRun for the same repo is already active — two concurrent releases
    // on one repo interleave merges/reverts and corrupt each other's watch.
    // (This is a read + our own Fail; no cross-entity transition dispatch.)
    if let Some(other) = active_release_conflict(ctx, temper_api_url, fields, &release.repo)? {
        return Err(format!(
            "release_run_lifecycle: a release for {} is already in flight ({other}); refusing to run a second concurrently (ARN-397)",
            release.repo
        ));
    }

    // Preflight: read the PR's base branch, head commit, and merge state.
    let pr = read_pr(ctx, handle, &release.repo, &release.pr_number)?;
    if pr.base_ref != RELEASE_BASE_BRANCH {
        return Err(format!(
            "release_run_lifecycle: PR {}#{} targets base {:?}, only {RELEASE_BASE_BRANCH} is releasable",
            release.repo, release.pr_number, pr.base_ref
        ));
    }
    // Commit-binding: if bound to a reviewed head, refuse unless it still matches.
    if !head_binding_ok(&release.expected_head_sha, &pr.head_sha) {
        return Err(format!(
            "release_run_lifecycle: PR {}#{} head is {:?}, expected reviewed head {:?} — refusing to merge an unreviewed commit",
            release.repo, release.pr_number, pr.head_sha, release.expected_head_sha
        ));
    }
    if pr.merged {
        let sha = pr.merge_commit_sha.clone().ok_or_else(|| {
            format!("release_run_lifecycle: PR {}#{} reports merged but carries no sha", release.repo, release.pr_number)
        })?;
        ctx.log("info", &format!("release_run_lifecycle: {}#{} already merged as {sha}", release.repo, release.pr_number));
        return Ok(merge_succeeded(sha, pr.base_ref, pr.head_sha));
    }

    // Do the merge, pinning the head sha we just read: GitHub refuses (405/409)
    // if the head moved since, closing the read→PUT TOCTOU (commit-binding).
    let command = merge_command(&release.repo, &release.pr_number, &pr.head_sha);
    let result = sandbox::sandbox_exec(ctx, handle, &command, "/")?;
    match parse_merge_sha(&result.stdout) {
        Ok(sha) => {
            ctx.log("info", &format!("release_run_lifecycle: merged {}#{} as {sha}", release.repo, release.pr_number));
            Ok(merge_succeeded(sha, pr.base_ref, pr.head_sha))
        }
        Err(merge_err) => {
            // Ambiguous outcome: the PUT may have merged before the connection
            // dropped. Reconcile by re-reading the PR; only fail if it is still
            // unmerged, so we never lose the watcher on an actually-merged PR.
            match read_pr(ctx, handle, &release.repo, &release.pr_number) {
                Ok(recheck) if recheck.merged => {
                    let sha = recheck.merge_commit_sha.unwrap_or_default();
                    if sha.is_empty() {
                        return Err(format!(
                            "release_run_lifecycle: {}#{} merged on reconcile but carried no sha",
                            release.repo, release.pr_number
                        ));
                    }
                    ctx.log("info", &format!("release_run_lifecycle: {}#{} merged (confirmed on reconcile) as {sha}", release.repo, release.pr_number));
                    Ok(merge_succeeded(sha, recheck.base_ref, recheck.head_sha))
                }
                _ => Err(format!(
                    "release_run_lifecycle: merge of {}#{} did not complete: {merge_err} (exit {}, stderr: {})",
                    release.repo, release.pr_number, result.exit_code, excerpt(&result.stderr)
                )),
            }
        }
    }
}

/// Commit-binding predicate: an empty expected head is unbound (any head ok);
/// a set expected head must equal the PR's current head, else the merge is
/// refused (an unreviewed commit was pushed after the target was bound).
fn head_binding_ok(expected: &str, actual: &str) -> bool {
    expected.is_empty() || expected == actual
}

/// Build the MergeSucceeded callback (merge_sha to watch, base_branch for the
/// rollback, head_sha for audit).
fn merge_succeeded(merge_sha: String, base_branch: String, head_sha: String) -> (&'static str, Value) {
    (
        "MergeSucceeded",
        json!({ "merge_sha": merge_sha, "base_branch": base_branch, "head_sha": head_sha }),
    )
}

/// What we need to know about a PR before/after merging.
#[derive(Debug, Clone, PartialEq)]
struct PrInfo {
    base_ref: String,
    /// The PR's current head commit sha (what a merge would land).
    head_sha: String,
    merged: bool,
    merge_commit_sha: Option<String>,
}

/// GET the PR and parse base branch + merge state.
fn read_pr(
    ctx: &Context,
    handle: &SandboxHandle,
    repo: &str,
    pr_number: &str,
) -> Result<PrInfo, String> {
    let command = pr_get_command(repo, pr_number);
    let result = sandbox::sandbox_exec(ctx, handle, &command, "/")?;
    parse_pr(&result.stdout).map_err(|e| {
        format!(
            "release_run_lifecycle: could not read PR {repo}#{pr_number}: {e} (exit {}, stderr: {})",
            result.exit_code,
            excerpt(&result.stderr)
        )
    })
}

fn parse_pr(stdout: &str) -> Result<PrInfo, String> {
    let body: Value = serde_json::from_str(stdout.trim())
        .map_err(|_| format!("unexpected GitHub response: {}", excerpt(stdout)))?;
    if let Some(msg) = body.get("message").and_then(Value::as_str) {
        // GitHub returns {"message": "..."} on error (404, bad creds, rate limit).
        if body.get("base").is_none() {
            return Err(format!("GitHub error: {msg}"));
        }
    }
    let base_ref = body
        .get("base")
        .and_then(|b| b.get("ref"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "PR response carried no base.ref".to_string())?;
    let head_sha = body
        .get("head")
        .and_then(|h| h.get("sha"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Ok(PrInfo {
        base_ref,
        head_sha,
        merged: body.get("merged").and_then(Value::as_bool).unwrap_or(false),
        merge_commit_sha: body
            .get("merge_commit_sha")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    })
}

/// The GitHub token, read as the password field of the first `github.com`
/// credential line in `~/.git-credentials`. The match is host-scoped to
/// `github.com` (so it skips credential lines for other hosts), which is the
/// reliable path on the sandbox — `git credential fill` depends on a
/// configured `credential.helper` and hangs/returns empty when one is not set.
/// Assigned into `$TOK`.
const TOKEN_PRELUDE: &str =
    "TOK=$(sed -nE 's#https://[^:]+:([^@]+)@github\\.com.*#\\1#p' ~/.git-credentials | head -1); ";

/// GET the PR as JSON (base branch, merge state).
fn pr_get_command(repo: &str, pr_number: &str) -> String {
    format!(
        "{TOKEN_PRELUDE}\
         curl -sS -m 30 \"https://api.github.com/repos/{repo}/pulls/{pr_number}\" \
         -H \"Authorization: token $TOK\" -H \"Accept: application/vnd.github+json\""
    )
}

/// Merge the PR through the GitHub API from the computer, using the token that
/// matches github.com. The `sha` field pins the expected PR head — GitHub
/// refuses (405/409 "Head branch was modified") if the head moved since we read
/// it, closing the read→PUT TOCTOU (commit-binding, ARN-394). `head_sha` is a
/// validated 40-hex sha, safe to interpolate. Prints the API response so the
/// merge sha can be read.
fn merge_command(repo: &str, pr_number: &str, head_sha: &str) -> String {
    format!(
        "{TOKEN_PRELUDE}\
         curl -sS -m 60 -X PUT \"https://api.github.com/repos/{repo}/pulls/{pr_number}/merge\" \
         -H \"Authorization: token $TOK\" -H \"Accept: application/vnd.github+json\" \
         -d '{{\"sha\":\"{head_sha}\",\"merge_method\":\"merge\"}}'"
    )
}

/// Non-terminal ReleaseRun states — a run in any of these is "in flight" for
/// per-repo serialization (ARN-397).
const ACTIVE_RELEASE_STATES: &[&str] = &["Requested", "Merging", "Watching", "Unhealthy"];

/// Query other ReleaseRuns for `repo` and return the id of one that is still
/// active (excluding self), if any. A loopback read; failure surfaces as an
/// error so the check fails closed (we would rather refuse a release than run a
/// second concurrently on an unverifiable state).
///
/// TOCTOU residual: two Requests can both pass this read before either commits
/// its Merging state. A fully-atomic guard is a per-repo lane entity — tracked
/// as the stronger form of ARN-397; this read-reject closes the common case.
fn active_release_conflict(
    ctx: &Context,
    temper_api_url: &str,
    fields: &Value,
    repo: &str,
) -> Result<Option<String>, String> {
    let headers = odata_headers(ctx, &ctx.tenant, fields);
    let path = format!(
        "/tdata/ReleaseRuns?$filter=repo eq '{}'",
        bounded_reads::odata_escape(repo)
    );
    let resp = bounded_reads::get_json(ctx, temper_api_url, &path, &headers, "release_run_lifecycle")
        .map_err(|e| format!("release_run_lifecycle: could not check for concurrent releases of {repo}: {e}"))?;
    let runs = parse_release_runs(&resp);
    Ok(conflicting_active_release(&runs, &ctx.entity_id, repo).map(str::to_string))
}

/// Extract (id, repo, status) from an OData ReleaseRuns list response.
fn parse_release_runs(resp: &Value) -> Vec<(String, String, String)> {
    resp.get("value")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    let id = entity_field_str(e, &["id", "Id"])?.to_string();
                    let repo = entity_field_str(e, &["repo", "Repo"]).unwrap_or("").to_string();
                    let status = entity_field_str(e, &["Status", "status"]).unwrap_or("").to_string();
                    Some((id, repo, status))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Pure: given other runs and this run's id+repo, return a conflicting active
/// release id for the same repo (excluding self), if any.
fn conflicting_active_release<'a>(
    runs: &'a [(String, String, String)],
    self_id: &str,
    repo: &str,
) -> Option<&'a str> {
    runs.iter()
        .find(|(id, r, status)| {
            id != self_id && r == repo && ACTIVE_RELEASE_STATES.contains(&status.as_str())
        })
        .map(|(id, _, _)| id.as_str())
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
    validate_url(&release.health_url)?;
    if release.merge_sha.is_empty() {
        return Err("release_run_lifecycle: no merge_sha to watch for".to_string());
    }
    let command = probe_command(&release.health_url);
    let result = sandbox::sandbox_exec(ctx, handle, &command, "/")?;
    let probe = parse_probe(&result.stdout);
    let verdict = evaluate_probe(
        &probe,
        &release.merge_sha,
        release.check_count,
        release.max_checks,
        release.degraded_streak,
    );
    ctx.log(
        "info",
        &format!(
            "release_run_lifecycle: check {}/{} streak {} -> http {} status {:?} sha {:?}: {:?}",
            release.check_count,
            release.max_checks,
            release.degraded_streak,
            probe.http_status,
            probe.status,
            probe.git_sha,
            verdict
        ),
    );
    let observed = probe.git_sha.clone().unwrap_or_default();
    Ok(match verdict {
        Verdict::Healthy => ("CheckHealthy", json!({ "observed_sha": observed })),
        Verdict::Pending { degraded_streak } => (
            "CheckPending",
            // Emit the streak as a JSON NUMBER: the spec's `set_counter_from_param`
            // effect (kernel `SetCounterFromParam`) only accepts numbers and
            // silently drops a string, which would pin the streak at 0 and make
            // the 3-strike rollback never fire (ARN-394 re-review).
            json!({ "observed_sha": observed, "degraded_streak": degraded_streak }),
        ),
        Verdict::Unhealthy(reason) => (
            "CheckUnhealthy",
            json!({ "reason": reason, "observed_sha": observed }),
        ),
    })
}

/// `curl` the health endpoint from the computer; the HTTP status is appended
/// on its own marker line so a body that is not JSON still yields a verdict.
/// `health_url` is single-quoted AND validated by [`validate_url`] first.
fn probe_command(health_url: &str) -> String {
    // `-g` (globoff): a `[` or `]` in the URL must be a literal, not a curl glob
    // range — otherwise curl multi-fetches or exits 3 and every probe reads as
    // status 0 (Pending), burning the whole budget before a real rollback
    // (ARN-394 re-review).
    format!("curl -g -sS -m 15 -w '\\n{HTTP_STATUS_MARKER} %{{http_code}}' '{health_url}'")
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
    /// Keep watching; carries the degraded-streak to persist for the next check
    /// (0 unless the new commit is serving degraded).
    Pending { degraded_streak: u64 },
    Unhealthy(String),
}

/// Decide the outcome of one probe.
///
/// - Healthy: HTTP 2xx, `status == healthy`, and the served `git_sha` is the
///   merge commit — the rollout landed and is serving cleanly.
/// - Unhealthy: the new commit is being served but reports non-2xx / not-healthy
///   for [`DEGRADED_STREAK_THRESHOLD`] consecutive probes (a real regression,
///   not a one-window warmup blip), OR the probe budget is spent without the
///   new commit becoming healthy. Both trigger the rollback.
/// - Pending: anything else while budget remains — old build still serving,
///   deploy in progress, transient non-2xx, or a not-yet-confirmed degraded
///   streak on the new commit (streak carried forward).
fn evaluate_probe(
    probe: &Probe,
    merge_sha: &str,
    check_count: u64,
    max_checks: u64,
    degraded_streak: u64,
) -> Verdict {
    let serving_new = probe.git_sha.as_deref() == Some(merge_sha);
    let ok = (200..300).contains(&probe.http_status);
    let healthy = probe.status.as_deref() == Some("healthy");

    if serving_new && ok && healthy {
        return Verdict::Healthy;
    }

    // The new commit is serving but not cleanly (non-2xx or status != healthy).
    // Require a consecutive streak before reverting, so one warmup window does
    // not roll back a healthy release.
    if serving_new {
        let streak = degraded_streak + 1;
        if streak >= DEGRADED_STREAK_THRESHOLD {
            return Verdict::Unhealthy(format!(
                "new commit {merge_sha} served degraded for {streak} consecutive checks (last: http {}, status {})",
                probe.http_status,
                probe.status.as_deref().unwrap_or("unknown")
            ));
        }
        return Verdict::Pending { degraded_streak: streak };
    }

    // Old commit still serving (or health has no sha yet). Reset the streak.
    if check_count >= max_checks {
        return Verdict::Unhealthy(format!(
            "rollout of {merge_sha} not healthy after {check_count} checks (last: http {}, status {}, sha {})",
            probe.http_status,
            probe.status.as_deref().unwrap_or("none"),
            probe.git_sha.as_deref().unwrap_or("none")
        ));
    }
    Verdict::Pending { degraded_streak: 0 }
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
        &format!("release_run_lifecycle: reverted {} as {revert_sha} and pushed {RELEASE_BASE_BRANCH}", release.merge_sha),
    );
    Ok(("RollbackPushed", json!({ "revert_sha": revert_sha })))
}

/// Revert the merge commit on the base branch and push, from a FRESH, ambient-
/// config-free checkout. Hardening (ARN-394 re-review P0-4 — the deterministic
/// reused dir was code-execution-capable shared state):
/// - `mktemp -d` per attempt (never reuses a checkout a prior sandbox user
///   could have poisoned), removed on exit via `trap`;
/// - `GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null` neutralize any
///   attacker-planted global/system git config (credential.helper, url
///   rewrites, `core.hooksPath`, `gpg.program`); `-c core.hooksPath=/dev/null
///   -c commit.gpgsign=false` disable repo hooks and signing on clone/revert;
/// - because the ambient credential helper is now off, the token is supplied
///   explicitly in the clone/push URL (`$TOK` is a runtime var, not a literal
///   in the stored command);
/// - `GIT_TERMINAL_PROMPT=0` and a `timeout` around every network git call;
/// - a configured commit identity so `git revert` (which commits) never stalls;
/// - idempotent at the TIP only: re-print HEAD if HEAD already reverts this
///   merge, else revert and push.
/// Prints the resulting head sha on the last line. `repo`/`merge_sha` are
/// pre-validated (owner/name, 40-hex) so they are safe to interpolate.
fn rollback_command(repo: &str, merge_sha: &str) -> String {
    let b = RELEASE_BASE_BRANCH;
    format!(
        "{TOKEN_PRELUDE}\
         set -e; export GIT_TERMINAL_PROMPT=0 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null; \
         URL=\"https://x-access-token:$TOK@github.com/{repo}.git\"; \
         DIR=$(mktemp -d); trap 'rm -rf \"$DIR\"' EXIT; \
         timeout 120 git -c core.hooksPath=/dev/null clone -q --branch {b} \"$URL\" \"$DIR\"; \
         cd \"$DIR\"; \
         git config core.hooksPath /dev/null; git config commit.gpgsign false; \
         git config user.name 'temperpaw-release'; git config user.email 'release@temperpaw.local'; \
         if git log -1 --format=%B HEAD | grep -qF \"This reverts commit {merge_sha}\"; then \
           git rev-parse HEAD; \
         else \
           git -c core.hooksPath=/dev/null revert -m 1 --no-edit {merge_sha} >/dev/null; \
           timeout 60 git push -q \"$URL\" HEAD:{b}; git rev-parse HEAD; \
         fi"
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

/// Build a SandboxHandle from a Computer row's recorded fields. The computer
/// must be explicitly `Ready` with a sandbox_url — an empty/absent status is
/// NOT treated as ready (no fail-open on the gate that decides whether to run
/// commands on a sandbox).
fn sandbox_handle_from_computer(computer: &Value) -> Result<SandboxHandle, String> {
    let status = entity_field_str(computer, &["Status", "status"]).unwrap_or("");
    if status != "Ready" {
        return Err(format!("computer status is {status:?}, must be \"Ready\""));
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

/// `owner/name` only — interpolated into shell commands, so anything else is
/// refused rather than quoted. Each part is a GitHub-legal segment: ASCII
/// alphanumerics plus `-_.`, not equal to `.`/`..`, and not starting with `-`
/// or `.` (which would let `owner/..` climb out of the workspace dir or a
/// leading `-` be read as a flag).
fn validate_repo(repo: &str) -> Result<(), String> {
    fn part_ok(p: &str) -> bool {
        !p.is_empty()
            && p != "."
            && p != ".."
            && !p.starts_with('-')
            && !p.starts_with('.')
            && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    }
    let mut parts = repo.split('/');
    let ok = matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(o), Some(n), None) if part_ok(o) && part_ok(n)
    );
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

/// Strictly validate the health URL before it is spliced into a shell command.
/// Must be `https://…` and contain only characters that are safe inside a
/// single-quoted shell string AND legal in a URL — no quotes, backslash,
/// whitespace, or shell metacharacters. The single-quoting in
/// [`probe_command`] is defense-in-depth; this allowlist is the real guard.
fn validate_url(url: &str) -> Result<(), String> {
    const DANGEROUS: &[char] = &[
        '\'', '"', '`', '\\', ';', '$', '<', '>', '|', '(', ')', '{', '}', '*', '!', ' ', '\t',
        '\n', '\r',
    ];
    let allowed = |c: char| {
        c.is_ascii_alphanumeric()
            || matches!(
                c,
                '-' | '.' | '_' | '~' | ':' | '/' | '?' | '#' | '[' | ']' | '@' | '&' | '+' | ',' | '=' | '%'
            )
    };
    if !url.starts_with("https://") {
        return Err(format!("release_run_lifecycle: health_url must be https://, got {url:?}"));
    }
    if url.len() > 2048 {
        return Err("release_run_lifecycle: health_url is too long".to_string());
    }
    if let Some(bad) = url.chars().find(|c| DANGEROUS.contains(c) || !allowed(*c)) {
        return Err(format!(
            "release_run_lifecycle: health_url contains a disallowed character {bad:?}"
        ));
    }
    Ok(())
}

fn is_sha(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit())
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

    // -- max_checks bounds ----------------------------------------------------

    #[test]
    fn max_checks_rejects_non_numeric_zero_and_huge() {
        assert_eq!(parse_max_checks("60").unwrap(), 60);
        assert_eq!(parse_max_checks(" 2 ").unwrap(), 2);
        assert!(parse_max_checks("banana").is_err());
        assert!(parse_max_checks("0").is_err());
        assert!(parse_max_checks("18446744073709551615").is_err());
        assert!(parse_max_checks(&(MAX_CHECKS_CEILING + 1).to_string()).is_err());
        assert_eq!(parse_max_checks(&MAX_CHECKS_CEILING.to_string()).unwrap(), MAX_CHECKS_CEILING);
    }

    // -- PR preflight / reconcile ---------------------------------------------

    #[test]
    fn pr_get_command_reads_the_pr_with_the_matched_token() {
        let cmd = pr_get_command("arni-labs/deep-sci-fi", "109");
        assert!(cmd.contains("~/.git-credentials"));
        assert!(cmd.contains("@github"));
        assert!(cmd.contains("https://api.github.com/repos/arni-labs/deep-sci-fi/pulls/109"));
        assert!(!cmd.contains("/merge"));
    }

    #[test]
    fn parse_pr_reads_base_head_and_merge_state() {
        let out = format!(
            r#"{{"base":{{"ref":"main"}},"head":{{"sha":"{SHA}"}},"merged":false,"merge_commit_sha":"{SHA}"}}"#
        );
        let pr = parse_pr(&out).unwrap();
        assert_eq!(pr.base_ref, "main");
        assert_eq!(pr.head_sha, SHA);
        assert!(!pr.merged);
        assert_eq!(pr.merge_commit_sha.as_deref(), Some(SHA));
    }

    #[test]
    fn head_binding_allows_unbound_and_matching_only() {
        assert!(head_binding_ok("", "anything")); // unbound
        assert!(head_binding_ok(SHA, SHA)); // matches
        assert!(!head_binding_ok(SHA, "0000000")); // reviewed head moved
    }

    #[test]
    fn parse_pr_surfaces_github_error() {
        let err = parse_pr(r#"{"message":"Not Found"}"#).unwrap_err();
        assert!(err.contains("Not Found"), "{err}");
    }

    #[test]
    fn parse_pr_rejects_non_json() {
        assert!(parse_pr("curl: (7) failed").is_err());
    }

    // -- merge ----------------------------------------------------------------

    #[test]
    fn merge_command_targets_the_pr_and_pins_the_head_sha() {
        let cmd = merge_command("arni-labs/deep-sci-fi", "106", SHA);
        assert!(cmd.contains("https://api.github.com/repos/arni-labs/deep-sci-fi/pulls/106/merge"));
        assert!(cmd.contains("~/.git-credentials"));
        assert!(cmd.contains("\"merge_method\":\"merge\""));
        assert!(cmd.contains("-X PUT"));
        // Pins the reviewed head so GitHub refuses if the head moved (TOCTOU).
        assert!(cmd.contains(&format!("\"sha\":\"{SHA}\"")), "{cmd}");
    }

    #[test]
    fn per_repo_reject_finds_a_concurrent_active_release() {
        let runs = vec![
            ("self".into(), "a/b".into(), "Merging".into()),
            ("other-done".into(), "a/b".into(), "Healthy".into()), // terminal, ignore
            ("other-active".into(), "a/b".into(), "Watching".into()), // conflict
            ("elsewhere".into(), "c/d".into(), "Merging".into()),   // other repo
        ];
        assert_eq!(conflicting_active_release(&runs, "self", "a/b"), Some("other-active"));
        // No conflict when the only same-repo runs are self or terminal.
        let clean = vec![
            ("self".into(), "a/b".into(), "Merging".into()),
            ("done".into(), "a/b".into(), "RolledBack".into()),
        ];
        assert_eq!(conflicting_active_release(&clean, "self", "a/b"), None);
    }

    #[test]
    fn parse_release_runs_reads_id_repo_status() {
        let resp = json!({"value":[
            {"id":"r1","fields":{"repo":"a/b"},"Status":"Watching"},
            {"Id":"r2","repo":"a/b","status":"Failed"}
        ]});
        let runs = parse_release_runs(&resp);
        assert_eq!(runs.len(), 2);
        assert!(runs.iter().any(|(i, r, s)| i == "r1" && r == "a/b" && s == "Watching"));
        assert!(runs.iter().any(|(i, _, s)| i == "r2" && s == "Failed"));
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
    fn validators_refuse_shell_unsafe_inputs() {
        assert!(validate_repo("arni-labs/deep-sci-fi").is_ok());
        assert!(validate_repo("a/b; rm -rf /").is_err());
        assert!(validate_repo("no-slash").is_err());
        assert!(validate_repo("a/b/c").is_err());
        // `.`/`..`/leading-dot/leading-dash parts are refused (dir-climb, flag).
        assert!(validate_repo("owner/..").is_err());
        assert!(validate_repo("../etc").is_err());
        assert!(validate_repo("owner/.hidden").is_err());
        assert!(validate_repo("-owner/name").is_err());
        assert!(validate_number("106", "pr_number").is_ok());
        assert!(validate_number("106 || true", "pr_number").is_err());
        assert!(validate_number("", "pr_number").is_err());
    }

    // -- health_url validation (P0: shell injection) --------------------------

    #[test]
    fn validate_url_accepts_a_normal_https_health_url() {
        assert!(validate_url("https://deep-sci-fi-production.up.railway.app/health").is_ok());
        assert!(validate_url("https://x.example/health?full=1&k=v#frag").is_ok());
    }

    #[test]
    fn validate_url_rejects_injection_and_non_https() {
        // The exact exploit shape from the review.
        assert!(validate_url("https://x/'; cat ~/.git-credentials; '").is_err());
        assert!(validate_url("https://x/$(whoami)").is_err());
        assert!(validate_url("https://x/`id`").is_err());
        assert!(validate_url("https://x/a|b").is_err());
        assert!(validate_url("https://x/a b").is_err());
        assert!(validate_url("http://x/health").is_err());
        assert!(validate_url("").is_err());
        assert!(validate_url("https://x/\"q\"").is_err());
    }

    // -- probe ----------------------------------------------------------------

    #[test]
    fn probe_command_curls_health_and_appends_http_status() {
        let cmd = probe_command("https://deep-sci-fi-production.up.railway.app/health");
        assert!(cmd.starts_with("curl -g -sS -m 15"), "{cmd}");
        // -g/globoff so a `[` in the URL is literal, not a curl glob range.
        assert!(cmd.contains("curl -g "), "{cmd}");
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

    fn probe(http: u16, status: Option<&str>, sha: Option<&str>) -> Probe {
        Probe { http_status: http, status: status.map(String::from), git_sha: sha.map(String::from) }
    }

    #[test]
    fn healthy_when_new_commit_serves_healthy() {
        assert_eq!(evaluate_probe(&probe(200, Some("healthy"), Some(SHA)), SHA, 1, 20, 0), Verdict::Healthy);
    }

    #[test]
    fn pending_while_old_commit_still_serves_resets_streak() {
        assert_eq!(
            evaluate_probe(&probe(200, Some("healthy"), Some("oldsha00")), SHA, 1, 20, 2),
            Verdict::Pending { degraded_streak: 0 }
        );
    }

    #[test]
    fn pending_when_health_has_no_sha_yet() {
        assert_eq!(
            evaluate_probe(&probe(200, Some("healthy"), None), SHA, 3, 20, 0),
            Verdict::Pending { degraded_streak: 0 }
        );
    }

    #[test]
    fn pending_on_transient_502_while_old_commit_serves() {
        assert_eq!(
            evaluate_probe(&probe(502, None, None), SHA, 2, 20, 0),
            Verdict::Pending { degraded_streak: 0 }
        );
    }

    #[test]
    fn single_degraded_probe_on_new_commit_does_not_roll_back() {
        // First degraded window on the new sha: streak -> 1, keep watching.
        assert_eq!(
            evaluate_probe(&probe(200, Some("degraded"), Some(SHA)), SHA, 3, 20, 0),
            Verdict::Pending { degraded_streak: 1 }
        );
        // Second: streak -> 2, still watching.
        assert_eq!(
            evaluate_probe(&probe(503, None, Some(SHA)), SHA, 4, 20, 1),
            Verdict::Pending { degraded_streak: 2 }
        );
    }

    #[test]
    fn consecutive_degraded_streak_rolls_back() {
        match evaluate_probe(&probe(200, Some("degraded"), Some(SHA)), SHA, 5, 20, 2) {
            Verdict::Unhealthy(reason) => {
                assert!(reason.contains("3 consecutive"), "{reason}");
                assert!(reason.contains("degraded"), "{reason}");
            }
            other => panic!("expected Unhealthy, got {other:?}"),
        }
    }

    #[test]
    fn matching_sha_with_http_500_counts_toward_the_streak() {
        // Serving the new sha but HTTP 500 is degraded, not pending.
        assert_eq!(
            evaluate_probe(&probe(500, Some("healthy"), Some(SHA)), SHA, 5, 20, 0),
            Verdict::Pending { degraded_streak: 1 }
        );
    }

    #[test]
    fn unhealthy_once_the_check_budget_is_spent() {
        match evaluate_probe(&probe(200, Some("healthy"), Some("oldsha00")), SHA, 20, 20, 0) {
            Verdict::Unhealthy(reason) => {
                assert!(reason.contains("after 20 checks"), "{reason}");
                assert!(reason.contains("oldsha00"), "{reason}");
            }
            other => panic!("expected Unhealthy, got {other:?}"),
        }
    }

    #[test]
    fn budget_is_not_spent_one_check_early() {
        assert_eq!(
            evaluate_probe(&probe(200, Some("healthy"), Some("oldsha00")), SHA, 19, 20, 0),
            Verdict::Pending { degraded_streak: 0 }
        );
    }

    // -- rollback -------------------------------------------------------------

    #[test]
    fn rollback_is_freshly_isolated_and_idempotent() {
        let cmd = rollback_command("arni-labs/deep-sci-fi", SHA);
        assert!(cmd.contains("GIT_TERMINAL_PROMPT=0"));
        // Fresh per-attempt checkout, cleaned up — no reuse of a poisonable dir.
        assert!(cmd.contains("mktemp -d"), "{cmd}");
        assert!(cmd.contains("trap 'rm -rf \"$DIR\"' EXIT"), "{cmd}");
        assert!(!cmd.contains("~/workspace/release-"), "{cmd}");
        // Ambient git config/hooks neutralized against a poisoned sandbox.
        assert!(cmd.contains("GIT_CONFIG_GLOBAL=/dev/null"), "{cmd}");
        assert!(cmd.contains("GIT_CONFIG_SYSTEM=/dev/null"), "{cmd}");
        assert!(cmd.contains("core.hooksPath=/dev/null"), "{cmd}");
        assert!(cmd.contains("commit.gpgsign false"), "{cmd}");
        // Token supplied explicitly (ambient helper is off); $TOK is runtime.
        assert!(cmd.contains("~/.git-credentials"), "{cmd}");
        assert!(cmd.contains("https://x-access-token:$TOK@github.com/arni-labs/deep-sci-fi.git"), "{cmd}");
        // Configured identity so `git revert` can commit.
        assert!(cmd.contains("git config user.email"));
        // Bounded network calls.
        assert!(cmd.contains("timeout 120 git"));
        assert!(cmd.contains("timeout 60 git push"));
        assert!(cmd.contains(&format!("revert -m 1 --no-edit {SHA}")));
        assert!(cmd.contains("HEAD:main"), "{cmd}");
        // Idempotency: skip ONLY if the revert of this merge is at the TIP
        // (HEAD's message), not anywhere in history (ARN-394 re-review).
        assert!(cmd.contains(&format!("This reverts commit {SHA}")));
        assert!(cmd.contains("git log -1 --format=%B HEAD"), "{cmd}");
        assert!(!cmd.contains("git log origin/main --grep"), "{cmd}");
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
    fn handle_requires_an_explicitly_ready_computer() {
        let ready = json!({"Status":"Ready","fields":{"sandbox_url":"https://s.example","machine_id":"m1","provider":"tl"}});
        let h = sandbox_handle_from_computer(&ready).unwrap();
        assert_eq!(h.sandbox_url, "https://s.example");
        assert_eq!(h.provider, "tensorlake");
        // Not Ready.
        let sleeping = json!({"Status":"Sleeping","fields":{"sandbox_url":"https://s.example"}});
        assert!(sandbox_handle_from_computer(&sleeping).err().unwrap().contains("must be"));
        // No fail-open on empty/absent status.
        let no_status = json!({"fields":{"sandbox_url":"https://s.example"}});
        assert!(sandbox_handle_from_computer(&no_status).is_err());
        let bare = json!({"Status":"Ready","fields":{"sandbox_url":""}});
        assert!(sandbox_handle_from_computer(&bare).err().unwrap().contains("no sandbox_url"));
    }
}
