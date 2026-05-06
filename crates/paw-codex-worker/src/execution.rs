async fn run_repo_sweep(
    client: &reqwest::Client,
    config: &Config,
    worker_run: &WorkerRunState,
    snapshot_id: &str,
) -> Result<String> {
    let scan_root = ensure_worktree(config, worker_run).await?;

    info!(
        worker_run_id = %worker_run.id,
        snapshot_id,
        root = %scan_root.display(),
        "running repo graph and dependency sweep"
    );

    let graph = scan_repo_health(&scan_root)?;
    let graph_json = serde_json::to_string(&graph).context("serialize repo sweep graph")?;
    let finding_count = graph.quality_findings.len() + graph.security_findings.len();
    let summary = repo_sweep_summary_markdown(&scan_root, &graph);

    post_entity_action(
        client,
        config,
        "RepoGraphSnapshots",
        snapshot_id,
        "ScanComplete",
        json!({
            "graph_json": graph_json,
            "summary_markdown": summary,
            "generated_at": generated_at_label(),
            "finding_count": finding_count.to_string()
        }),
    )
    .await?;

    Ok(format!(
        "Repo sweep completed for RepoGraphSnapshot {snapshot_id}: {} quality finding(s), {} security finding(s).\n\n{}",
        graph.quality_findings.len(),
        graph.security_findings.len(),
        repo_sweep_summary_markdown(&scan_root, &graph)
    ))
}

async fn repo_sweep_live_e2e_summary(
    client: &reqwest::Client,
    config: &Config,
    worker_run: &WorkerRunState,
) -> Result<String> {
    let snapshot_id = extract_repo_sweep_snapshot_id(&worker_run.task)
        .context("WorkerRun task did not include RepoGraphSnapshot id")?;
    let response = client
        .get(config.entity_url("RepoGraphSnapshots", &snapshot_id))
        .headers(headers(config)?)
        .send()
        .await
        .context("fetch RepoGraphSnapshot for live evidence")?;
    if !response.status().is_success() {
        bail!("fetch RepoGraphSnapshot returned {}", response.status());
    }
    let value: Value = response.json().await.context("parse RepoGraphSnapshot")?;
    let fields = value.get("fields").cloned().unwrap_or_else(|| json!({}));
    let status = first_string(
        &value,
        &fields,
        &["status", "Status"],
        &["status", "Status"],
    );
    let finding_count = first_string(
        &value,
        &fields,
        &["finding_count", "FindingCount"],
        &["finding_count", "FindingCount"],
    );
    Ok(format!(
        "RepoGraphSnapshot {snapshot_id} is {status} with finding_count={}. WorkerRun {} self-reported through Temper actions.",
        if finding_count.is_empty() {
            "unknown"
        } else {
            finding_count.as_str()
        },
        worker_run.id
    ))
}

async fn run_codex(config: &Config, worker_run: &WorkerRunState) -> Result<String> {
    if !config.enable_execution {
        return Ok(format!(
            "dry-run: would run codex for WorkerRun {}. Set PAW_CODEX_ENABLE_EXECUTION=1 to execute.",
            worker_run.id
        ));
    }

    let workdir = ensure_worktree(config, worker_run).await?;

    let task = if worker_run.task.is_empty() {
        "Inspect this WorkerRun and report that no task was provided.".to_string()
    } else {
        worker_run.task.clone()
    };

    info!(worker_run_id = %worker_run.id, workdir = %workdir.display(), "starting local codex");
    let output = run_codex_exec_command(config, &workdir, task, "run local codex exec").await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        bail!(
            "codex exec failed with status {:?}: {}{}{}",
            output.status.code(),
            stdout,
            if stdout.is_empty() || stderr.is_empty() {
                ""
            } else {
                "\n"
            },
            stderr
        );
    }

    let evidence = collect_worktree_evidence(&workdir)
        .await
        .unwrap_or_else(|error| WorktreeEvidence {
            status_short: format!("git evidence unavailable: {error}"),
            diff_stat: String::new(),
        });
    Ok(format_codex_success_summary(
        worker_run,
        &workdir,
        stdout.as_ref(),
        &evidence,
    ))
}

