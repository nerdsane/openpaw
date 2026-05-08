async fn publish_pull_request_if_needed(
    config: &Config,
    worker_run: &WorkerRunState,
    workdir: &Path,
    evidence: &WorktreeEvidence,
    mode: PullRequestMode,
) -> Result<Option<PullRequestEvidence>> {
    match mode {
        PullRequestMode::Disabled => Ok(None),
        PullRequestMode::Optional | PullRequestMode::Required => {
            if !worktree_has_changes(&evidence.status_short) {
                return Ok(Some(PullRequestEvidence {
                    mode,
                    url: String::new(),
                    commit_sha: String::new(),
                    branch_name: worker_run.branch_name.clone(),
                    base_branch: pull_request_base_branch(),
                    note: "No PR was created because Codex left the assigned worktree clean."
                        .to_string(),
                }));
            }

            match create_github_pull_request(config, worker_run, workdir, evidence, mode).await {
                Ok(evidence) => Ok(Some(evidence)),
                Err(error) if mode == PullRequestMode::Optional => Ok(Some(PullRequestEvidence {
                    mode,
                    url: String::new(),
                    commit_sha: String::new(),
                    branch_name: worker_run.branch_name.clone(),
                    base_branch: pull_request_base_branch(),
                    note: format!(
                        "Optional PR publication failed; WorkerRun was allowed to report done: {error:#}"
                    ),
                })),
                Err(error) => Err(error),
            }
        }
    }
}

async fn create_github_pull_request(
    config: &Config,
    worker_run: &WorkerRunState,
    workdir: &Path,
    evidence: &WorktreeEvidence,
    mode: PullRequestMode,
) -> Result<PullRequestEvidence> {
    let branch_name = worker_run.branch_name.trim();
    if branch_name.is_empty() {
        bail!(
            "PAW_CODEX_PR_MODE={} requires WorkerRun.BranchName",
            mode.as_str()
        );
    }
    let branch_name = branch_name.to_string();
    let base_branch = pull_request_base_branch();

    ensure_git_identity(workdir).await?;
    git_capture(workdir, &["add", "-A"])
        .await
        .context("stage Codex work before PR publication")?;
    let staged_paths = git_capture(workdir, &["diff", "--cached", "--name-only"])
        .await
        .context("inspect staged Codex work before PR publication")?;
    if staged_paths.trim().is_empty() {
        return Ok(PullRequestEvidence {
            mode,
            url: String::new(),
            commit_sha: String::new(),
            branch_name,
            base_branch,
            note: "No PR was created because there were no staged changes after git add -A."
                .to_string(),
        });
    }

    let title = pull_request_title(worker_run);
    let body = pull_request_body(config, worker_run, workdir, evidence, &base_branch);
    git_capture_owned(
        workdir,
        vec![
            "commit".to_string(),
            "-m".to_string(),
            title.clone(),
            "-m".to_string(),
            body.clone(),
        ],
    )
    .await
    .context("commit Codex work before PR publication")?;
    let commit_sha = git_capture(workdir, &["rev-parse", "HEAD"])
        .await
        .context("read Codex worker commit sha")?;
    git_capture_owned(
        workdir,
        vec![
            "push".to_string(),
            "-u".to_string(),
            "origin".to_string(),
            branch_name.clone(),
        ],
    )
    .await
    .with_context(|| format!("push Codex worker branch {branch_name}"))?;

    let url = match github_pr_view_url(workdir, &branch_name).await? {
        Some(url) => url,
        None => github_pr_create_url(workdir, &title, &body, &base_branch, &branch_name).await?,
    };

    Ok(PullRequestEvidence {
        mode,
        url,
        commit_sha,
        branch_name,
        base_branch,
        note: "Ready GitHub PR created or reused by paw-codex-worker after Codex finished."
            .to_string(),
    })
}

async fn ensure_git_identity(workdir: &Path) -> Result<()> {
    let name = git_capture(workdir, &["config", "user.name"])
        .await
        .unwrap_or_default();
    if name.trim().is_empty() {
        git_capture(workdir, &["config", "user.name", "paw-codex-worker"])
            .await
            .context("set local git user.name for paw-codex-worker")?;
    }
    let email = git_capture(workdir, &["config", "user.email"])
        .await
        .unwrap_or_default();
    if email.trim().is_empty() {
        git_capture(
            workdir,
            &[
                "config",
                "user.email",
                "paw-codex-worker@users.noreply.github.com",
            ],
        )
        .await
        .context("set local git user.email for paw-codex-worker")?;
    }
    Ok(())
}

