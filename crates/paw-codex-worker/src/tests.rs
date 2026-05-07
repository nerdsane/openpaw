#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    include!("datadog_patrol_tests.rs");
    include!("fake_codex_tests.rs");

    #[test]
    fn worker_run_state_reads_temper_odata_fields() {
        let value = json!({
            "entity_id": "wr-1",
            "status": "Queued",
            "fields": {
                "task": "Do useful work",
                "worktree_path": "/tmp/worktree",
                "branch_name": "codex/test",
                "runner_kind": "local_codex",
                "allowed_worker_id": "mac-mini-codex-1",
                "worker_id": "mac-mini-codex-1",
                "provider_id": "local-codex",
                "required_capabilities": "local_codex,repo_write"
            }
        });

        let worker_run = worker_run_from_odata_value(value).expect("worker run should parse");

        assert_eq!(worker_run.id, "wr-1");
        assert_eq!(worker_run.status, "Queued");
        assert_eq!(worker_run.task, "Do useful work");
        assert_eq!(worker_run.worktree_path, "/tmp/worktree");
        assert_eq!(worker_run.branch_name, "codex/test");
        assert_eq!(worker_run.runner_kind, "local_codex");
        assert_eq!(worker_run.allowed_worker_id, "mac-mini-codex-1");
        assert_eq!(worker_run.worker_id, "mac-mini-codex-1");
        assert_eq!(worker_run.provider_id, "local-codex");
        assert_eq!(worker_run.required_capabilities, "local_codex,repo_write");
    }

    #[test]
    fn local_worker_claims_only_configured_local_codex_runs() {
        let configured = WorkerRunState {
            id: "wr-1".to_string(),
            status: "Queued".to_string(),
            task: "Do useful work".to_string(),
            worktree_path: "/tmp/worktree".to_string(),
            branch_name: "codex/test".to_string(),
            runner_kind: "local_codex".to_string(),
            allowed_worker_id: "mac-mini-codex-1".to_string(),
            worker_id: String::new(),
            provider_id: "local-codex".to_string(),
            required_capabilities: "local_codex,repo_write".to_string(),
        };
        let unconfigured = WorkerRunState {
            id: "wr-2".to_string(),
            status: "Queued".to_string(),
            task: String::new(),
            worktree_path: String::new(),
            branch_name: String::new(),
            runner_kind: String::new(),
            allowed_worker_id: String::new(),
            worker_id: String::new(),
            provider_id: String::new(),
            required_capabilities: String::new(),
        };
        let cloud = WorkerRunState {
            id: "wr-3".to_string(),
            status: "Queued".to_string(),
            task: "Cloud overflow".to_string(),
            worktree_path: String::new(),
            branch_name: String::new(),
            runner_kind: "codex_cloud".to_string(),
            allowed_worker_id: "mac-mini-codex-1".to_string(),
            worker_id: String::new(),
            provider_id: "local-codex".to_string(),
            required_capabilities: "local_codex,repo_write".to_string(),
        };
        let no_worktree_assignment = WorkerRunState {
            id: "wr-4".to_string(),
            status: "Queued".to_string(),
            task: "Fix a Discord trace leak".to_string(),
            worktree_path: String::new(),
            branch_name: String::new(),
            runner_kind: "local_codex".to_string(),
            allowed_worker_id: "mac-mini-codex-1".to_string(),
            worker_id: String::new(),
            provider_id: "local-codex".to_string(),
            required_capabilities: "local_codex,repo_write".to_string(),
        };

        assert!(worker_run_is_claimable_by_local_codex(
            &configured,
            "mac-mini-codex-1"
        ));
        assert!(!worker_run_is_claimable_by_local_codex(
            &configured,
            "other-worker"
        ));
        assert!(!worker_run_is_claimable_by_local_codex(
            &unconfigured,
            "mac-mini-codex-1"
        ));
        assert!(!worker_run_is_claimable_by_local_codex(
            &cloud,
            "mac-mini-codex-1"
        ));
        assert!(!worker_run_is_claimable_by_local_codex(
            &no_worktree_assignment,
            "mac-mini-codex-1"
        ));
    }

    #[test]
    fn local_worker_recovers_only_own_running_codex_runs() {
        let recoverable = WorkerRunState {
            id: "wr-1".to_string(),
            status: "Running".to_string(),
            task: "Resume this work".to_string(),
            worktree_path: "/tmp/worktree".to_string(),
            branch_name: "codex/test".to_string(),
            runner_kind: "local_codex".to_string(),
            allowed_worker_id: "mac-mini-codex-1".to_string(),
            worker_id: "mac-mini-codex-1".to_string(),
            provider_id: "local-codex".to_string(),
            required_capabilities: "local_codex,repo_write".to_string(),
        };
        let other_worker = WorkerRunState {
            worker_id: "other-worker".to_string(),
            ..recoverable.clone()
        };
        let queued = WorkerRunState {
            status: "Queued".to_string(),
            ..recoverable.clone()
        };

        assert!(worker_run_is_recoverable_by_local_codex(
            &recoverable,
            "mac-mini-codex-1"
        ));
        assert!(!worker_run_is_recoverable_by_local_codex(
            &other_worker,
            "mac-mini-codex-1"
        ));
        assert!(!worker_run_is_recoverable_by_local_codex(
            &queued,
            "mac-mini-codex-1"
        ));
    }

    #[test]
    fn repo_sweep_worker_runs_are_detected_for_review_and_evaluation() {
        let worker_run = WorkerRunState {
            id: "wr-1".to_string(),
            status: "Done".to_string(),
            task: "RepoGraphSnapshot: snap-1\nWorkCycle: wc-1".to_string(),
            worktree_path: "/tmp/worktree".to_string(),
            branch_name: "codex/repo-sweep".to_string(),
            runner_kind: "local_codex".to_string(),
            allowed_worker_id: "mac-mini-codex-1".to_string(),
            worker_id: String::new(),
            provider_id: "local-codex".to_string(),
            required_capabilities: "local_codex,repo_write".to_string(),
        };
        let normal_worker_run = WorkerRunState {
            id: "wr-2".to_string(),
            status: "Done".to_string(),
            task: "Fix a Discord reply bug".to_string(),
            worktree_path: "/tmp/worktree".to_string(),
            branch_name: "codex/bugfix".to_string(),
            runner_kind: "local_codex".to_string(),
            allowed_worker_id: "mac-mini-codex-1".to_string(),
            worker_id: String::new(),
            provider_id: "local-codex".to_string(),
            required_capabilities: "local_codex,repo_write".to_string(),
        };

        assert!(worker_run_is_repo_sweep(&worker_run));
        assert!(!worker_run_is_repo_sweep(&normal_worker_run));
    }

    #[test]
    fn worker_command_defaults_to_run_and_accepts_doctor() {
        assert_eq!(
            parse_worker_command(Vec::<String>::new()),
            WorkerCommand::Run
        );
        assert_eq!(
            parse_worker_command(vec!["doctor".to_string()]),
            WorkerCommand::Doctor
        );
        assert_eq!(
            parse_worker_command(vec!["--doctor".to_string()]),
            WorkerCommand::Doctor
        );
        assert_eq!(
            parse_worker_command(vec!["launchd-plist".to_string()]),
            WorkerCommand::LaunchdPlist
        );
        assert_eq!(
            parse_worker_command(vec!["--launchd-plist".to_string()]),
            WorkerCommand::LaunchdPlist
        );
        assert_eq!(
            parse_worker_command(vec!["run".to_string()]),
            WorkerCommand::Run
        );
    }

    #[test]
    fn launchd_plist_renders_concrete_worker_environment() {
        let config = Config {
            temper_url: "https://temperpaw.example.test".to_string(),
            tenant: "prod".to_string(),
            worker_id: "mac-mini-codex-1".to_string(),
            worker_token: Some("secret&token".to_string()),
            workspace_root: PathBuf::from("/Users/me/Development/temperpaw-worktrees"),
            repo_root: PathBuf::from("/Users/me/Development/temperpaw"),
            codex_bin: "/Users/me/.local/bin/codex".to_string(),
            max_concurrent_runs: 1,
            enable_execution: true,
            poll_on_start: false,
            codex_exec_smoke: true,
            codex_exec_timeout: Duration::from_secs(90),
        };

        let plist = render_launchd_plist(
            &config,
            Path::new("/Users/me/.local/bin/paw-codex-worker"),
            Some("cargo test -p temperpaw --test paw_patrol_foundation"),
        );

        for needle in [
            "<key>Label</key>",
            "<string>com.temperpaw.paw-codex-worker</string>",
            "<string>/Users/me/.local/bin/paw-codex-worker</string>",
            "<key>TEMPER_URL</key>",
            "<string>https://temperpaw.example.test</string>",
            "<key>WORKER_ID</key>",
            "<string>mac-mini-codex-1</string>",
            "<key>PATH</key>",
            "<string>/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/Users/openclaw/.cargo/bin</string>",
            "<key>PAW_CODEX_ENABLE_EXECUTION</key>",
            "<string>1</string>",
            "<key>PAW_CODEX_POLL_ON_START</key>",
            "<string>0</string>",
            "<key>PAW_CODEX_DOCTOR_EXEC_SMOKE</key>",
            "<string>1</string>",
            "<key>PAW_CODEX_EXEC_TIMEOUT_SECS</key>",
            "<string>90</string>",
            "<key>PAW_CODEX_WORKER_CAPABILITIES</key>",
            "<string>local_codex,repo_write,review,evaluation,datadog_query</string>",
            "<key>PAW_CODEX_WORKER_ENV_FILE</key>",
            "<string>/Users/openclaw/.config/temperpaw/paw-codex-worker.env</string>",
            "<key>PAW_CODEX_EVAL_COMMANDS</key>",
            "<string>cargo test -p temperpaw --test paw_patrol_foundation</string>",
        ] {
            assert!(plist.contains(needle), "plist should contain {needle}");
        }
        assert!(
            !plist.contains("<key>WORKER_TOKEN</key>"),
            "launchd plist should keep WORKER_TOKEN in the 0600 env file, not in launchctl-visible environment"
        );
    }

    #[test]
    fn doctor_status_fails_only_on_failures() {
        let pass_and_warn = vec![
            DoctorCheck::pass("repo", "ok".to_string()),
            DoctorCheck::warn("token", "missing token".to_string()),
        ];
        let fail = vec![DoctorCheck::fail("odata", "not reachable".to_string())];

        assert!(!doctor_has_failures(&pass_and_warn));
        assert!(doctor_has_failures(&fail));
        assert_eq!(doctor_status_label(DoctorStatus::Pass), "pass");
        assert_eq!(doctor_status_label(DoctorStatus::Warn), "warn");
        assert_eq!(doctor_status_label(DoctorStatus::Fail), "fail");
    }

    #[tokio::test]
    async fn doctor_codex_exec_smoke_runs_fixture_when_enabled() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/fake-codex.sh");
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("temp dir");
        let config = Config {
            temper_url: "http://127.0.0.1:3497".to_string(),
            tenant: "default".to_string(),
            worker_id: "mac-mini-codex-1".to_string(),
            worker_token: Some("secret".to_string()),
            workspace_root: root.clone(),
            repo_root: root.clone(),
            codex_bin: fixture.display().to_string(),
            max_concurrent_runs: 1,
            enable_execution: false,
            poll_on_start: true,
            codex_exec_smoke: true,
            codex_exec_timeout: Duration::from_secs(30),
        };

        let check = check_codex_exec_smoke(&config).await;

        assert_eq!(check.name, "codex_exec_smoke");
        assert_eq!(check.status, DoctorStatus::Pass);
        assert!(
            check.detail.contains("PAW_CODEX_DOCTOR_EXEC_OK"),
            "unexpected detail: {}",
            check.detail
        );
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn run_codex_times_out_stuck_local_exec() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/fake-codex.sh");
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("temp dir");
        let config = Config {
            temper_url: "http://127.0.0.1:3497".to_string(),
            tenant: "default".to_string(),
            worker_id: "mac-mini-codex-1".to_string(),
            worker_token: Some("secret".to_string()),
            workspace_root: root.clone(),
            repo_root: root.clone(),
            codex_bin: fixture.display().to_string(),
            max_concurrent_runs: 1,
            enable_execution: true,
            poll_on_start: true,
            codex_exec_smoke: false,
            codex_exec_timeout: Duration::from_millis(100),
        };
        let worker_run = WorkerRunState {
            id: "wr-timeout".to_string(),
            status: "Running".to_string(),
            task: "PAW_FAKE_CODEX_HANG: simulate a stuck Codex child".to_string(),
            worktree_path: root.display().to_string(),
            branch_name: "codex/timeout".to_string(),
            runner_kind: "local_codex".to_string(),
            allowed_worker_id: "mac-mini-codex-1".to_string(),
            worker_id: "mac-mini-codex-1".to_string(),
            provider_id: "local-codex".to_string(),
            required_capabilities: "local_codex,repo_write".to_string(),
        };

        let result = run_codex(&config, &worker_run).await;

        let error = result.expect_err("stuck codex exec should time out");
        let message = format!("{error:#}");
        assert!(
            message.contains("codex exec timed out after"),
            "unexpected timeout error: {message}"
        );
        fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn run_codex_timeout_cleans_child_process_group() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/fake-codex.sh");
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("temp dir");
        let marker = root.join("orphan-survived");
        let config = Config {
            temper_url: "http://127.0.0.1:3497".to_string(),
            tenant: "default".to_string(),
            worker_id: "mac-mini-codex-1".to_string(),
            worker_token: Some("secret".to_string()),
            workspace_root: root.clone(),
            repo_root: root.clone(),
            codex_bin: fixture.display().to_string(),
            max_concurrent_runs: 1,
            enable_execution: true,
            poll_on_start: true,
            codex_exec_smoke: false,
            codex_exec_timeout: Duration::from_millis(100),
        };
        let worker_run = WorkerRunState {
            id: "wr-timeout-tree".to_string(),
            status: "Running".to_string(),
            task: format!("PAW_FAKE_CODEX_ORPHAN:{}", marker.display()),
            worktree_path: root.display().to_string(),
            branch_name: "codex/timeout-tree".to_string(),
            runner_kind: "local_codex".to_string(),
            allowed_worker_id: "mac-mini-codex-1".to_string(),
            worker_id: "mac-mini-codex-1".to_string(),
            provider_id: "local-codex".to_string(),
            required_capabilities: "local_codex,repo_write".to_string(),
        };

        let result = run_codex(&config, &worker_run).await;

        let error = result.expect_err("stuck codex exec should time out");
        assert!(
            format!("{error:#}").contains("codex exec timed out after"),
            "unexpected timeout error: {error:#}"
        );
        sleep(Duration::from_millis(1_500)).await;
        assert!(
            !marker.exists(),
            "timed-out codex exec must not leave descendant processes running"
        );
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn codex_review_verdict_requires_explicit_marker() {
        let approved =
            parse_codex_review_verdict("SUMMARY: Looks good\nVERDICT: approve\nLIVE_E2E: passed");
        let changes =
            parse_codex_review_verdict("VERDICT: request_changes\nSUMMARY: Missing E2E proof");
        let escalated =
            parse_codex_review_verdict("VERDICT: escalate\nSUMMARY: Cedar risk needs human");
        let unknown = parse_codex_review_verdict("Looks fine to me");

        assert_eq!(approved.action, ReviewDecisionAction::Approve);
        assert_eq!(changes.action, ReviewDecisionAction::RequestChanges);
        assert_eq!(escalated.action, ReviewDecisionAction::Escalate);
        assert_eq!(unknown.action, ReviewDecisionAction::Escalate);
        assert!(unknown.summary.contains("explicit VERDICT"));
    }

    #[test]
    fn evaluation_commands_default_to_temperpaw_foundation_check() {
        let commands = evaluation_commands(None);
        assert_eq!(
            commands,
            vec!["cargo test -p temperpaw --test paw_patrol_foundation -- --nocapture"]
        );

        let configured =
            evaluation_commands(Some("cargo check -p paw-codex-worker\n git diff --check "));
        assert_eq!(
            configured,
            vec!["cargo check -p paw-codex-worker", "git diff --check"]
        );
    }

    #[test]
    fn codex_success_summary_includes_git_evidence_for_proof_packets() {
        let worker_run = WorkerRunState {
            id: "wr-proof".to_string(),
            status: "Running".to_string(),
            task: "Fix a Discord trace leak".to_string(),
            worktree_path: "/tmp/paw-worktree".to_string(),
            branch_name: "codex/trace-leak".to_string(),
            runner_kind: "local_codex".to_string(),
            allowed_worker_id: "mac-mini-codex-1".to_string(),
            worker_id: "mac-mini-codex-1".to_string(),
            provider_id: "local-codex".to_string(),
            required_capabilities: "local_codex,repo_write".to_string(),
        };
        let evidence = WorktreeEvidence {
            status_short: " M crates/temperpaw/src/discord.rs\n?? docs/proofs/trace.md\n"
                .to_string(),
            diff_stat: "crates/temperpaw/src/discord.rs | 12 ++++++------".to_string(),
        };

        let summary = format_codex_success_summary(
            &worker_run,
            Path::new("/tmp/paw-worktree"),
            "implemented the fix",
            &evidence,
        );

        assert!(summary.contains("codex exec completed for WorkerRun wr-proof"));
        assert!(summary.contains("Worktree: /tmp/paw-worktree"));
        assert!(summary.contains("```git-status"));
        assert!(summary.contains(" M crates/temperpaw/src/discord.rs"));
        assert!(summary.contains("?? docs/proofs/trace.md"));
        assert!(summary.contains("```git-diff-stat"));
        assert!(summary.contains("Codex stdout"));
    }

    #[test]
    fn codex_exec_bypasses_sandbox_for_assigned_worktree() {
        let args = codex_exec_args(Path::new("/tmp/paw-worktree"), "Create the proof file");
        let args = args
            .iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            vec![
                "exec",
                "--dangerously-bypass-approvals-and-sandbox",
                "--cd",
                "/tmp/paw-worktree",
                "--skip-git-repo-check",
                "Create the proof file"
            ]
        );
    }

    #[test]
    fn worker_proof_text_does_not_call_assigned_worktree_current_checkout() {
        let worker_run = WorkerRunState {
            id: "wr-existing-worktree".to_string(),
            status: "Running".to_string(),
            task: "Inspect an existing assigned worktree".to_string(),
            worktree_path: "/tmp/paw-existing-worktree".to_string(),
            branch_name: String::new(),
            runner_kind: "local_codex".to_string(),
            allowed_worker_id: "mac-mini-codex-1".to_string(),
            worker_id: "mac-mini-codex-1".to_string(),
            provider_id: "local-codex".to_string(),
            required_capabilities: "local_codex,repo_write".to_string(),
        };
        let evidence = WorktreeEvidence {
            status_short: String::new(),
            diff_stat: String::new(),
        };

        let summary = format_codex_success_summary(
            &worker_run,
            Path::new("/tmp/paw-existing-worktree"),
            "inspected",
            &evidence,
        );
        let review = ReviewRunState {
            status: "Requested".to_string(),
            worker_run_id: worker_run.id.clone(),
            proof_packet_id: "proof-1".to_string(),
        };
        let prompt = codex_review_prompt(&worker_run, &review);

        assert!(summary.contains("Branch: (assigned worktree without branch)"));
        assert!(prompt.contains("Branch: (assigned worktree without branch)"));
        assert!(!summary.contains("Branch: (current checkout)"));
        assert!(!prompt.contains("Branch: (current checkout)"));
    }

    #[test]
    fn repo_health_review_prompt_reviews_scan_contract_not_patch_contract() {
        let worker_run = WorkerRunState {
            id: "wr-repo-scan".to_string(),
            status: "Done".to_string(),
            task: "RepoGraphSnapshot: snap-1\nWorkCycle: wc-1\nRun an agent-led repo health patrol.".to_string(),
            worktree_path: "/tmp/paw-repo-scan".to_string(),
            branch_name: "codex/paw-repo-sweep-snap-1".to_string(),
            runner_kind: "local_codex".to_string(),
            allowed_worker_id: "mac-mini-codex-1".to_string(),
            worker_id: "mac-mini-codex-1".to_string(),
            provider_id: "local-codex".to_string(),
            required_capabilities: "local_codex,repo_write,evaluation".to_string(),
        };
        let review = ReviewRunState {
            status: "Requested".to_string(),
            worker_run_id: worker_run.id.clone(),
            proof_packet_id: "proof-1".to_string(),
        };

        let prompt = codex_review_prompt(&worker_run, &review);

        assert!(prompt.contains("repo-health Patrol scan reviewer"));
        assert!(prompt.contains("RepoGraphSnapshot.ScanComplete"));
        assert!(prompt.contains("do not require an implementation patch"));
        assert!(!prompt.contains("Inspect the git diff, changed files, tests/proofs"));
    }

    #[test]
    fn review_evaluation_and_work_cycle_state_read_temper_odata_fields() {
        let review = review_run_from_odata_value(json!({
            "entity_id": "rev-1",
            "status": "Requested",
            "fields": {
                "work_cycle_id": "wc-1",
                "worker_run_id": "wr-1",
                "proof_packet_id": "proof-1"
            }
        }))
        .expect("review run");
        let evaluation = evaluation_run_from_odata_value(json!({
            "entity_id": "eval-1",
            "status": "Queued",
            "fields": {
                "work_cycle_id": "wc-1",
                "required_checks": "[\"repo-sweep\"]"
            }
        }))
        .expect("evaluation run");
        let work_cycle = work_cycle_from_odata_value(json!({
            "entity_id": "wc-1",
            "status": "Reviewing",
            "fields": {
                "implementer_worker_run_id": "wr-1",
                "reviewer_run_id": "rev-1",
                "evaluation_run_id": "eval-1",
                "review_passed": "true"
            }
        }))
        .expect("work cycle");

        assert_eq!(review.worker_run_id, "wr-1");
        assert_eq!(evaluation.work_cycle_id, "wc-1");
        assert_eq!(work_cycle.implementer_worker_run_id, "wr-1");
        assert!(work_cycle.review_passed);
    }

    #[test]
    fn worker_headers_identify_the_daemon_as_an_agent_principal() {
        let config = Config {
            temper_url: "http://127.0.0.1:3497".to_string(),
            tenant: "default".to_string(),
            worker_id: "mac-mini-codex-1".to_string(),
            worker_token: Some("secret".to_string()),
            workspace_root: PathBuf::from("/tmp/worktrees"),
            repo_root: PathBuf::from("/tmp/temperpaw"),
            codex_bin: "codex".to_string(),
            max_concurrent_runs: 1,
            enable_execution: false,
            poll_on_start: true,
            codex_exec_smoke: false,
            codex_exec_timeout: Duration::from_secs(30),
        };

        let headers = headers(&config).expect("headers");

        assert_eq!(
            headers
                .get("x-temper-principal-kind")
                .and_then(|value| value.to_str().ok()),
            Some("agent")
        );
        assert_eq!(
            headers
                .get("x-temper-principal-id")
                .and_then(|value| value.to_str().ok()),
            Some("mac-mini-codex-1")
        );
    }

    #[test]
    fn event_stream_headers_use_token_without_worker_principal_headers() {
        let config = Config {
            temper_url: "http://127.0.0.1:3497".to_string(),
            tenant: "default".to_string(),
            worker_id: "mac-mini-codex-1".to_string(),
            worker_token: Some("secret".to_string()),
            workspace_root: PathBuf::from("/tmp/worktrees"),
            repo_root: PathBuf::from("/tmp/temperpaw"),
            codex_bin: "codex".to_string(),
            max_concurrent_runs: 1,
            enable_execution: false,
            poll_on_start: true,
            codex_exec_smoke: false,
            codex_exec_timeout: Duration::from_secs(30),
        };

        let headers = event_stream_headers(&config).expect("headers");

        assert_eq!(
            headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer secret")
        );
        assert!(
            !headers.contains_key("x-temper-principal-kind"),
            "current Temper event stream rejects agent principals; WorkerRun actions still use worker identity"
        );
    }

    #[test]
    fn event_urls_try_planned_and_current_temper_streams() {
        let config = Config {
            temper_url: "http://127.0.0.1:3497".to_string(),
            tenant: "default".to_string(),
            worker_id: "mac-mini-codex-1".to_string(),
            worker_token: Some("secret".to_string()),
            workspace_root: PathBuf::from("/tmp/worktrees"),
            repo_root: PathBuf::from("/tmp/temperpaw"),
            codex_bin: "codex".to_string(),
            max_concurrent_runs: 1,
            enable_execution: false,
            poll_on_start: true,
            codex_exec_smoke: false,
            codex_exec_timeout: Duration::from_secs(30),
        };

        assert_eq!(
            config.events_urls(),
            vec![
                "http://127.0.0.1:3497/tdata/$events".to_string(),
                "http://127.0.0.1:3497/observe/events/stream".to_string()
            ]
        );
    }

    #[test]
    fn event_stream_watch_does_not_wrap_active_work() {
        let main_src = include_str!("main.rs");

        assert!(
            main_src.contains("match watch_events(&client, &config).await"),
            "main loop should not wrap the whole event watcher in a timeout"
        );
        assert_eq!(event_stream_queue_poll_interval(), Duration::from_secs(15));
    }

    #[test]
    fn worker_event_status_reads_observe_stream_shape() {
        let event: EntityEvent = serde_json::from_value(json!({
            "seq": 12,
            "entity_type": "WorkerRun",
            "entity_id": "wr-queued",
            "action": "Create",
            "status": "Queued",
            "tenant": "default"
        }))
        .expect("event should parse");

        assert_eq!(event.entity_type, "WorkerRun");
        assert_eq!(event.entity_id, "wr-queued");
        assert_eq!(event.status, "Queued");
    }

    #[test]
    fn worker_event_status_accepts_status_aliases() {
        let event: EntityEvent = serde_json::from_value(json!({
            "entityType": "WorkerRun",
            "entityId": "wr-queued",
            "new_status": "Queued"
        }))
        .expect("event should parse");

        assert_eq!(event.entity_type, "WorkerRun");
        assert_eq!(event.entity_id, "wr-queued");
        assert_eq!(event.status, "Queued");
    }

    #[test]
    fn repo_sweep_task_is_detected_from_worker_prompt() {
        let task = "RepoGraphSnapshot: en-123\nWorkCycle: wc-456\nRequired loop:";

        assert_eq!(
            extract_repo_sweep_snapshot_id(task).as_deref(),
            Some("en-123")
        );
    }

    #[test]
    fn datadog_patrol_classifier_ignores_followup_implementation_prompts() {
        let patrol_task = "You are the local Codex Datadog Patrol agent for TemperPaw paw-patrol.\n\nPatrolRun: en-patrol\nPatrolKind: datadog_observability";
        let implementer_task = "You are the local Codex implementer for a Paw Patrol Datadog MCP observability finding.\n\nPatrolRun: en-patrol\nPatrol kind: datadog_observability\nFinding: OpenPaw monitor coverage is degraded by No Data states";

        assert_eq!(
            extract_datadog_patrol_run_id(patrol_task).as_deref(),
            Some("en-patrol")
        );
        assert_eq!(
            extract_datadog_patrol_run_id(implementer_task),
            None,
            "Datadog follow-up implementer work must run as normal Codex implementation, not as the patrol writeback collector"
        );
    }

    #[test]
    fn boot_poll_scans_newest_queue_window_not_old_stale_runs() {
        let source = include_str!("boot_watch.rs");

        assert!(
            source.contains("$orderby=Id desc&$top=50"),
            "boot poll should inspect newest queued entities first so old stale runs cannot starve newer claimable runs"
        );
    }

    #[test]
    fn event_stream_triggers_worker_queue_fallback_after_each_entity_event() {
        let source = include_str!("event_loop.rs");

        assert!(
            source.contains("claim_boot_queued_runs(client, config).await?"),
            "SSE handling should immediately rescan queued WorkerRuns after entity events so missed WorkerRun events cannot wait behind stale stream replay"
        );
    }

    #[test]
    fn event_stream_polls_queue_while_connection_stays_open() {
        let source = include_str!("event_loop.rs");

        assert!(
            source.contains("event_stream_queue_poll_interval()"),
            "an open SSE stream with heartbeats must still poll the queued work window"
        );
        assert!(
            source.contains("claim_event_stream_backlog(client, config).await?"),
            "periodic stream fallback should process WorkerRun, ReviewRun, and EvaluationRun backlogs"
        );
    }

    #[test]
    fn repo_health_patrol_parser_requires_agent_evidence_surfaces() {
        let output = r##"
REPO_HEALTH_PATROL_RESULT_JSON_BEGIN
{
  "summary_markdown": "# Agent-led repo health",
  "evidence_scope": [
    {"surface":"codebase_graph","query_or_command":"rg --files","result_summary":"graph inspected"},
    {"surface":"wasm_modules","query_or_command":"rg os-apps","result_summary":"wasm inspected"},
    {"surface":"specs_policies","query_or_command":"rg cedar ioa","result_summary":"specs inspected"},
    {"surface":"dependencies","query_or_command":"cargo metadata","result_summary":"dependencies inspected"},
    {"surface":"tests_proofs","query_or_command":"cargo test --no-run","result_summary":"tests inspected"},
    {"surface":"security_readability","query_or_command":"rg TODO HACK","result_summary":"readability inspected"}
  ],
  "quality_findings": [
    {
      "title": "Mixed-concern WASM module",
      "severity": "warn",
      "evidence": "os-apps/paw-agent/wasm/monty_repl/src/lib.rs mixes REPL, parsing, and orchestration.",
      "affected_paths": ["./os-apps/paw-agent/wasm/monty_repl/src/lib.rs"]
    }
  ],
  "security_findings": [
    {
      "title": "Broad Cedar policy needs review",
      "severity": "critical",
      "risk_lane": "l3",
      "evidence": "policy permits a broad shape.",
      "affected_paths": ["os-apps/demo/policies/demo.cedar"]
    }
  ],
  "summary": {
    "scanned_files": 120,
    "scanned_lines": 44000,
    "giant_modules": 1,
    "todo_hack_hits": 4,
    "duplicate_logic_candidates": 2,
    "broad_cedar_policies": 1,
    "dependency_risk_hits": 0,
    "rust_orchestration_hits": 1,
    "polling_loop_hits": 1,
    "missing_test_coverage_hits": 3
  },
  "residual_risks": ["human should approve L3"],
  "recommended_next_actions": ["split Monty REPL"]
}
REPO_HEALTH_PATROL_RESULT_JSON_END
"##;

        let parsed = parse_repo_health_agent_output(output).expect("parse agent output");

        assert_eq!(parsed.graph.quality_findings.len(), 1);
        assert_eq!(parsed.graph.security_findings.len(), 1);
        assert_eq!(parsed.graph.quality_findings[0].severity, "low");
        assert!(
            parsed.graph.quality_findings[0]
                .fingerprint
                .starts_with("quality:")
        );
        assert_eq!(parsed.graph.security_findings[0].severity, "high");
        assert_eq!(parsed.graph.security_findings[0].risk_lane, "L3");
        assert_eq!(
            parsed.graph.quality_findings[0].affected_paths,
            vec!["os-apps/paw-agent/wasm/monty_repl/src/lib.rs"]
        );
    }

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        env::temp_dir().join(format!(
            "paw-codex-worker-test-{}-{nanos}",
            std::process::id()
        ))
    }
}