async fn collect_worktree_evidence(workdir: &Path) -> Result<WorktreeEvidence> {
    let status_short = git_capture(workdir, &["status", "--short"])
        .await
        .context("git status --short")?;
    let diff_stat = git_capture(workdir, &["diff", "--stat", "--"])
        .await
        .context("git diff --stat")?;

    Ok(WorktreeEvidence {
        status_short,
        diff_stat,
    })
}

fn format_codex_success_summary(
    worker_run: &WorkerRunState,
    workdir: &Path,
    stdout: &str,
    evidence: &WorktreeEvidence,
) -> String {
    let branch = worker_run_branch_label(worker_run);
    let stdout_tail = nonempty_block(&tail_string(stdout.trim(), 4_000), "(no stdout captured)");
    let status_short = nonempty_block(&evidence.status_short, "(clean worktree)");
    let diff_stat = nonempty_block(&evidence.diff_stat, "(no unstaged diff stat)");

    format!(
        "codex exec completed for WorkerRun {}\nBranch: {branch}\nWorktree: {}\n\nCodex stdout:\n{stdout_tail}\n\nWorktree evidence:\n```git-status\n{status_short}\n```\n\n```git-diff-stat\n{diff_stat}\n```",
        worker_run.id,
        workdir.display()
    )
}

async fn git_capture(workdir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workdir)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("run git {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "git {} failed with status {:?}: {}",
            args.join(" "),
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
}

fn nonempty_block(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value.trim_end().to_string()
    }
}

fn worker_run_branch_label(worker_run: &WorkerRunState) -> String {
    if !worker_run.branch_name.trim().is_empty() {
        worker_run.branch_name.clone()
    } else if !worker_run.worktree_path.trim().is_empty() {
        "(assigned worktree without branch)".to_string()
    } else {
        "(unassigned worktree)".to_string()
    }
}

async fn run_codex_review(
    config: &Config,
    worker_run: &WorkerRunState,
    review_run: &ReviewRunState,
) -> Result<ReviewDecision> {
    let workdir = ensure_worktree(config, worker_run).await?;
    let prompt = codex_review_prompt(worker_run, review_run);

    info!(
        worker_run_id = %worker_run.id,
        workdir = %workdir.display(),
        "starting local Codex reviewer"
    );
    let output = run_codex_exec_command(config, &workdir, prompt, "run local codex review").await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = if stderr.trim().is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}\n\n[stderr]\n{stderr}")
    };
    if !output.status.success() {
        bail!(
            "codex review failed with status {:?}: {}",
            output.status.code(),
            truncate_middle(&combined, 4_000)
        );
    }

    Ok(parse_codex_review_verdict(&combined))
}

