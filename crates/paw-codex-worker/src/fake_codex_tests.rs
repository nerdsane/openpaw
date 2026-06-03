use std::process::Command as StdCommand;

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

    let implementation_with_config = StdCommand::new(&fixture)
        .arg("exec")
        .arg("-c")
        .arg("mcp_servers.datadog.url=\"https://mcp.datadoghq.test/mcp\"")
        .arg("--cd")
        .arg(&root)
        .arg("Implement a task with a Codex config override.")
        .output()
        .expect("run fake implementation with config override");
    assert!(
        implementation_with_config.status.success(),
        "fake implementation should accept Codex -c config overrides"
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

    let repo_health_review = StdCommand::new(&fixture)
        .arg("exec")
        .arg(
            "You are the independent repo-health Patrol scan reviewer for TemperPaw paw-patrol.\n\nTask:\nYou are the local Codex repo-health Patrol agent for TemperPaw paw-patrol.",
        )
        .current_dir(&root)
        .output()
        .expect("run fake repo-health reviewer");
    assert!(
        repo_health_review.status.success(),
        "fake repo-health reviewer should succeed"
    );
    let repo_health_stdout = String::from_utf8_lossy(&repo_health_review.stdout);
    assert!(
        repo_health_stdout.contains("VERDICT: approve"),
        "repo-health reviewer prompt should emit an approval verdict, not another scan result: {repo_health_stdout}"
    );

    let datadog_patrol = StdCommand::new(&fixture)
        .arg("exec")
        .arg(
            "You are the Datadog MCP Risk Patrol agent for TemperPaw and Temper.\n\nRequired Datadog MCP investigation surfaces:",
        )
        .current_dir(&root)
        .output()
        .expect("run fake Datadog Patrol");
    assert!(
        datadog_patrol.status.success(),
        "fake Datadog Patrol should succeed"
    );
    let datadog_stdout = String::from_utf8_lossy(&datadog_patrol.stdout);
    assert!(
        datadog_stdout.contains("DATADOG_PATROL_RESULT_JSON_BEGIN"),
        "Datadog Risk Patrol prompt should emit the MCP result envelope: {datadog_stdout}"
    );

    fs::remove_dir_all(root).ok();
}
