use std::sync::Arc;
use tokio::io::{AsyncReadExt as TokioAsyncReadExt, AsyncWriteExt as TokioAsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

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

#[tokio::test]
async fn queued_evaluation_handler_fails_when_parent_work_cycle_is_terminal() {
    let scenario = TerminalEvaluationScenario {
        work_cycle_status: "Failed",
        review_status: None,
    };
    let (base_url, requests, server) = spawn_terminal_evaluation_server(scenario).await;
    let client = reqwest::Client::new();
    let config = terminal_evaluation_config(base_url);

    handle_queued_evaluation_run(&client, &config, "eval-stuck")
        .await
        .expect("terminal parent should fail queued evaluation");

    server.abort();
    let requests = requests.lock().await;
    assert!(
        requests.iter().any(|request| request.path.ends_with(
            "/tdata/EvaluationRuns('eval-stuck')/TemperPaw.Patrol.Claim"
        )),
        "handler should claim before failing so Cedar permits EvaluationRun.Fail: {requests:?}"
    );
    let fail_request = requests
        .iter()
        .find(|request| {
            request
                .path
                .ends_with("/tdata/EvaluationRuns('eval-stuck')/TemperPaw.Patrol.Fail")
        })
        .expect("handler should terminalize the queued evaluation");
    assert!(
        fail_request
            .body
            .contains("\"failure_classification\":\"parent_work_cycle_terminal\""),
        "Fail body should classify parent terminal cleanup: {}",
        fail_request.body
    );
}

#[tokio::test]
async fn queued_evaluation_handler_fails_when_review_terminal_without_execution_enabled() {
    let scenario = TerminalEvaluationScenario {
        work_cycle_status: "Reviewing",
        review_status: Some("ChangesRequested"),
    };
    let (base_url, requests, server) = spawn_terminal_evaluation_server(scenario).await;
    let client = reqwest::Client::new();
    let config = terminal_evaluation_config(base_url);

    handle_queued_evaluation_run(&client, &config, "eval-stuck")
        .await
        .expect("terminal review should fail queued evaluation");

    server.abort();
    let requests = requests.lock().await;
    let fail_request = requests
        .iter()
        .find(|request| {
            request
                .path
                .ends_with("/tdata/EvaluationRuns('eval-stuck')/TemperPaw.Patrol.Fail")
        })
        .expect("handler should terminalize the queued evaluation");
    assert!(
        fail_request
            .body
            .contains("\"failure_classification\":\"review_terminal_without_approval\""),
        "Fail body should classify terminal review cleanup: {}",
        fail_request.body
    );
    assert!(
        !requests.iter().any(|request| request.path.ends_with(
            "/tdata/ReviewRuns('rev-1')/TemperPaw.Patrol.StartReview"
        )),
        "execution-disabled worker should clean terminal residue without starting review: {requests:?}"
    );
}

#[test]
fn queued_evaluation_terminal_blocker_detects_dead_parent_or_review() {
    let failed_work_cycle = WorkCycleState {
        id: "wc-failed".to_string(),
        status: "Failed".to_string(),
        implementer_worker_run_id: "wr-1".to_string(),
        reviewer_run_id: "rev-1".to_string(),
        review_passed: false,
    };

    let blocker = queued_evaluation_terminal_blocker(&failed_work_cycle, None)
        .expect("terminal parent should block a queued evaluation");

    assert_eq!(
        blocker.failure_classification,
        "parent_work_cycle_terminal"
    );
    assert!(
        blocker
            .error_message
            .contains("WorkCycle wc-failed is Failed"),
        "blocker should identify the terminal parent: {}",
        blocker.error_message
    );

    let reviewing_work_cycle = WorkCycleState {
        id: "wc-reviewing".to_string(),
        status: "Reviewing".to_string(),
        implementer_worker_run_id: "wr-1".to_string(),
        reviewer_run_id: "rev-1".to_string(),
        review_passed: false,
    };
    let changes_requested = ReviewRunState {
        status: "ChangesRequested".to_string(),
        worker_run_id: "wr-1".to_string(),
        proof_packet_id: "proof-1".to_string(),
    };

    let blocker = queued_evaluation_terminal_blocker(&reviewing_work_cycle, Some(&changes_requested))
        .expect("changes-requested review should block the stale queued evaluation");

    assert_eq!(
        blocker.failure_classification,
        "review_terminal_without_approval"
    );
    assert!(
        blocker
            .error_message
            .contains("ReviewRun rev-1 is ChangesRequested"),
        "blocker should identify the terminal review: {}",
        blocker.error_message
    );
}

#[test]
fn queued_evaluation_keeps_waiting_for_review_states_that_can_still_pass() {
    let work_cycle = WorkCycleState {
        id: "wc-reviewing".to_string(),
        status: "Reviewing".to_string(),
        implementer_worker_run_id: "wr-1".to_string(),
        reviewer_run_id: "rev-1".to_string(),
        review_passed: false,
    };

    for status in ["Requested", "Claimed", "Reviewing", "Approved"] {
        let review = ReviewRunState {
            status: status.to_string(),
            worker_run_id: "wr-1".to_string(),
            proof_packet_id: "proof-1".to_string(),
        };

        assert!(
            queued_evaluation_terminal_blocker(&work_cycle, Some(&review)).is_none(),
            "ReviewRun status {status} can still lead to WorkCycle.review_passed"
        );
    }
}

#[derive(Clone, Copy)]
struct TerminalEvaluationScenario {
    work_cycle_status: &'static str,
    review_status: Option<&'static str>,
}

#[derive(Debug)]
struct RecordedRequest {
    path: String,
    body: String,
}

fn terminal_evaluation_config(temper_url: String) -> Config {
    let root = unique_temp_dir();
    Config {
        temper_url,
        tenant: "default".to_string(),
        worker_id: "mac-mini-codex-1".to_string(),
        worker_token: Some("secret".to_string()),
        workspace_root: root.clone(),
        repo_root: root,
        codex_bin: "codex".to_string(),
        max_concurrent_runs: 1,
        enable_execution: false,
        poll_on_start: true,
        codex_exec_smoke: false,
        codex_exec_timeout: Duration::from_secs(30),
    }
}

async fn spawn_terminal_evaluation_server(
    scenario: TerminalEvaluationScenario,
) -> (String, Arc<Mutex<Vec<RecordedRequest>>>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test HTTP server");
    let addr = listener.local_addr().expect("server addr");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&requests);
    let server = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let requests = Arc::clone(&recorded);
            tokio::spawn(async move {
                let Some(request) = read_test_http_request(&mut stream).await else {
                    return;
                };
                let body = terminal_evaluation_response_body(&request.path, scenario);
                requests.lock().await.push(request);
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });

    (format!("http://{addr}"), requests, server)
}