async fn run_codex_exec_command(
    config: &Config,
    workdir: &Path,
    prompt: String,
    context_label: &str,
) -> Result<Output> {
    let mut command = Command::new(&config.codex_bin);
    command
        .args(codex_exec_args(workdir, &prompt))
        .current_dir(workdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    match timeout(config.codex_exec_timeout, command.output()).await {
        Ok(output) => output.with_context(|| context_label.to_string()),
        Err(_) => bail!(
            "codex exec timed out after {}s during {context_label}",
            config.codex_exec_timeout.as_secs()
        ),
    }
}

fn codex_exec_args(workdir: &Path, prompt: &str) -> Vec<std::ffi::OsString> {
    vec![
        "exec".into(),
        "--dangerously-bypass-approvals-and-sandbox".into(),
        "--cd".into(),
        workdir.as_os_str().to_os_string(),
        "--skip-git-repo-check".into(),
        prompt.into(),
    ]
}

async fn run_code_change_evaluation(
    client: &reqwest::Client,
    config: &Config,
    evaluation_run_id: &str,
    worker_run: &WorkerRunState,
) -> Result<()> {
    info!(
        evaluation_run_id,
        action = EVALUATION_START_LABEL,
        "starting code-change EvaluationRun"
    );
    post_entity_action(
        client,
        config,
        "EvaluationRuns",
        evaluation_run_id,
        "Start",
        json!({}),
    )
    .await?;

    let outcome = run_evaluation_commands(config, worker_run).await?;
    if outcome.passed {
        post_entity_action(
            client,
            config,
            "EvaluationRuns",
            evaluation_run_id,
            "Pass",
            json!({
                "results_json": outcome.results_json,
                "e2e_summary": outcome.e2e_summary,
            }),
        )
        .await
    } else {
        post_entity_action(
            client,
            config,
            "EvaluationRuns",
            evaluation_run_id,
            "Fail",
            json!({
                "results_json": outcome.results_json,
                "error_message": outcome.error_message,
            }),
        )
        .await
    }
}

async fn run_evaluation_commands(
    config: &Config,
    worker_run: &WorkerRunState,
) -> Result<EvaluationOutcome> {
    let workdir = ensure_worktree(config, worker_run).await?;
    let commands = evaluation_commands(env::var("PAW_CODEX_EVAL_COMMANDS").ok().as_deref());
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let mut results = Vec::new();
    let mut passed = true;

    for command in commands {
        info!(worker_run_id = %worker_run.id, command, "running evaluation command");
        let output = Command::new(&shell)
            .arg("-lc")
            .arg(&command)
            .current_dir(&workdir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("run evaluation command: {command}"))?;
        let status = output.status.code().unwrap_or(-1);
        let success = output.status.success();
        passed &= success;
        results.push(json!({
            "command": command,
            "success": success,
            "status": status,
            "stdout_tail": tail_string(&String::from_utf8_lossy(&output.stdout), 4_000),
            "stderr_tail": tail_string(&String::from_utf8_lossy(&output.stderr), 4_000),
        }));
    }

    let results_json = serde_json::to_string(&json!({
        "kind": "code_change_evaluation",
        "worker_run_id": worker_run.id,
        "commands": results,
    }))
    .context("serialize code-change evaluation result")?;
    let e2e_summary = if passed {
        format!(
            "Code-change evaluation passed for WorkerRun {} with configured local commands.",
            worker_run.id
        )
    } else {
        format!(
            "Code-change evaluation failed for WorkerRun {}; inspect results_json command output.",
            worker_run.id
        )
    };
    let error_message = if passed {
        String::new()
    } else {
        e2e_summary.clone()
    };

    Ok(EvaluationOutcome {
        passed,
        results_json,
        e2e_summary,
        error_message,
    })
}

fn codex_review_prompt(worker_run: &WorkerRunState, review_run: &ReviewRunState) -> String {
    format!(
        "You are the independent reviewer for a TemperPaw paw-patrol WorkerRun.\n\nWorkerRun: {}\nReviewRun worker reference: {}\nProofPacket: {}\nBranch: {}\n\nTask:\n{}\n\nReview requirements:\n1. Treat this as read-only review. Do not modify files.\n2. Inspect the git diff, changed files, tests/proofs, and Temper-native architecture constraints.\n3. Run targeted checks or live/E2E verification when relevant and mention exactly what you ran.\n4. Look for security, Cedar, WASM, Discord/user-facing, dependency, and readability risks.\n5. Return an explicit verdict marker on its own line: VERDICT: approve, VERDICT: request_changes, or VERDICT: escalate.\n6. Include SUMMARY: and LIVE_E2E: lines that are concise and human-readable.\n\nIf you cannot confidently approve, use request_changes or escalate.",
        worker_run.id,
        review_run.worker_run_id,
        if review_run.proof_packet_id.is_empty() {
            "(not attached yet)"
        } else {
            review_run.proof_packet_id.as_str()
        },
        worker_run_branch_label(worker_run),
        if worker_run.task.is_empty() {
            "(no task text recorded)"
        } else {
            worker_run.task.as_str()
        }
    )
}

fn parse_codex_review_verdict(output: &str) -> ReviewDecision {
    let raw_verdict = prefixed_line(output, "VERDICT:")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .replace('-', "_");
    let summary = prefixed_line(output, "SUMMARY:")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| truncate_middle(output.trim(), 2_000));
    let live_e2e_summary = prefixed_line(output, "LIVE_E2E:")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Reviewer did not provide a LIVE_E2E line.".to_string());

    match raw_verdict.trim() {
        "approve" | "approved" => ReviewDecision {
            action: ReviewDecisionAction::Approve,
            summary,
            live_e2e_summary,
            verdict: "approve".to_string(),
        },
        "request_changes" | "changes_requested" | "request changes" => ReviewDecision {
            action: ReviewDecisionAction::RequestChanges,
            summary,
            live_e2e_summary,
            verdict: "request_changes".to_string(),
        },
        "escalate" | "human" | "human_review" => ReviewDecision {
            action: ReviewDecisionAction::Escalate,
            summary,
            live_e2e_summary,
            verdict: "escalate".to_string(),
        },
        _ => ReviewDecision {
            action: ReviewDecisionAction::Escalate,
            summary: format!(
                "Codex reviewer did not provide an explicit VERDICT marker. Raw output: {}",
                truncate_middle(output.trim(), 2_000)
            ),
            live_e2e_summary,
            verdict: "escalate_missing_verdict".to_string(),
        },
    }
}