async fn github_pr_view_url(workdir: &Path, branch_name: &str) -> Result<Option<String>> {
    let args = vec![
        "pr".to_string(),
        "view".to_string(),
        "--head".to_string(),
        branch_name.to_string(),
        "--json".to_string(),
        "url".to_string(),
        "--jq".to_string(),
        ".url".to_string(),
    ];
    let output = Command::new("gh")
        .args(&args)
        .current_dir(workdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("run gh pr view")?;
    if !output.status.success() {
        return Ok(None);
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!url.is_empty()).then_some(url))
}

async fn github_pr_create_url(
    workdir: &Path,
    title: &str,
    body: &str,
    base_branch: &str,
    branch_name: &str,
) -> Result<String> {
    let args = vec![
        "pr".to_string(),
        "create".to_string(),
        "--title".to_string(),
        title.to_string(),
        "--body".to_string(),
        body.to_string(),
        "--base".to_string(),
        base_branch.to_string(),
        "--head".to_string(),
        branch_name.to_string(),
    ];
    let output = Command::new("gh")
        .args(&args)
        .current_dir(workdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("run gh pr create")?;
    if !output.status.success() {
        bail!(
            "gh pr create failed with status {:?}: {}{}{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            if output.stdout.is_empty() || output.stderr.is_empty() {
                ""
            } else {
                "\n"
            },
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() {
        bail!("gh pr create succeeded without returning a PR URL");
    }
    Ok(url)
}

async fn git_capture_owned(workdir: &Path, args: Vec<String>) -> Result<String> {
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    git_capture(workdir, &refs).await
}

fn worktree_has_changes(status_short: &str) -> bool {
    status_short.lines().any(|line| !line.trim().is_empty())
}

fn pull_request_mode_from_env() -> PullRequestMode {
    if env::var("PAW_CODEX_ENABLE_GITHUB_PR")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return PullRequestMode::Required;
    }

    env::var("PAW_CODEX_PR_MODE")
        .ok()
        .as_deref()
        .map(parse_pull_request_mode)
        .unwrap_or(PullRequestMode::Disabled)
}

fn parse_pull_request_mode(value: &str) -> PullRequestMode {
    match value.trim().to_ascii_lowercase().as_str() {
        "required" | "require" | "1" | "true" => PullRequestMode::Required,
        "optional" | "try" => PullRequestMode::Optional,
        _ => PullRequestMode::Disabled,
    }
}

fn pull_request_base_branch() -> String {
    env::var("PAW_CODEX_PR_BASE_BRANCH")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "main".to_string())
}

fn pull_request_title(worker_run: &WorkerRunState) -> String {
    format!("Paw Patrol WorkerRun {}", worker_run.id)
}

fn pull_request_body(
    config: &Config,
    worker_run: &WorkerRunState,
    workdir: &Path,
    evidence: &WorktreeEvidence,
    base_branch: &str,
) -> String {
    format!(
        "Created automatically by paw-codex-worker.\n\nWorkerRun: `{}`\nWorker: `{}`\nBase branch: `{}`\nHead branch: `{}`\nWorktree: `{}`\n\nTask:\n{}\n\nCodex change evidence:\n```git-status\n{}\n```\n\n```git-diff-stat\n{}\n```\n\nAfter this PR is reported to Temper, Patrol should run independent ReviewRun and EvaluationRun gates before human merge review.",
        worker_run.id,
        config.worker_id,
        base_branch,
        worker_run_branch_label(worker_run),
        workdir.display(),
        nonempty_block(&worker_run.task, "(no task text recorded)"),
        nonempty_block(&evidence.status_short, "(clean worktree)"),
        nonempty_block(&evidence.diff_stat, "(no unstaged diff stat)")
    )
}

fn format_pull_request_evidence(evidence: &PullRequestEvidence) -> String {
    let url = if evidence.url.trim().is_empty() {
        "(not created)"
    } else {
        evidence.url.as_str()
    };
    let commit = if evidence.commit_sha.trim().is_empty() {
        "(none)"
    } else {
        evidence.commit_sha.as_str()
    };

    format!(
        "\n\nPull request:\nMode: {}\nURL: {url}\nBase: {}\nBranch: {}\nCommit: {commit}\nNote: {}",
        evidence.mode.as_str(),
        evidence.base_branch,
        evidence.branch_name,
        evidence.note
    )
}
