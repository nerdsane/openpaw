#[tokio::test]
async fn run_codex_creates_ready_github_pr_when_required_and_changes_exist() {
    let _guard = ENV_LOCK.lock().await;
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/fake-codex.sh");
    let root = unique_temp_dir();
    let remote = unique_temp_dir();
    let fake_bin = unique_temp_dir();
    fs::create_dir_all(&root).expect("repo root");
    fs::create_dir_all(&fake_bin).expect("fake bin");

    run_raw_git_for_test(&["init", "--bare", remote.to_str().expect("remote path")]);
    run_git_for_test(&root, &["init"]);
    run_git_for_test(&root, &["checkout", "-B", "main"]);
    run_git_for_test(&root, &["config", "user.name", "test worker"]);
    run_git_for_test(&root, &["config", "user.email", "worker@example.test"]);
    fs::write(root.join("README.md"), "worker pr test\n").expect("readme");
    run_git_for_test(&root, &["add", "README.md"]);
    run_git_for_test(&root, &["commit", "-m", "initial"]);
    run_git_for_test(
        &root,
        &["remote", "add", "origin", remote.to_str().expect("remote path")],
    );
    run_git_for_test(&root, &["push", "-u", "origin", "main"]);
    run_git_for_test(&root, &["checkout", "-B", "codex/worker-pr-proof"]);

    let fake_gh_log = fake_bin.join("fake-gh.log");
    let fake_gh = fake_bin.join("gh");
    fs::write(
        &fake_gh,
        r#"#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$PAW_FAKE_GH_LOG"
if [ "${1:-}" = "pr" ] && [ "${2:-}" = "view" ]; then
  exit 1
fi
if [ "${1:-}" = "pr" ] && [ "${2:-}" = "create" ]; then
  echo "https://github.com/nerdsane/temperpaw/pull/999"
  exit 0
fi
echo "unexpected gh invocation: $*" >&2
exit 2
"#,
    )
    .expect("fake gh");
    make_executable(&fake_gh);

    let old_path = env::var("PATH").unwrap_or_default();
    let _path = EnvOverride::set(
        "PATH",
        OsString::from(format!("{}:{old_path}", fake_bin.display())),
    );
    let _mode = EnvOverride::set("PAW_CODEX_PR_MODE", OsString::from("required"));
    let _base = EnvOverride::set("PAW_CODEX_PR_BASE_BRANCH", OsString::from("main"));
    let _gh_log = EnvOverride::set("PAW_FAKE_GH_LOG", fake_gh_log.as_os_str().to_os_string());

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
        codex_exec_timeout: Duration::from_secs(30),
    };
    let worker_run = WorkerRunState {
        id: "wr-pr-proof".to_string(),
        status: "Running".to_string(),
        task: "Create a small proof file so the worker can produce a ready PR.".to_string(),
        worktree_path: root.display().to_string(),
        branch_name: "codex/worker-pr-proof".to_string(),
        runner_kind: "local_codex".to_string(),
        allowed_worker_id: "mac-mini-codex-1".to_string(),
        worker_id: "mac-mini-codex-1".to_string(),
        provider_id: "local-codex".to_string(),
        required_capabilities: "local_codex,repo_write".to_string(),
    };

    let summary = run_codex(&config, &worker_run)
        .await
        .expect("run_codex should create a PR");

    assert!(summary.contains("Pull request:"));
    assert!(summary.contains("Mode: required"));
    assert!(summary.contains("URL: https://github.com/nerdsane/temperpaw/pull/999"));
    assert!(summary.contains("Commit:"));
    assert_eq!(
        run_git_capture_for_test(&root, &["status", "--short"]),
        "",
        "worker should leave the PR branch committed and clean"
    );
    let gh_log = fs::read_to_string(&fake_gh_log).expect("fake gh log");
    assert!(gh_log.contains("pr create"), "fake gh log: {gh_log}");
    assert!(gh_log.contains("--base main"), "fake gh log: {gh_log}");
    assert!(
        gh_log.contains("--head codex/worker-pr-proof"),
        "fake gh log: {gh_log}"
    );
    assert!(
        !run_git_capture_for_test(
            &root,
            &["ls-remote", "--heads", "origin", "codex/worker-pr-proof"]
        )
        .is_empty(),
        "worker should push the implementation branch"
    );

    fs::remove_dir_all(root).ok();
    fs::remove_dir_all(remote).ok();
    fs::remove_dir_all(fake_bin).ok();
}

