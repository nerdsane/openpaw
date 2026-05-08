#[tokio::test]
async fn evaluation_commands_classify_local_timeouts() {
    let root = unique_temp_dir();
    fs::create_dir_all(&root).expect("temp dir");
    let config = Config {
        temper_url: "http://127.0.0.1:3497".to_string(),
        tenant: "default".to_string(),
        worker_id: "mac-mini-codex-1".to_string(),
        worker_token: Some("secret".to_string()),
        workspace_root: root.clone(),
        repo_root: root.clone(),
        codex_bin: "codex".to_string(),
        max_concurrent_runs: 1,
        enable_execution: true,
        poll_on_start: true,
        codex_exec_smoke: false,
        codex_exec_timeout: Duration::from_millis(100),
    };
    let worker_run = WorkerRunState {
        id: "wr-eval-timeout".to_string(),
        status: "Done".to_string(),
        task: "Evaluate a code change".to_string(),
        worktree_path: root.display().to_string(),
        branch_name: "codex/eval-timeout".to_string(),
        runner_kind: "local_codex".to_string(),
        allowed_worker_id: "mac-mini-codex-1".to_string(),
        worker_id: "mac-mini-codex-1".to_string(),
        provider_id: "local-codex".to_string(),
        required_capabilities: "local_codex,repo_write,evaluation".to_string(),
    };

    let outcome = run_evaluation_command_list(&config, &worker_run, vec!["sleep 2".to_string()])
        .await
        .expect("timeout should produce a classified evaluation outcome");

    assert!(!outcome.passed);
    assert_eq!(outcome.failure_classification, "evaluator_timeout");
    assert!(
        outcome.error_message.contains("evaluator_timeout"),
        "timeout error should include the failure class: {}",
        outcome.error_message
    );
    let evidence: Value =
        serde_json::from_str(&outcome.results_json).expect("results_json should parse");
    assert_eq!(evidence["failure_classification"], "evaluator_timeout");
    assert_eq!(
        evidence["commands"][0]["failure_classification"],
        "evaluator_timeout"
    );
    assert_eq!(evidence["commands"][0]["timed_out"], true);
    assert_eq!(evidence["commands"][0]["timeout_ms"], 100);

    fs::remove_dir_all(root).ok();
}