fn evaluation_commands(configured: Option<&str>) -> Vec<String> {
    configured
        .unwrap_or("cargo test -p temperpaw --test paw_patrol_foundation -- --nocapture")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn prefixed_line(output: &str, prefix: &str) -> Option<String> {
    output.lines().find_map(|line| {
        line.trim()
            .strip_prefix(prefix)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn tail_string(value: &str, max_chars: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars {
        value.to_string()
    } else {
        chars[chars.len() - max_chars..].iter().collect()
    }
}

fn truncate_middle(value: &str, max_chars: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars {
        return value.to_string();
    }
    let head = max_chars / 2;
    let tail = max_chars.saturating_sub(head + 15);
    format!(
        "{}\n...[truncated]...\n{}",
        chars[..head].iter().collect::<String>(),
        chars[chars.len() - tail..].iter().collect::<String>()
    )
}

async fn ensure_worktree(config: &Config, worker_run: &WorkerRunState) -> Result<PathBuf> {
    if worker_run.worktree_path.is_empty() && worker_run.branch_name.trim().is_empty() {
        bail!(
            "WorkerRun {} has no assigned worktree_path or branch_name; refusing to run in the main checkout",
            worker_run.id
        );
    }

    let worktree = if worker_run.worktree_path.is_empty() {
        config
            .workspace_root
            .join(worker_run.branch_name.replace('/', "-"))
    } else {
        PathBuf::from(&worker_run.worktree_path)
    };
    if worktree.exists() {
        return Ok(worktree);
    }

    if worker_run.branch_name.trim().is_empty() {
        bail!(
            "WorkerRun {} has worktree_path but no branch_name",
            worker_run.id
        );
    }

    if let Some(parent) = worktree.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }

    info!(
        worker_run_id = %worker_run.id,
        branch = %worker_run.branch_name,
        worktree = %worktree.display(),
        repo = %config.repo_root.display(),
        "creating git worktree"
    );
    let output = Command::new("git")
        .arg("-C")
        .arg(&config.repo_root)
        .arg("worktree")
        .arg("add")
        .arg("-B")
        .arg(&worker_run.branch_name)
        .arg(&worktree)
        .arg("HEAD")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("git worktree add")?;

    if !output.status.success() {
        bail!(
            "git worktree add failed with status {:?}: {}{}{}",
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

    Ok(worktree)
}

fn generated_at_label() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("unix:{seconds}")
}