#[tokio::test]
async fn run_codex_reuses_existing_github_pr_when_create_reports_duplicate() {
    let _guard = ENV_LOCK.lock().await;
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/fake-codex.sh");
    let root = unique_temp_dir();
    let remote = unique_temp_dir();
    let fake_bin = unique_temp_dir();
    fs::create_dir_all(&root).expect("repo root");
    fs::create_dir_all(&fake_bin).expect("fake bin");

    run_raw_git_for_test(&["init", "--bare", remote.to_str().expect("remote path")]);
    run_git_for_test(&root, &["init"]);
    run_git_for_test(&root, &["checkout", "-B", "main"]);
    run_git_for_test(&root, &["config", "user.name", "test worker"]);
    run_git_for_test(&root, &["config", "user.email", "worker@example.test"]);
    fs::write(root.join("README.md"), "worker pr test\n").expect("readme");
    run_git_for_test(&root, &["add", "README.md"]);
    run_git_for_test(&root, &["commit", "-m", "initial"]);
    run_git_for_test(
        &root,
        &["remote", "add", "origin", remote.to_str().expect("remote path")],
    );
    run_git_for_test(&root, &["push", "-u", "origin", "main"]);
    run_git_for_test(&root, &["checkout", "-B", "codex/worker-pr-rework"]);

    let fake_gh_log = fake_bin.join("fake-gh.log");
    let fake_gh = fake_bin.join("gh");
    fs::write(
        &fake_gh,
        r#"#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$PAW_FAKE_GH_LOG"
if [ "${1:-}" = "pr" ] && [ "${2:-}" = "view" ]; then
  exit 1
fi
if [ "${1:-}" = "pr" ] && [ "${2:-}" = "create" ]; then
  cat >&2 <<'EOF'
a pull request for branch "codex/worker-pr-rework" into branch "main" already exists:
https://github.com/nerdsane/temperpaw/pull/1001
EOF
  exit 1
fi
echo "unexpected gh invocation: $*" >&2
exit 2
"#,
    )
    .expect("fake gh");
    make_executable(&fake_gh);

    let old_path = env::var("PATH").unwrap_or_default();
    let _path = EnvOverride::set(
        "PATH",
        OsString::from(format!("{}:{old_path}", fake_bin.display())),
    );
    let _mode = EnvOverride::set("PAW_CODEX_PR_MODE", OsString::from("required"));
    let _base = EnvOverride::set("PAW_CODEX_PR_BASE_BRANCH", OsString::from("main"));
    let _gh_log = EnvOverride::set("PAW_FAKE_GH_LOG", fake_gh_log.as_os_str().to_os_string());

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
        codex_exec_timeout: Duration::from_secs(30),
    };
    let worker_run = WorkerRunState {
        id: "wr-pr-rework".to_string(),
        status: "Running".to_string(),
        task: "Update a branch that already has a PR.".to_string(),
        worktree_path: root.display().to_string(),
        branch_name: "codex/worker-pr-rework".to_string(),
        runner_kind: "local_codex".to_string(),
        allowed_worker_id: "mac-mini-codex-1".to_string(),
        worker_id: "mac-mini-codex-1".to_string(),
        provider_id: "local-codex".to_string(),
        required_capabilities: "local_codex,repo_write".to_string(),
    };

    let summary = run_codex(&config, &worker_run)
        .await
        .expect("run_codex should reuse the existing PR");

    assert!(summary.contains("Pull request:"));
    assert!(summary.contains("URL: https://github.com/nerdsane/temperpaw/pull/1001"));
    assert!(summary.contains("Ready GitHub PR created or reused"));
    assert_eq!(
        run_git_capture_for_test(&root, &["status", "--short"]),
        "",
        "worker should leave the existing PR branch committed and clean"
    );
    let gh_log = fs::read_to_string(&fake_gh_log).expect("fake gh log");
    assert!(gh_log.contains("pr view"), "fake gh log: {gh_log}");
    assert!(gh_log.contains("pr create"), "fake gh log: {gh_log}");

    fs::remove_dir_all(root).ok();
    fs::remove_dir_all(remote).ok();
    fs::remove_dir_all(fake_bin).ok();
}