async fn read_test_http_request(stream: &mut tokio::net::TcpStream) -> Option<RecordedRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let header_end = loop {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(header_end) = find_header_end(&buffer) {
            break header_end;
        }
    };

    let header_text = String::from_utf8_lossy(&buffer[..header_end]);
    let first_line = header_text.lines().next().unwrap_or_default();
    let path = first_line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_string();
    let content_length = header_text
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);
    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let body = String::from_utf8_lossy(
        &buffer[body_start..buffer.len().min(body_start + content_length)],
    )
    .to_string();

    Some(RecordedRequest { path, body })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn terminal_evaluation_response_body(
    path: &str,
    scenario: TerminalEvaluationScenario,
) -> String {
    if path.contains("/tdata/EvaluationRuns('eval-stuck')/") {
        return "{}".to_string();
    }
    if path.ends_with("/tdata/EvaluationRuns('eval-stuck')") {
        return json!({
            "entity_id": "eval-stuck",
            "status": "Queued",
            "fields": {
                "work_cycle_id": "wc-1",
                "evaluator_id": "",
                "required_checks": "[\"cargo test\"]"
            }
        })
        .to_string();
    }
    if path.ends_with("/tdata/WorkCycles('wc-1')") {
        return json!({
            "entity_id": "wc-1",
            "status": scenario.work_cycle_status,
            "fields": {
                "implementer_worker_run_id": "wr-1",
                "reviewer_run_id": "rev-1",
                "evaluation_run_id": "eval-stuck",
                "review_passed": false
            }
        })
        .to_string();
    }
    if path.ends_with("/tdata/WorkerRuns('wr-1')") {
        return json!({
            "entity_id": "wr-1",
            "status": "Done",
            "fields": {
                "task": "Fix a normal code-change task",
                "worktree_path": "/tmp/worktree",
                "branch_name": "codex/test",
                "runner_kind": "local_codex",
                "allowed_worker_id": "mac-mini-codex-1",
                "worker_id": "mac-mini-codex-1",
                "provider_id": "local-codex",
                "required_capabilities": "local_codex,repo_write,evaluation"
            }
        })
        .to_string();
    }
    if path.ends_with("/tdata/ReviewRuns('rev-1')") {
        return json!({
            "entity_id": "rev-1",
            "status": scenario.review_status.unwrap_or("Requested"),
            "fields": {
                "worker_run_id": "wr-1",
                "proof_packet_id": "proof-1"
            }
        })
        .to_string();
    }
    panic!("unexpected test HTTP path: {path}");
}
