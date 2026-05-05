#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command as StdCommand;
    use std::time::{SystemTime, UNIX_EPOCH};

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
                "allowed_worker_id": "mac-mini-codex-1"
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
        };
        let unconfigured = WorkerRunState {
            id: "wr-2".to_string(),
            status: "Queued".to_string(),
            task: String::new(),
            worktree_path: String::new(),
            branch_name: String::new(),
            runner_kind: String::new(),
            allowed_worker_id: String::new(),
        };
        let cloud = WorkerRunState {
            id: "wr-3".to_string(),
            status: "Queued".to_string(),
            task: "Cloud overflow".to_string(),
            worktree_path: String::new(),
            branch_name: String::new(),
            runner_kind: "codex_cloud".to_string(),
            allowed_worker_id: "mac-mini-codex-1".to_string(),
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
        };
        let normal_worker_run = WorkerRunState {
            id: "wr-2".to_string(),
            status: "Done".to_string(),
            task: "Fix a Discord reply bug".to_string(),
            worktree_path: "/tmp/worktree".to_string(),
            branch_name: "codex/bugfix".to_string(),
            runner_kind: "local_codex".to_string(),
            allowed_worker_id: "mac-mini-codex-1".to_string(),
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
            "<key>WORKER_TOKEN</key>",
            "<string>secret&amp;token</string>",
            "<key>PAW_CODEX_ENABLE_EXECUTION</key>",
            "<string>1</string>",
            "<key>PAW_CODEX_POLL_ON_START</key>",
            "<string>0</string>",
            "<key>PAW_CODEX_DOCTOR_EXEC_SMOKE</key>",
            "<string>1</string>",
            "<key>PAW_CODEX_EVAL_COMMANDS</key>",
            "<string>cargo test -p temperpaw --test paw_patrol_foundation</string>",
        ] {
            assert!(plist.contains(needle), "plist should contain {needle}");
        }
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
    fn fake_codex_fixture_only_uses_reviewer_mode_for_reviewer_prompt() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/fake-codex.sh");
        let root = unique_temp_dir();
        fs::create_dir_all(&root).expect("temp dir");

        let implementation = StdCommand::new(&fixture)
            .arg("exec")
            .arg("Implement a task whose request text mentions an independent reviewer later.")
            .current_dir(&root)
            .output()
            .expect("run fake implementation");
        assert!(
            implementation.status.success(),
            "fake implementation should succeed"
        );
        assert!(
            root.join(".paw-fake-codex-implementation").is_file(),
            "implementation prompt should write the marker even if task text mentions a reviewer"
        );

        let review = StdCommand::new(&fixture)
            .arg("exec")
            .arg("You are the independent reviewer for a TemperPaw paw-patrol WorkerRun.")
            .current_dir(&root)
            .output()
            .expect("run fake reviewer");
        assert!(review.status.success(), "fake reviewer should succeed");
        let stdout = String::from_utf8_lossy(&review.stdout);
        assert!(
            stdout.contains("VERDICT: approve"),
            "reviewer prompt should emit an approval verdict: {stdout}"
        );

        fs::remove_dir_all(root).ok();
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
    fn repo_health_scan_emits_quality_and_security_findings() {
        let root = unique_temp_dir();
        fs::create_dir_all(root.join("src")).expect("src dir");
        fs::create_dir_all(root.join("policies")).expect("policy dir");

        let mut huge = String::new();
        for index in 0..901 {
            huge.push_str(&format!("fn generated_{index}() {{}}\n"));
        }
        fs::write(root.join("src/huge.rs"), huge).expect("huge source");
        fs::write(
            root.join("src/bandaid.rs"),
            "// TODO remove band-aid\n// HACK duplicated workaround\n",
        )
        .expect("bandaid source");
        fs::write(
            root.join("policies/broad.cedar"),
            "permit(principal, action, resource);",
        )
        .expect("policy");

        let graph = scan_repo_health(&root).expect("scan");

        assert!(
            graph.quality_findings.iter().any(|finding| {
                finding.title.contains("Giant module")
                    && finding.affected_paths.contains(&"src/huge.rs".to_string())
            }),
            "quality findings should include giant module evidence: {graph:?}"
        );
        assert!(
            graph
                .quality_findings
                .iter()
                .any(|finding| finding.title.contains("TODO/HACK")),
            "quality findings should include band-aid evidence: {graph:?}"
        );
        assert!(
            graph
                .security_findings
                .iter()
                .any(|finding| finding.title.contains("Broad Cedar")),
            "security findings should include broad Cedar evidence: {graph:?}"
        );

        fs::remove_dir_all(root).ok();
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
