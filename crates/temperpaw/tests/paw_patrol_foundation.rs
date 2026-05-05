use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use temper_authz::{AuthzEngine, SecurityContext};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path.as_ref())
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.as_ref().display()))
}

fn worker_source_files(root: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(root.join("crates/paw-codex-worker/src"))
        .expect("read paw-codex-worker src")
        .map(|entry| entry.expect("worker source entry").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn read_worker_sources(root: &Path) -> String {
    worker_source_files(root)
        .into_iter()
        .map(read)
        .collect::<Vec<_>>()
        .join("\n")
}

fn agent_context(id: &str, agent_type: &str) -> SecurityContext {
    SecurityContext::from_headers(&[
        ("X-Temper-Principal-Id".to_string(), id.to_string()),
        ("X-Temper-Principal-Kind".to_string(), "agent".to_string()),
        ("X-Temper-Agent-Type".to_string(), agent_type.to_string()),
    ])
}

fn resource_attrs(pairs: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect()
}

#[test]
fn paw_patrol_owns_the_dark_factory_entities_without_extra_factory_apps() {
    let root = repo_root();
    let patrol = root.join("os-apps/paw-patrol");

    assert!(patrol.is_dir(), "paw-patrol app should exist");
    assert!(
        !root.join("os-apps/paw-factory").exists(),
        "factory should be a paw-patrol capability, not a separate app"
    );
    assert!(
        !root.join("os-apps/paw-quality").exists(),
        "quality should be a paw-patrol capability, not a separate app"
    );

    let manifest = read(patrol.join("app.toml"));
    assert!(manifest.contains("name = \"paw-patrol\""));
    assert!(manifest.contains("paw-ingest"));
    assert!(manifest.contains("paw-pm"));
    assert!(manifest.contains("paw-agent"));

    let app_doc = read(patrol.join("APP.md"));
    for needle in [
        "PatrolRequest",
        "Signal",
        "FactoryCase",
        "WorkCycle",
        "WorkerRun",
        "ReviewRun",
        "EvaluationRun",
        "ProofPacket",
        "RiskRule",
        "RepoGraphSnapshot",
        "QualityFinding",
        "SecurityFinding",
        "DailyBrief",
        "PatrolSchedule",
        "paw-pm Issues",
        "Mac mini",
        "resource-bound ownership",
    ] {
        assert!(app_doc.contains(needle), "APP.md should mention {needle}");
    }

    for spec in [
        "patrol_request.ioa.toml",
        "signal.ioa.toml",
        "factory_case.ioa.toml",
        "work_cycle.ioa.toml",
        "worker_run.ioa.toml",
        "review_run.ioa.toml",
        "evaluation_run.ioa.toml",
        "proof_packet.ioa.toml",
        "risk_rule.ioa.toml",
        "repo_graph_snapshot.ioa.toml",
        "quality_finding.ioa.toml",
        "security_finding.ioa.toml",
        "daily_brief.ioa.toml",
        "patrol_schedule.ioa.toml",
    ] {
        assert!(
            patrol.join("specs").join(spec).is_file(),
            "paw-patrol should define specs/{spec}"
        );
    }

    let csdl = read(patrol.join("specs/model.csdl.xml"));
    for entity in [
        "PatrolRequest",
        "Signal",
        "FactoryCase",
        "WorkCycle",
        "WorkerRun",
        "ReviewRun",
        "EvaluationRun",
        "ProofPacket",
        "RiskRule",
        "RepoGraphSnapshot",
        "QualityFinding",
        "SecurityFinding",
        "DailyBrief",
        "PatrolSchedule",
    ] {
        assert!(
            csdl.contains(&format!("<EntityType Name=\"{entity}\">")),
            "CSDL should expose {entity}"
        );
    }
}

#[test]
fn worker_run_encodes_local_codex_claiming_and_manual_cloud_overflow() {
    let root = repo_root();
    let spec = read(root.join("os-apps/paw-patrol/specs/worker_run.ioa.toml"));

    for needle in [
        "name = \"WorkerRun\"",
        "states = [\"Queued\", \"Claimed\", \"Running\", \"WaitingForCloudApproval\", \"CloudApproved\", \"Done\", \"Failed\", \"TimedOut\"]",
        "name = \"worker_id\"",
        "name = \"runner_kind\"",
        "name = \"work_cycle_id\"",
        "name = \"risk_lane\"",
        "name = \"proof_packet_id\"",
        "name = \"Claim\"",
        "name = \"StartLocal\"",
        "name = \"RequestCloudOverflow\"",
        "name = \"ApproveCloudOverflow\"",
        "name = \"DispatchCloud\"",
        "name = \"ReportDone\"",
        "name = \"ReportFailed\"",
        "on_timeout = \"Timeout\"",
    ] {
        assert!(
            spec.contains(needle),
            "WorkerRun spec should contain {needle}"
        );
    }
}

#[test]
fn paw_patrol_dark_factory_architecture_is_recorded_in_app_adr() {
    let root = repo_root();
    let adr = root
        .join("os-apps/paw-patrol/adrs")
        .join("0001-patrol-controlled-dark-factory.md");

    assert!(
        adr.is_file(),
        "material paw-patrol architecture changes should be recorded in an app-scoped ADR"
    );

    let text = read(adr);
    for needle in [
        "Patrol-Controlled Dark Factory",
        "Status: Accepted",
        "Temper-native",
        "PatrolRequest",
        "Signal",
        "FactoryCase",
        "WorkCycle",
        "WorkerRun",
        "ReviewRun",
        "EvaluationRun",
        "ProofPacket",
        "RiskRule",
        "RepoGraphSnapshot",
        "QualityFinding",
        "SecurityFinding",
        "DailyBrief",
        "paw-pm",
        "Mac mini",
        "Cedar",
        "principal.id == resource.worker_id",
        "principal.id == resource.reviewer_id",
        "human-gated",
    ] {
        assert!(
            text.contains(needle),
            "paw-patrol Dark Factory ADR should contain {needle}"
        );
    }
}

#[test]
fn paw_patrol_docs_explain_worker_scripts_tests_and_wasms() {
    let root = repo_root();
    let app_doc = read(root.join("os-apps/paw-patrol/APP.md"));
    for needle in [
        "## WASM Modules",
        "patrol_request_router",
        "signal_router",
        "repo_sweep_lifecycle",
        "worker_run_lifecycle",
        "review_gate_lifecycle",
        "finding_lifecycle",
        "patrol_schedule_lifecycle",
        "daily_brief_lifecycle",
    ] {
        assert!(app_doc.contains(needle), "APP.md should document {needle}");
    }

    let worker_readme = read(root.join("crates/paw-codex-worker/README.md"));
    for needle in [
        "## Worker Responsibilities",
        "## Scripts Versus Rust Tests",
        "not one-off migration scripts",
        "acceptance and operations harnesses",
        "deterministic-smoke.sh",
        "webhook-intake-smoke.sh",
        "repo-sweep-brief-smoke.sh",
        "production-preflight.sh",
        "production-readiness.sh",
        "production-observe-only.sh",
        "paw-patrol-acceptance.sh",
        "Rust tests",
    ] {
        assert!(
            worker_readme.contains(needle),
            "paw-codex-worker README should explain {needle}"
        );
    }
}

#[test]
fn paw_patrol_docs_explain_schedule_boundary_and_cleanup_status() {
    let root = repo_root();
    let app_doc = read(root.join("os-apps/paw-patrol/APP.md"));
    for needle in [
        "## PatrolSchedule And CronJob",
        "PatrolSchedule intentionally does not reuse the paw-agent CronJob entity",
        "Both entities use Temper's schedule_at timer effect",
        "CronJob is for scheduled agent Session creation",
        "PatrolSchedule is for scheduled Patrol maintenance",
        "RepoGraphSnapshot and DailyBrief",
        "## Quality Cleanup Status",
        "Detection is not cleanup",
        "giant WASM modules remain work to be done",
        "Monty REPL",
        "provider_caller",
        "context_preparer",
        "route_message",
        "QualityFinding",
        "WorkCycle",
    ] {
        assert!(
            app_doc.contains(needle),
            "APP.md should explain schedule boundary and cleanup status: {needle}"
        );
    }

    let adr = read(
        root.join("os-apps/paw-patrol/adrs")
            .join("0001-patrol-controlled-dark-factory.md"),
    );
    for needle in [
        "PatrolSchedule intentionally remains a Patrol entity",
        "CronJob remains the paw-agent scheduled Session entity",
        "share the platform schedule_at effect",
        "RepoGraphSnapshot",
        "DailyBrief",
        "Detection is not the same as cleanup",
    ] {
        assert!(
            adr.contains(needle),
            "Dark Factory ADR should explain schedule boundary and cleanup status: {needle}"
        );
    }
}

#[test]
fn worker_claims_are_bound_to_the_configured_local_worker() {
    let root = repo_root();
    let spec = read(root.join("os-apps/paw-patrol/specs/worker_run.ioa.toml"));
    let csdl = read(root.join("os-apps/paw-patrol/specs/model.csdl.xml"));
    let policy = read(root.join("os-apps/paw-patrol/policies/patrol.cedar"));

    for needle in [
        "name = \"allowed_worker_id\"",
        "params = [\"work_cycle_id\", \"factory_case_id\", \"risk_lane\", \"task\", \"branch_name\", \"worktree_path\", \"runner_kind\", \"allowed_worker_id\"]",
    ] {
        assert!(
            spec.contains(needle),
            "WorkerRun should encode the allowed local worker identity: {needle}"
        );
    }

    assert!(
        csdl.contains("<Property Name=\"AllowedWorkerId\" Type=\"Edm.String\"/>"),
        "CSDL should expose WorkerRun.AllowedWorkerId for Cedar ABAC and human audit"
    );

    for needle in [
        "principal.id == resource.AllowedWorkerId",
        "principal.id == resource.allowed_worker_id",
        "Action::\"Claim\"",
    ] {
        assert!(
            policy.contains(needle),
            "patrol.cedar should bind WorkerRun.Claim to the configured worker: {needle}"
        );
    }

    for module in [
        "patrol_request_router",
        "signal_router",
        "finding_lifecycle",
        "repo_sweep_lifecycle",
        "work_cycle_lifecycle",
    ] {
        let source = read(root.join(format!("os-apps/paw-patrol/wasm/{module}/src/lib.rs")));
        assert!(
            source.contains("configured_local_worker_id"),
            "{module} should configure WorkerRuns with the registered local worker id"
        );
        assert!(
            source.contains("\"allowed_worker_id\":"),
            "{module} should pass allowed_worker_id to WorkerRun.Configure"
        );
    }

    for spec in [
        "patrol_request.ioa.toml",
        "signal.ioa.toml",
        "quality_finding.ioa.toml",
        "security_finding.ioa.toml",
        "repo_graph_snapshot.ioa.toml",
        "work_cycle.ioa.toml",
    ] {
        let source = read(root.join("os-apps/paw-patrol/specs").join(spec));
        assert!(
            source.contains("local_codex_worker_id = \"{secret:local_codex_worker_id}\""),
            "{spec} should pass the registered local Codex worker id into its WASM config"
        );
    }
}

#[test]
fn risk_rules_set_a_floor_that_agents_cannot_silently_lower() {
    let root = repo_root();
    let risk_rule = read(root.join("os-apps/paw-patrol/specs/risk_rule.ioa.toml"));
    let factory_case = read(root.join("os-apps/paw-patrol/specs/factory_case.ioa.toml"));

    for needle in [
        "name = \"RiskRule\"",
        "name = \"minimum_risk_lane\"",
        "name = \"evidence_selector\"",
        "name = \"required_checks\"",
        "name = \"Define\"",
        "name = \"Activate\"",
        "name = \"Archive\"",
    ] {
        assert!(
            risk_rule.contains(needle),
            "RiskRule should contain {needle}"
        );
    }

    for needle in [
        "name = \"minimum_risk_lane\"",
        "name = \"risk_floor_source\"",
        "name = \"SetRiskFloor\"",
        "name = \"RaiseRisk\"",
        "name = \"ApproveRiskOverride\"",
    ] {
        assert!(
            factory_case.contains(needle),
            "FactoryCase should contain {needle}"
        );
    }

    assert!(
        !factory_case.contains("LowerRisk"),
        "agents should not have a silent LowerRisk action"
    );
}

#[test]
fn local_codex_worker_is_a_real_daemon_scaffold() {
    let root = repo_root();

    let workspace = read(root.join("Cargo.toml"));
    assert!(
        workspace.contains("\"crates/paw-codex-worker\""),
        "paw-codex-worker should be a workspace member"
    );

    let manifest = read(root.join("crates/paw-codex-worker/Cargo.toml"));
    assert!(manifest.contains("name = \"paw-codex-worker\""));

    let worker_src = read_worker_sources(&root);
    for needle in [
        "TEMPER_URL",
        "TEMPER_TENANT",
        "WORKER_ID",
        "REPO_ROOT",
        "/tdata/$events",
        "WorkerRun.Claim",
        "WorkerRun.ReportDone",
        "RepoGraphSnapshot",
        "ScanComplete",
        "scan_repo_health",
        "git",
        "worktree",
        "doctor",
        "check_odata",
        "check_event_stream",
        "check_codex_binary",
        "check_codex_exec_smoke",
        "--skip-git-repo-check",
        "LaunchdPlist",
        "render_launchd_plist",
        "timeout(Duration::from_secs(10), watch_events",
        "Temper event stream poll window elapsed; using OData fallback",
        "x-temper-principal-kind",
        "codex",
    ] {
        assert!(
            worker_src.contains(needle),
            "worker source should contain {needle}"
        );
    }

    let launchd =
        read(root.join("crates/paw-codex-worker/launchd/com.temperpaw.paw-codex-worker.plist"));
    assert!(launchd.contains("com.temperpaw.paw-codex-worker"));
    assert!(launchd.contains("paw-codex-worker"));
    for needle in [
        "WORKER_TOKEN",
        "REPO_ROOT",
        "CODEX_BIN",
        "PAW_CODEX_DOCTOR_EXEC_SMOKE",
        "PAW_CODEX_POLL_ON_START",
        "RUST_LOG",
    ] {
        assert!(
            launchd.contains(needle),
            "launchd worker template should include {needle}"
        );
    }

    let readme = read(root.join("crates/paw-codex-worker/README.md"));
    for needle in [
        "launchctl bootstrap",
        "PAW_CODEX_ENABLE_EXECUTION",
        "PAW_CODEX_DOCTOR_EXEC_SMOKE",
        "RepoGraphSnapshot.StartScan",
        "ChatGPT/Codex auth",
        "paw-codex-worker doctor",
        "launchd-plist",
        "proof bundle",
        "PROOF_DIR",
    ] {
        assert!(
            readme.contains(needle),
            "worker README should document {needle}"
        );
    }
}

#[test]
fn paw_codex_worker_sources_stay_under_giant_module_budget() {
    let root = repo_root();

    for path in worker_source_files(&root) {
        let source = read(&path);
        let line_count = source.lines().count();
        assert!(
            line_count < 900,
            "{} has {line_count} lines; paw-codex-worker source files should stay below the repo-health giant-module threshold",
            path.display()
        );
    }
}

#[test]
fn local_worker_can_review_and_evaluate_repo_sweep_runs() {
    let root = repo_root();
    let worker_src = read_worker_sources(&root);

    for needle in [
        "ReviewRun.Claim",
        "ReviewRun.Approve",
        "EvaluationRun.Start",
        "EvaluationRun.Pass",
        "handle_requested_review_run",
        "handle_queued_evaluation_run",
        "claim_boot_requested_review_runs",
        "claim_boot_queued_evaluation_runs",
        "worker_run_is_repo_sweep",
        "run_codex_review",
        "parse_codex_review_verdict",
        "run_code_change_evaluation",
        "EvaluationRun.Claim",
        "PAW_CODEX_EVAL_COMMANDS",
    ] {
        assert!(
            worker_src.contains(needle),
            "local worker should include repo-sweep review/evaluation support: {needle}"
        );
    }
}

#[test]
fn patrol_cedar_human_gate_approvals_are_not_available_to_system_agents() {
    let root = repo_root();
    let policy = read(root.join("os-apps/paw-patrol/policies/patrol.cedar"));
    let engine = AuthzEngine::new(&policy).expect("patrol.cedar should parse");
    let attrs = resource_attrs(&[
        ("id", serde_json::json!("wc-approval")),
        ("risk_lane", serde_json::json!("L3")),
    ]);

    let system_agent = agent_context("patrol-wasm-system", "system");
    for action in ["ApproveHumanStart", "ApproveHumanCompletion"] {
        let decision = engine.authorize(&system_agent, action, "WorkCycle", &attrs);
        assert!(
            !decision.is_allowed(),
            "system agent must not be able to dispatch human gate {action}: {decision:?}"
        );
    }

    let human = agent_context("sesh", "human");
    for action in ["ApproveHumanStart", "ApproveHumanCompletion"] {
        let decision = engine.authorize(&human, action, "WorkCycle", &attrs);
        assert!(
            decision.is_allowed(),
            "human agent should be able to dispatch human gate {action}: {decision:?}"
        );
    }
}

#[test]
fn patrol_cedar_binds_evaluation_pass_fail_to_claimed_evaluator() {
    let root = repo_root();
    let policy = read(root.join("os-apps/paw-patrol/policies/patrol.cedar"));
    let engine = AuthzEngine::new(&policy).expect("patrol.cedar should parse");
    let attrs = resource_attrs(&[
        ("id", serde_json::json!("eval-1")),
        ("evaluator_id", serde_json::json!("mac-mini-codex-prod")),
    ]);

    let unrelated_agent = agent_context("generic-agent", "agent");
    for action in ["Start", "Pass", "Fail"] {
        let decision = engine.authorize(&unrelated_agent, action, "EvaluationRun", &attrs);
        assert!(
            !decision.is_allowed(),
            "unclaimed generic agent must not be able to dispatch EvaluationRun.{action}: {decision:?}"
        );
    }

    let wrong_worker = agent_context("other-worker", "worker");
    for action in ["Start", "Pass", "Fail"] {
        let decision = engine.authorize(&wrong_worker, action, "EvaluationRun", &attrs);
        assert!(
            !decision.is_allowed(),
            "wrong worker must not be able to dispatch EvaluationRun.{action}: {decision:?}"
        );
    }

    let claimed_worker = agent_context("mac-mini-codex-prod", "worker");
    for action in ["Start", "Pass", "Fail"] {
        let decision = engine.authorize(&claimed_worker, action, "EvaluationRun", &attrs);
        assert!(
            decision.is_allowed(),
            "claimed evaluator should be able to dispatch EvaluationRun.{action}: {decision:?}"
        );
    }
}

#[test]
fn deterministic_smoke_exports_a_visual_proof_bundle() {
    let root = repo_root();
    let script = read(root.join("crates/paw-codex-worker/scripts/deterministic-smoke.sh"));

    for needle in [
        "PROOF_DIR",
        "summary.json",
        "proof.md",
        "proof.svg",
        "visual_summary_url",
        "state_diagram_mermaid",
        "changed_files_map",
        "reviewer_verdict",
        "residual_risks",
        "## OData Links",
        "## Trace And Log Evidence",
        "os-apps/paw-patrol/wasm/build.sh",
        "proof bundle:",
    ] {
        assert!(
            script.contains(needle),
            "deterministic smoke should export human-reviewable proof evidence: {needle}"
        );
    }
}

#[test]
fn live_smoke_scripts_choose_non_colliding_odata_and_webhook_ports() {
    let root = repo_root();

    for script_name in [
        "deterministic-smoke.sh",
        "webhook-intake-smoke.sh",
        "repo-sweep-brief-smoke.sh",
        "production-readiness-smoke.sh",
        "production-observe-only-smoke.sh",
    ] {
        let script = read(
            root.join("crates/paw-codex-worker/scripts")
                .join(script_name),
        );
        for needle in [
            "pick_available_port",
            "port_is_free",
            "candidate + 12",
            "implicit webhook trigger port",
        ] {
            assert!(
                script.contains(needle),
                "{script_name} should avoid OData/webhook port collisions with {needle}"
            );
        }
    }
}

#[test]
fn repo_sweep_brief_smoke_exports_visual_proof_bundle() {
    let root = repo_root();
    let script = read(root.join("crates/paw-codex-worker/scripts/repo-sweep-brief-smoke.sh"));

    for needle in [
        "PatrolSchedules",
        "patrol-default-daily-maintenance",
        "patrol-schedule.json",
        "default_schedule",
        "RepoGraphSnapshots",
        "TemperPaw.Patrol.StartScan",
        "DailyBriefs",
        "TemperPaw.Patrol.Start",
        "repo-graph.json",
        "daily-brief.svg",
        "proof.svg",
        "## Daily Brief",
        "## OData Links",
        "## Trace And Log Evidence",
        "proof bundle:",
    ] {
        assert!(
            script.contains(needle),
            "repo sweep/brief smoke should export maintenance proof evidence: {needle}"
        );
    }
}

#[test]
fn webhook_intake_smoke_exercises_the_trigger_boundary() {
    let root = repo_root();
    let script = read(root.join("crates/paw-codex-worker/scripts/webhook-intake-smoke.sh"));

    for needle in [
        "WEBHOOK_URL",
        "/triggers/webhook/${route_key}",
        "WebhookEvents",
        "TemperPaw.Ingest.Received",
        "TemperPaw.Ingest.Register",
        "TemperPaw.Patrol.Submit",
        "TemperPaw.Patrol.Ingest",
        "PatrolRequests",
        "Signals",
        "FactoryCases",
        "WorkCycles",
        "patrol-discord",
        "summary.json",
        "webhook-intake.svg",
        "request-webhook-event.json",
        "datadog-webhook-event.json",
        "discord-webhook-event.json",
        "discord-signal.json",
        "## State Diagram",
        "## OData Links",
        "proof bundle:",
    ] {
        assert!(
            script.contains(needle),
            "webhook intake smoke should prove live trigger-boundary intake: {needle}"
        );
    }
}

#[test]
fn production_readiness_script_keeps_mac_mini_activation_checkable() {
    let root = repo_root();
    let script = read(root.join("crates/paw-codex-worker/scripts/production-readiness.sh"));
    let preflight = read(root.join("crates/paw-codex-worker/scripts/production-preflight.sh"));
    let preflight_diff =
        read(root.join("crates/paw-codex-worker/scripts/production-preflight-diff.sh"));
    let smoke = read(root.join("crates/paw-codex-worker/scripts/production-readiness-smoke.sh"));
    let readme = read(root.join("crates/paw-codex-worker/README.md"));
    let env_example = read(root.join(".env.example"));

    for needle in [
        "TEMPER_URL",
        "WORKER_TOKEN",
        "WORKER_ID",
        "mac-mini-codex-prod",
        "cargo build -p paw-codex-worker --release",
        "paw-codex-worker doctor",
        "launchd-plist",
        "WRITE_LAUNCHD_PLIST",
        "INSTALL_LAUNCHD=1",
        "PAW_CODEX_ENABLE_EXECUTION=0",
        "PAW_CODEX_DOCTOR_EXEC_SMOKE",
    ] {
        assert!(
            script.contains(needle),
            "production readiness script should contain {needle}"
        );
    }

    for needle in [
        "PROOF_DIR",
        "summary.json",
        "proof.md",
        "preflight.svg",
        "operator-handoff.md",
        "git_head",
        "git_branch",
        "git_status_short",
        "git_clean",
        "Git head:",
        "Git clean:",
        "Human Blocker Decisions",
        "Secret And Approval Inputs",
        "human_blockers",
        "TEMPER_URL",
        "WORKER_TOKEN",
        "PATROL_OPERATOR_TOKEN",
        "launchctl print",
        "railway status",
        "railway project list --json",
        "railway-candidates.json",
        "railway:candidate_projects",
        "railway link <project_id>",
        "CHECK_RAILWAY",
        "STRICT=1",
        "CONFIRM_TEMPER_PIN_OK",
        "CONFIRM_TEMPERPAW_PR_OK",
        "statusCheckRollup",
        "TemperPaw PR #218 is clean and green but unmerged",
        "does not mutate Railway, launchd, or Temper",
    ] {
        assert!(
            preflight.contains(needle),
            "production preflight should report non-mutating activation readiness: {needle}"
        );
    }

    for needle in [
        "baseline-summary.json",
        "current-summary.json",
        "preflight-diff.svg",
        "Resolved Blockers",
        "New Blockers",
        "Railway Candidate Drift",
        "does not mutate Railway, launchd, Temper, or git",
    ] {
        assert!(
            preflight_diff.contains(needle),
            "production preflight diff should report activation drift: {needle}"
        );
    }

    for needle in [
        "production-readiness.sh",
        "production-preflight.sh",
        "paw-codex-worker doctor",
        "WRITE_LAUNCHD_PLIST=1",
        "INSTALL_LAUNCHD=0",
        "PAW_CODEX_ENABLE_EXECUTION=0",
        "PAW_CODEX_DOCTOR_EXEC_SMOKE=1",
        "token_not_printed_to_readiness_log",
        "launchd_not_loaded",
        "production readiness smoke passed",
    ] {
        assert!(
            smoke.contains(needle),
            "production readiness smoke should prove guarded cutover readiness: {needle}"
        );
    }

    for needle in [
        "scripts/production-preflight.sh",
        "scripts/production-readiness.sh",
        "scripts/production-readiness-smoke.sh",
        "WRITE_LAUNCHD_PLIST=1",
        "INSTALL_LAUNCHD=1",
        "does not print `WORKER_TOKEN`",
    ] {
        assert!(
            readme.contains(needle),
            "worker README should document production readiness: {needle}"
        );
    }

    for needle in [
        "Paw Patrol local Codex worker",
        "TEMPER_URL=",
        "WORKER_TOKEN=",
        "WORKER_ID=mac-mini-codex-prod",
        "PAW_CODEX_ENABLE_EXECUTION=0",
        "PAW_CODEX_DOCTOR_EXEC_SMOKE=1",
        "PAW_CODEX_POLL_ON_START=1",
    ] {
        assert!(
            env_example.contains(needle),
            ".env.example should document Patrol worker activation: {needle}"
        );
    }
}

#[test]
fn production_observe_only_script_turns_cutover_gate_into_a_guarded_proof() {
    let root = repo_root();
    let script = read(root.join("crates/paw-codex-worker/scripts/production-observe-only.sh"));
    let smoke = read(root.join("crates/paw-codex-worker/scripts/production-observe-only-smoke.sh"));
    let acceptance = read(root.join("crates/paw-codex-worker/scripts/paw-patrol-acceptance.sh"));
    let ci = read(root.join(".github/workflows/ci.yml"));
    let readme = read(root.join("crates/paw-codex-worker/README.md"));
    let runbook = read(root.join("docs/runbooks/paw-patrol-production-cutover.md"));

    for needle in [
        "ALLOW_PRODUCTION_WRITE=1",
        "PATROL_OPERATOR_TOKEN",
        "RepoGraphSnapshots",
        "TemperPaw.Patrol.StartScan",
        "WorkerRuns",
        "WorkCycles",
        "ReviewRuns",
        "EvaluationRuns",
        "ProofPackets",
        "DailyBriefs",
        "summary.json",
        "proof.md",
        "observe-only.svg",
        "allowed_worker_id",
        "PAW_CODEX_ENABLE_EXECUTION=0",
    ] {
        assert!(
            script.contains(needle),
            "production observe-only script should make Gate 6 executable and evidenced: {needle}"
        );
    }

    for needle in [
        "production-observe-only.sh",
        "ALLOW_PRODUCTION_WRITE=1",
        "PAW_CODEX_ENABLE_EXECUTION=0",
        "fixtures/fake-codex.sh",
        "production observe-only smoke passed",
        "observe-only.svg",
    ] {
        assert!(
            smoke.contains(needle),
            "production observe-only smoke should prove the guarded script locally: {needle}"
        );
    }

    for needle in [
        "production-observe-only.sh",
        "production-observe-only-smoke.sh",
        "production-observe-only/summary.json",
        "production-observe-only/observe-only.svg",
    ] {
        assert!(
            acceptance.contains(needle),
            "acceptance harness should collect observe-only proof evidence: {needle}"
        );
    }

    for needle in [
        "bash -n crates/paw-codex-worker/scripts/production-observe-only.sh",
        "bash -n crates/paw-codex-worker/scripts/production-observe-only-smoke.sh",
    ] {
        assert!(ci.contains(needle), "CI should syntax-check {needle}");
    }

    for needle in [
        "production-observe-only.sh",
        "production-observe-only-smoke.sh",
        "ALLOW_PRODUCTION_WRITE=1",
        "PATROL_OPERATOR_TOKEN",
    ] {
        assert!(
            readme.contains(needle),
            "worker README should document observe-only production proof: {needle}"
        );
        assert!(
            runbook.contains(needle),
            "production runbook should document observe-only production proof: {needle}"
        );
    }
}

#[test]
fn production_cutover_runbook_maps_every_human_blocker_to_a_gate() {
    let root = repo_root();
    let runbook = read(root.join("docs/runbooks/paw-patrol-production-cutover.md"));
    let readme = read(root.join("crates/paw-codex-worker/README.md"));

    for needle in [
        "```mermaid",
        "Railway TemperPaw URL",
        "WORKER_TOKEN",
        "production-preflight.sh",
        "local_codex_worker_id",
        "production-readiness-smoke.sh",
        "production-readiness.sh",
        "WRITE_LAUNCHD_PLIST=1",
        "INSTALL_LAUNCHD=1",
        "PAW_CODEX_ENABLE_EXECUTION=0",
        "PAW_CODEX_DOCTOR_EXEC_SMOKE=1",
        "Datadog",
        "Discord",
        "GitHub",
        "launchctl bootout",
        "Temper Cedar",
        "rollback",
        "human approval",
        "evidence to capture",
    ] {
        assert!(
            runbook.contains(needle),
            "production cutover runbook should map blocker or gate: {needle}"
        );
    }

    assert!(
        readme.contains("docs/runbooks/paw-patrol-production-cutover.md"),
        "worker README should point operators to the production cutover runbook"
    );
}

#[test]
fn paw_patrol_acceptance_harness_collects_quick_and_live_proofs() {
    let root = repo_root();
    let script = read(root.join("crates/paw-codex-worker/scripts/paw-patrol-acceptance.sh"));
    let readme = read(root.join("crates/paw-codex-worker/README.md"));
    let ci = read(root.join(".github/workflows/ci.yml"));

    for needle in [
        "quick",
        "live",
        "summary.json",
        "git_head",
        "git_branch",
        "git_status_short",
        "Git head:",
        "proof.md",
        "index.html",
        "acceptance.log",
        "cargo fmt --check --all",
        "cargo test --locked -p temperpaw --test paw_patrol_foundation -- --nocapture",
        "cargo test --locked -p paw-codex-worker --quiet",
        "deterministic-smoke.sh",
        "webhook-intake-smoke.sh",
        "repo-sweep-brief-smoke.sh",
        "production-preflight.sh",
        "production-preflight/summary.json",
        "production-preflight/proof.md",
        "production-preflight/operator-handoff.md",
        "production-preflight/preflight.svg",
        "production-preflight-railway-discovery-smoke.sh",
        "production-preflight-railway-discovery-smoke/railway-candidates.json",
        "production-preflight-railway-discovery-smoke/operator-handoff.md",
        "Railway Discovery Preflight",
        "production-preflight-diff-smoke.sh",
        "production-preflight-diff-smoke/preflight-diff.svg",
        "Preflight Diff",
        "production-preflight-github-smoke.sh",
        "production-preflight-github-smoke/summary-without-confirm.json",
        "production-preflight-github-smoke/summary-with-confirm.json",
        "production-readiness-smoke.sh",
        "deterministic-smoke/proof.svg",
        "webhook-intake-smoke/webhook-intake.svg",
        "repo-sweep-brief-smoke/patrol-schedule.json",
        "Default PatrolSchedule",
        "patrol-github",
        "github-webhook-event.json",
        "github-signal.json",
        "GitHub Signal",
        "repo-sweep-brief-smoke/daily-brief.svg",
        "write_artifact_link",
        "not generated in",
        "```mermaid",
        "proof bundle:",
    ] {
        assert!(
            script.contains(needle),
            "acceptance harness should collect quick/live proof evidence: {needle}"
        );
    }

    for needle in [
        "paw-patrol-acceptance.sh quick",
        "paw-patrol-acceptance.sh live",
        "/tmp/paw-patrol-acceptance-",
        "index.html",
    ] {
        assert!(
            readme.contains(needle),
            "worker README should document the acceptance harness: {needle}"
        );
    }

    assert!(
        ci.contains("bash -n crates/paw-codex-worker/scripts/paw-patrol-acceptance.sh"),
        "CI should syntax-check the acceptance harness"
    );
}

#[test]
fn ci_covers_paw_patrol_worker_and_wasm_gates() {
    let root = repo_root();
    let ci = read(root.join(".github/workflows/ci.yml"));

    for needle in [
        "workflow_dispatch:",
        "cargo clippy --locked -p temperpaw -p paw-codex-worker --all-targets -- -D warnings",
        "cargo check --locked -p temperpaw -p paw-codex-worker",
        "bash -n crates/paw-codex-worker/scripts/deterministic-smoke.sh",
        "bash -n crates/paw-codex-worker/scripts/repo-sweep-brief-smoke.sh",
        "bash -n crates/paw-codex-worker/scripts/webhook-intake-smoke.sh",
        "bash -n crates/paw-codex-worker/scripts/production-readiness.sh",
        "bash -n crates/paw-codex-worker/scripts/production-preflight.sh",
        "bash -n crates/paw-codex-worker/scripts/production-preflight-github-smoke.sh",
        "bash -n crates/paw-codex-worker/scripts/production-readiness-smoke.sh",
        "bash -n crates/paw-codex-worker/scripts/paw-patrol-acceptance.sh",
        "os-apps/paw-ingest/wasm/build.sh",
        "os-apps/paw-patrol/wasm/build.sh",
        "cargo test --locked -p paw-codex-worker --quiet",
        "cargo test --locked -p temperpaw --quiet",
        "cargo test --manifest-path os-apps/paw-patrol/wasm/review_gate_lifecycle/Cargo.toml --quiet",
    ] {
        assert!(
            ci.contains(needle),
            "CI should keep the Patrol worker/wasm surface covered: {needle}"
        );
    }
}

#[test]
fn paw_patrol_is_discoverable_by_the_os_app_catalog() {
    let root = repo_root();
    temper_platform::os_apps::set_os_apps_dir(root.join("os-apps"));

    let startup_apps = temper_platform::os_apps::list_startup_os_apps();
    assert!(
        startup_apps.iter().any(|app| app == "paw-patrol"),
        "paw-patrol should be part of the startup OS app surface: {startup_apps:?}"
    );

    let bundle = temper_platform::os_apps::get_os_app("paw-patrol")
        .expect("paw-patrol should load as an OS app bundle");
    assert_eq!(
        bundle.specs.len(),
        14,
        "paw-patrol should expose all Patrol entity specs"
    );
    assert!(
        bundle
            .csdl
            .as_deref()
            .unwrap_or_default()
            .contains("TemperPaw.Patrol"),
        "paw-patrol bundle should include the Patrol CSDL namespace"
    );
    assert!(
        !bundle.cedar_policies.is_empty(),
        "paw-patrol should include Cedar policies"
    );
    assert!(
        bundle.seed_instances.len() >= 10,
        "paw-patrol should seed RiskRule floors and Patrol webhook intake routes"
    );
}

#[test]
fn paw_patrol_has_webhook_intake_routes_through_paw_ingest() {
    let root = repo_root();

    let routes = read(root.join("os-apps/paw-patrol/seed-data/webhook_routes.toml"));
    for needle in [
        "id = \"patrol-request-route\"",
        "route_key = \"patrol-request\"",
        "target_entity_type = \"PatrolRequest\"",
        "target_action = \"TemperPaw.Patrol.Submit\"",
        "id = \"patrol-signal-route\"",
        "route_key = \"patrol-signal\"",
        "target_entity_type = \"Signal\"",
        "target_action = \"TemperPaw.Patrol.Ingest\"",
        "route_key = \"patrol-datadog\"",
        "route_key = \"patrol-github\"",
        "route_key = \"patrol-discord\"",
    ] {
        assert!(
            routes.contains(needle),
            "Patrol webhook route seed data should contain {needle}"
        );
    }

    let process_webhook = read(root.join("os-apps/paw-ingest/wasm/process_webhook/src/lib.rs"));
    for needle in [
        "build_patrol_request_submit_params",
        "\"request_text\"",
        "\"requester_id\"",
        "build_signal_ingest_params",
        "\"source_url\"",
        "\"severity\"",
        "\"payload\"",
        "fallback_source",
    ] {
        assert!(
            process_webhook.contains(needle),
            "process_webhook should translate payloads into Patrol params: {needle}"
        );
    }

    let ingest_manifest = read(root.join("os-apps/paw-ingest/app.toml"));
    for needle in [
        "name = \"validate_webhook\"",
        "name = \"route_webhook\"",
        "name = \"process_webhook\"",
        "criticality = \"app-required\"",
        "startup_loading = \"lazy\"",
    ] {
        assert!(
            ingest_manifest.contains(needle),
            "paw-ingest app.toml should explicitly install webhook WASM module {needle}"
        );
    }

    let ingest_build = read(root.join("os-apps/paw-ingest/wasm/build.sh"));
    for needle in [
        "validate_webhook",
        "route_webhook",
        "process_webhook",
        "cargo build --target wasm32-unknown-unknown --release",
    ] {
        assert!(
            ingest_build.contains(needle),
            "paw-ingest build.sh should build {needle}"
        );
    }

    let app_doc = read(root.join("os-apps/paw-patrol/APP.md"));
    for needle in [
        "/triggers/webhook/patrol-request",
        "/triggers/webhook/patrol-signal",
        "/triggers/webhook/patrol-datadog",
        "/triggers/webhook/patrol-github",
        "WebhookEvent -> PatrolRequest.Submit",
        "WebhookEvent -> Signal.Ingest",
    ] {
        assert!(app_doc.contains(needle), "APP.md should document {needle}");
    }
}

#[test]
fn patrol_schedule_recurs_sweeps_and_daily_briefs_inside_patrol() {
    let root = repo_root();
    let patrol = root.join("os-apps/paw-patrol");

    let manifest = read(patrol.join("app.toml"));
    for needle in [
        "name = \"patrol_schedule_lifecycle\"",
        "target = \"wasm32-unknown-unknown\"",
        "criticality = \"app-required\"",
        "startup_loading = \"lazy\"",
    ] {
        assert!(
            manifest.contains(needle),
            "app.toml should contain {needle}"
        );
    }

    let spec = read(patrol.join("specs/patrol_schedule.ioa.toml"));
    for needle in [
        "name = \"PatrolSchedule\"",
        "effect = [{ type = \"trigger\", name = \"patrol_schedule_activate\" }]",
        "effect = [{ type = \"increment\", var = \"run_count\" }, { type = \"trigger\", name = \"patrol_schedule_trigger\" }]",
        "{ type = \"schedule_at\", field = \"next_run_at\", action = \"Trigger\" }",
        "module = \"patrol_schedule_lifecycle\"",
        "name = \"TriggerComplete\"",
        "last_repo_graph_snapshot_id",
        "last_daily_brief_id",
    ] {
        assert!(
            spec.contains(needle),
            "PatrolSchedule spec should contain {needle}"
        );
    }

    let csdl = read(patrol.join("specs/model.csdl.xml"));
    for needle in [
        "<EntityType Name=\"PatrolSchedule\">",
        "<EntitySet Name=\"PatrolSchedules\" EntityType=\"TemperPaw.Patrol.PatrolSchedule\"/>",
        "<Property Name=\"NextRunAt\" Type=\"Edm.String\"/>",
    ] {
        assert!(csdl.contains(needle), "CSDL should expose {needle}");
    }

    let wasm_root = patrol.join("wasm/patrol_schedule_lifecycle");
    assert!(
        wasm_root.join("Cargo.toml").is_file(),
        "patrol_schedule_lifecycle should have a standalone WASM Cargo manifest"
    );
    assert!(
        wasm_root.join("src/lib.rs").is_file(),
        "patrol_schedule_lifecycle should have a WASM entry point"
    );

    let lifecycle = read(wasm_root.join("src/lib.rs"));
    for needle in [
        "/tdata/RepoGraphSnapshots",
        "/tdata/DailyBriefs",
        "TemperPaw.Patrol.StartScan",
        "TemperPaw.Patrol.Start",
        "TriggerComplete",
        "ActivateComplete",
        "parse_patrol_interval",
    ] {
        assert!(
            lifecycle.contains(needle),
            "patrol_schedule_lifecycle should contain {needle}"
        );
    }

    let policy = read(patrol.join("policies/patrol.cedar"));
    for needle in [
        "patrol_schedule_lifecycle",
        "PatrolSchedule",
        "Action::\"Configure\"",
        "Action::\"Activate\"",
        "Action::\"Trigger\"",
        "Action::\"TriggerComplete\"",
    ] {
        assert!(
            policy.contains(needle),
            "patrol.cedar should authorize PatrolSchedule with {needle}"
        );
    }
    for needle in [
        "principal.id == resource.WorkerId",
        "principal.id == resource.ReviewerId",
        "principal.id == resource.worker_id",
        "principal.id == resource.reviewer_id",
        "principal.agent_type == \"worker\"",
        "principal.agent_type == \"system\"",
        "[\"supervisor\", \"human\"].contains(principal.agent_type)",
    ] {
        assert!(
            policy.contains(needle),
            "patrol.cedar should bind lifecycle actions to claimed identities: {needle}"
        );
    }

    let build_script = read(patrol.join("wasm/build.sh"));
    assert!(
        build_script.contains("patrol_schedule_lifecycle"),
        "build.sh should build patrol_schedule_lifecycle"
    );

    let default_schedule = read(patrol.join("seed-data/default_schedules.toml"));
    for needle in [
        "id = \"patrol-default-daily-maintenance\"",
        "type = \"PatrolSchedule\"",
        "name = \"Default daily Patrol maintenance\"",
        "cadence = \"daily\"",
        "enable_repo_sweep = true",
        "enable_daily_brief = true",
        "[[instance.actions]]",
        "name = \"Configure\"",
        "name = \"Activate\"",
    ] {
        assert!(
            default_schedule.contains(needle),
            "default_schedules.toml should seed an active daily PatrolSchedule: {needle}"
        );
    }
}

#[test]
fn patrol_request_submit_is_temper_native_intake_routing() {
    let root = repo_root();
    let patrol = root.join("os-apps/paw-patrol");

    let manifest = read(patrol.join("app.toml"));
    for needle in [
        "[[wasm_modules]]",
        "name = \"patrol_request_router\"",
        "target = \"wasm32-unknown-unknown\"",
        "criticality = \"app-required\"",
        "startup_loading = \"lazy\"",
    ] {
        assert!(
            manifest.contains(needle),
            "app.toml should contain {needle}"
        );
    }

    let spec = read(patrol.join("specs/patrol_request.ioa.toml"));
    for needle in [
        "effect = [{ type = \"trigger\", name = \"route_patrol_request\" }]",
        "[[action.triggers]]",
        "name = \"route_patrol_request\"",
        "kind = \"wasm\"",
        "module = \"patrol_request_router\"",
        "on_failure = \"RouteFailed\"",
        "temper_api_url = \"{secret:temper_api_url}\"",
        "name = \"RouteFailed\"",
        "params = [\"error_message\", \"integration\"]",
    ] {
        assert!(
            spec.contains(needle),
            "PatrolRequest spec should contain {needle}"
        );
    }

    let wasm_root = patrol.join("wasm/patrol_request_router");
    assert!(
        wasm_root.join("Cargo.toml").is_file(),
        "patrol_request_router should have a standalone WASM Cargo manifest"
    );
    assert!(
        wasm_root.join("src/lib.rs").is_file(),
        "patrol_request_router should have a WASM entry point"
    );

    let router = read(wasm_root.join("src/lib.rs"));
    for needle in [
        "/tdata/Issues",
        "/tdata/FactoryCases",
        "/tdata/WorkCycles",
        "/tdata/WorkerRuns",
        "TemperPaw.PM.SetDescription",
        "TemperPaw.PM.SetPriority",
        "TemperPaw.PM.MoveToTriage",
        "TemperPaw.Patrol.Triage",
        "TemperPaw.Patrol.AcceptAsCase",
        "TemperPaw.Patrol.LinkPmIssue",
        "TemperPaw.Patrol.Open",
        "TemperPaw.Patrol.SetRiskFloor",
        "TemperPaw.Patrol.LinkPmIssue",
        "TemperPaw.Patrol.OpenWorkCycle",
        "TemperPaw.Patrol.QueueWork",
        "TemperPaw.Patrol.Configure",
        "TemperPaw.Patrol.WritePlan",
        "TemperPaw.Patrol.StartWork",
        "TemperPaw.Patrol.AttachWorkerRun",
        "runner_kind",
        "allowed_worker_id",
        "local_codex",
        "paw-codex-worker",
    ] {
        assert!(
            router.contains(needle),
            "patrol_request_router should contain {needle}"
        );
    }

    let policy = read(patrol.join("policies/patrol.cedar"));
    for needle in [
        "action == Action::\"http_call\"",
        "resource is HttpEndpoint",
        "patrol_request_router",
        "worker_run_lifecycle",
    ] {
        assert!(
            policy.contains(needle),
            "patrol.cedar should authorize WASM host HTTP calls with {needle}"
        );
    }
}

#[test]
fn signal_ingest_routes_observable_failures_into_patrol_work() {
    let root = repo_root();
    let patrol = root.join("os-apps/paw-patrol");

    let manifest = read(patrol.join("app.toml"));
    for needle in [
        "name = \"signal_router\"",
        "target = \"wasm32-unknown-unknown\"",
        "criticality = \"app-required\"",
        "startup_loading = \"lazy\"",
    ] {
        assert!(
            manifest.contains(needle),
            "app.toml should contain {needle}"
        );
    }

    let spec = read(patrol.join("specs/signal.ioa.toml"));
    for needle in [
        "effect = [{ type = \"trigger\", name = \"route_signal\" }]",
        "[[action.triggers]]",
        "name = \"route_signal\"",
        "kind = \"wasm\"",
        "module = \"signal_router\"",
        "on_failure = \"Archive\"",
        "temper_api_url = \"{secret:temper_api_url}\"",
    ] {
        assert!(spec.contains(needle), "Signal spec should contain {needle}");
    }

    let wasm_root = patrol.join("wasm/signal_router");
    assert!(
        wasm_root.join("Cargo.toml").is_file(),
        "signal_router should have a standalone WASM Cargo manifest"
    );
    assert!(
        wasm_root.join("src/lib.rs").is_file(),
        "signal_router should have a WASM entry point"
    );

    let router = read(wasm_root.join("src/lib.rs"));
    for needle in [
        "/tdata/Issues",
        "/tdata/FactoryCases",
        "/tdata/WorkCycles",
        "/tdata/WorkerRuns",
        "TemperPaw.PM.SetDescription",
        "TemperPaw.PM.SetPriority",
        "TemperPaw.PM.MoveToTriage",
        "TemperPaw.Patrol.Normalize",
        "TemperPaw.Patrol.Triage",
        "TemperPaw.Patrol.AttachCase",
        "TemperPaw.Patrol.Archive",
        "TemperPaw.Patrol.Open",
        "TemperPaw.Patrol.SetRiskFloor",
        "TemperPaw.Patrol.LinkPmIssue",
        "TemperPaw.Patrol.OpenWorkCycle",
        "TemperPaw.Patrol.QueueWork",
        "TemperPaw.Patrol.Configure",
        "TemperPaw.Patrol.WritePlan",
        "TemperPaw.Patrol.StartWork",
        "TemperPaw.Patrol.AttachWorkerRun",
        "Datadog",
        "Discord",
        "local_codex",
    ] {
        assert!(
            router.contains(needle),
            "signal_router should contain {needle}"
        );
    }

    let policy = read(patrol.join("policies/patrol.cedar"));
    assert!(
        policy.contains("signal_router"),
        "patrol.cedar should authorize signal_router host HTTP calls"
    );
}

#[test]
fn worker_run_done_fans_out_to_review_evaluation_and_proof() {
    let root = repo_root();
    let patrol = root.join("os-apps/paw-patrol");

    let manifest = read(patrol.join("app.toml"));
    for needle in [
        "name = \"worker_run_lifecycle\"",
        "target = \"wasm32-unknown-unknown\"",
        "criticality = \"app-required\"",
        "startup_loading = \"lazy\"",
    ] {
        assert!(
            manifest.contains(needle),
            "app.toml should contain {needle}"
        );
    }

    let spec = read(patrol.join("specs/worker_run.ioa.toml"));
    for needle in [
        "effect = [{ type = \"trigger\", name = \"worker_run_started\" }]",
        "effect = [{ type = \"trigger\", name = \"worker_run_finished\" }]",
        "effect = [{ type = \"trigger\", name = \"worker_run_failed\" }]",
        "module = \"worker_run_lifecycle\"",
        "on_failure = \"ReportFailed\"",
    ] {
        assert!(
            spec.contains(needle),
            "WorkerRun spec should contain {needle}"
        );
    }

    let wasm_root = patrol.join("wasm/worker_run_lifecycle");
    assert!(
        wasm_root.join("Cargo.toml").is_file(),
        "worker_run_lifecycle should have a standalone WASM Cargo manifest"
    );
    assert!(
        wasm_root.join("src/lib.rs").is_file(),
        "worker_run_lifecycle should have a WASM entry point"
    );

    let lifecycle = read(wasm_root.join("src/lib.rs"));
    for needle in [
        "/tdata/FactoryCases",
        "/tdata/WorkCycles",
        "/tdata/ReviewRuns",
        "/tdata/EvaluationRuns",
        "/tdata/ProofPackets",
        "TemperPaw.Patrol.BeginWork",
        "TemperPaw.Patrol.BeginReview",
        "TemperPaw.Patrol.WorkerDone",
        "TemperPaw.Patrol.SubmitForReview",
        "TemperPaw.Patrol.AttachReviewRun",
        "TemperPaw.Patrol.AttachEvaluationRun",
        "TemperPaw.Patrol.Request",
        "TemperPaw.Patrol.Queue",
        "TemperPaw.Patrol.AttachDraft",
        "independent reviewer",
        "visual ProofPacket",
        "data:image/svg+xml",
        "visual_summary_svg",
    ] {
        assert!(
            lifecycle.contains(needle),
            "worker_run_lifecycle should contain {needle}"
        );
    }
}

#[test]
fn reviewer_and_evaluator_results_gate_completion_before_human_review() {
    let root = repo_root();
    let patrol = root.join("os-apps/paw-patrol");

    let manifest = read(patrol.join("app.toml"));
    for needle in [
        "name = \"review_gate_lifecycle\"",
        "target = \"wasm32-unknown-unknown\"",
        "criticality = \"app-required\"",
        "startup_loading = \"lazy\"",
    ] {
        assert!(
            manifest.contains(needle),
            "app.toml should contain {needle}"
        );
    }

    let review_spec = read(patrol.join("specs/review_run.ioa.toml"));
    for needle in [
        "effect = [{ type = \"trigger\", name = \"review_run_approved\" }]",
        "effect = [{ type = \"trigger\", name = \"review_run_changes_requested\" }]",
        "effect = [{ type = \"trigger\", name = \"review_run_escalated\" }]",
        "effect = [{ type = \"trigger\", name = \"review_run_failed\" }]",
        "module = \"review_gate_lifecycle\"",
    ] {
        assert!(
            review_spec.contains(needle),
            "ReviewRun spec should contain {needle}"
        );
    }

    let evaluation_spec = read(patrol.join("specs/evaluation_run.ioa.toml"));
    for needle in [
        "name = \"evaluator_id\"",
        "name = \"Claim\"",
        "params = [\"evaluator_id\"]",
        "effect = [{ type = \"trigger\", name = \"evaluation_run_passed\" }]",
        "effect = [{ type = \"trigger\", name = \"evaluation_run_failed\" }]",
        "module = \"review_gate_lifecycle\"",
    ] {
        assert!(
            evaluation_spec.contains(needle),
            "EvaluationRun spec should contain {needle}"
        );
    }

    let wasm_root = patrol.join("wasm/review_gate_lifecycle");
    assert!(
        wasm_root.join("Cargo.toml").is_file(),
        "review_gate_lifecycle should have a standalone WASM Cargo manifest"
    );
    assert!(
        wasm_root.join("src/lib.rs").is_file(),
        "review_gate_lifecycle should have a WASM entry point"
    );

    let lifecycle = read(wasm_root.join("src/lib.rs"));
    for needle in [
        "/tdata/WorkCycles",
        "/tdata/FactoryCases",
        "/tdata/ProofPackets",
        "TemperPaw.Patrol.PassReview",
        "TemperPaw.Patrol.RequestChanges",
        "TemperPaw.Patrol.ReportE2e",
        "TemperPaw.Patrol.PassEvaluation",
        "TemperPaw.Patrol.AttachProofPacket",
        "TemperPaw.Patrol.Complete",
        "TemperPaw.Patrol.BeginProof",
        "TemperPaw.Patrol.MarkReady",
        "TemperPaw.Patrol.Reject",
        "TemperPaw.Patrol.Escalate",
        "record_e2e_if_present",
        "reviewer approved before human review",
        "evaluation gates passed before proof readiness",
    ] {
        assert!(
            lifecycle.contains(needle),
            "review_gate_lifecycle should contain {needle}"
        );
    }
}

#[test]
fn work_cycle_completion_requires_recorded_live_e2e_evidence() {
    let root = repo_root();
    let patrol = root.join("os-apps/paw-patrol");
    let work_cycle = read(patrol.join("specs/work_cycle.ioa.toml"));

    for needle in [
        "name = \"ReportE2e\"",
        "name = \"e2e_summary\"",
        "from = [\"Reviewing\"]",
        "to = \"Reviewing\"",
        "params = [\"e2e_summary\"]",
        "effect = \"set e2e_ok true\"",
        "{ type = \"is_true\", var = \"e2e_ok\" }",
        "Complete only when review, evaluation, proof, and live/E2E evidence are all attached.",
    ] {
        assert!(
            work_cycle.contains(needle),
            "WorkCycle should require explicit live/E2E evidence before completion: {needle}"
        );
    }

    let review_gate = read(patrol.join("wasm/review_gate_lifecycle/src/lib.rs"));
    for needle in [
        "TemperPaw.Patrol.ReportE2e",
        "record_e2e_if_present",
        "e2e_summary",
        "EvaluationRun",
        "live_e2e_summary",
    ] {
        assert!(
            review_gate.contains(needle),
            "review_gate_lifecycle should record E2E evidence before final gates: {needle}"
        );
    }

    let csdl = read(patrol.join("specs/model.csdl.xml"));
    assert!(
        csdl.contains("<Property Name=\"E2eSummary\" Type=\"Edm.String\"/>"),
        "WorkCycle CSDL should expose durable live/E2E evidence"
    );
}

#[test]
fn high_risk_work_requires_human_start_and_completion_approval() {
    let root = repo_root();
    let patrol = root.join("os-apps/paw-patrol");

    let work_cycle = read(patrol.join("specs/work_cycle.ioa.toml"));
    for needle in [
        "AwaitingHumanStartApproval",
        "AwaitingHumanCompletionApproval",
        "name = \"human_start_approval_required\"",
        "name = \"human_start_approved\"",
        "name = \"human_completion_approval_required\"",
        "name = \"human_completion_approved\"",
        "name = \"task_detail\"",
        "name = \"RequestHumanStartApproval\"",
        "name = \"ApproveHumanStart\"",
        "name = \"RequestHumanCompletionApproval\"",
        "name = \"ApproveHumanCompletion\"",
        "module = \"work_cycle_lifecycle\"",
    ] {
        assert!(
            work_cycle.contains(needle),
            "WorkCycle spec should encode L3 human approval gates: {needle}"
        );
    }

    let manifest = read(patrol.join("app.toml"));
    assert!(
        manifest.contains("name = \"work_cycle_lifecycle\""),
        "paw-patrol should install work_cycle_lifecycle for approved L3 dispatch"
    );

    let request_router = read(patrol.join("wasm/patrol_request_router/src/lib.rs"));
    for needle in [
        "requires_human_start_approval",
        "PATROL_REQUEST_HUMAN_START_APPROVAL",
        "if requires_human_start_approval(risk.lane)",
        "queued after human start approval",
    ] {
        assert!(
            request_router.contains(needle),
            "patrol_request_router should pause L3 requests before WorkerRun: {needle}"
        );
    }

    let signal_router = read(patrol.join("wasm/signal_router/src/lib.rs"));
    for needle in [
        "requires_human_start_approval",
        "PATROL_REQUEST_HUMAN_START_APPROVAL",
        "if requires_human_start_approval(risk.lane)",
        "queued after human start approval",
    ] {
        assert!(
            signal_router.contains(needle),
            "signal_router should pause L3 signals before WorkerRun: {needle}"
        );
    }

    let work_cycle_lifecycle = read(patrol.join("wasm/work_cycle_lifecycle/src/lib.rs"));
    for needle in [
        "/tdata/WorkerRuns",
        "ApproveHumanStart",
        "TemperPaw.Patrol.StartWork",
        "TemperPaw.Patrol.AttachWorkerRun",
        "TemperPaw.Patrol.QueueWork",
        "human-approved L3 work",
        "local_codex",
    ] {
        assert!(
            work_cycle_lifecycle.contains(needle),
            "work_cycle_lifecycle should dispatch approved high-risk work: {needle}"
        );
    }

    let review_gate = read(patrol.join("wasm/review_gate_lifecycle/src/lib.rs"));
    for needle in [
        "requires_human_completion_approval",
        "TemperPaw.Patrol.RequestHumanCompletionApproval",
        "human completion approval required before WorkCycle.Complete",
        "ApproveHumanCompletion",
    ] {
        assert!(
            review_gate.contains(needle),
            "review_gate_lifecycle should pause L3 completion after proof gates: {needle}"
        );
    }

    let policy = read(patrol.join("policies/patrol.cedar"));
    for needle in [
        "Action::\"RequestHumanStartApproval\"",
        "Action::\"ApproveHumanStart\"",
        "Action::\"RequestHumanCompletionApproval\"",
        "Action::\"ApproveHumanCompletion\"",
        "work_cycle_lifecycle",
    ] {
        assert!(
            policy.contains(needle),
            "patrol.cedar should authorize L3 approval transitions: {needle}"
        );
    }
}

#[test]
fn repo_graph_snapshot_queues_sweep_and_fans_out_findings() {
    let root = repo_root();
    let patrol = root.join("os-apps/paw-patrol");

    let manifest = read(patrol.join("app.toml"));
    for needle in [
        "name = \"repo_sweep_lifecycle\"",
        "target = \"wasm32-unknown-unknown\"",
        "criticality = \"app-required\"",
        "startup_loading = \"lazy\"",
    ] {
        assert!(
            manifest.contains(needle),
            "app.toml should contain {needle}"
        );
    }

    let spec = read(patrol.join("specs/repo_graph_snapshot.ioa.toml"));
    for needle in [
        "effect = [{ type = \"trigger\", name = \"repo_sweep_started\" }]",
        "effect = [{ type = \"trigger\", name = \"repo_sweep_completed\" }]",
        "module = \"repo_sweep_lifecycle\"",
        "on_failure = \"ScanFailed\"",
        "name = \"work_cycle_id\"",
        "name = \"worker_run_id\"",
        "name = \"AttachWorkerRun\"",
        "params = [\"error_message\", \"integration\"]",
    ] {
        assert!(
            spec.contains(needle),
            "RepoGraphSnapshot spec should contain {needle}"
        );
    }

    let wasm_root = patrol.join("wasm/repo_sweep_lifecycle");
    assert!(
        wasm_root.join("Cargo.toml").is_file(),
        "repo_sweep_lifecycle should have a standalone WASM Cargo manifest"
    );
    assert!(
        wasm_root.join("src/lib.rs").is_file(),
        "repo_sweep_lifecycle should have a WASM entry point"
    );

    let lifecycle = read(wasm_root.join("src/lib.rs"));
    for needle in [
        "/tdata/WorkCycles",
        "/tdata/WorkerRuns",
        "/tdata/QualityFindings",
        "/tdata/SecurityFindings",
        "TemperPaw.Patrol.Configure",
        "TemperPaw.Patrol.WritePlan",
        "TemperPaw.Patrol.StartWork",
        "TemperPaw.Patrol.AttachWorkerRun",
        "TemperPaw.Patrol.OpenFinding",
        "repo graph and dependency sweep",
        "giant modules",
        "duplicate logic",
        "fingerprint",
        "TODO/HACK",
        "Cedar drift",
        "local_codex",
    ] {
        assert!(
            lifecycle.contains(needle),
            "repo_sweep_lifecycle should contain {needle}"
        );
    }

    let policy = read(patrol.join("policies/patrol.cedar"));
    for needle in [
        "repo_sweep_lifecycle",
        "RepoGraphSnapshot",
        "QualityFinding",
        "SecurityFinding",
        "Action::\"StartScan\"",
        "Action::\"ScanComplete\"",
        "Action::\"OpenFinding\"",
    ] {
        assert!(
            policy.contains(needle),
            "patrol.cedar should authorize repo sweep lifecycle with {needle}"
        );
    }
}

#[test]
fn accepted_findings_queue_cleanup_work_cycles() {
    let root = repo_root();
    let patrol = root.join("os-apps/paw-patrol");

    let quality_spec = read(patrol.join("specs/quality_finding.ioa.toml"));
    let security_spec = read(patrol.join("specs/security_finding.ioa.toml"));
    for (label, spec) in [
        ("QualityFinding", quality_spec.as_str()),
        ("SecurityFinding", security_spec.as_str()),
    ] {
        for needle in [
            "effect = [{ type = \"trigger\"",
            "module = \"finding_lifecycle\"",
            "name = \"Accept\"",
            "name = \"LinkPmIssue\"",
            "name = \"StartWork\"",
            "name = \"fingerprint\"",
        ] {
            assert!(
                spec.contains(needle),
                "{label} should trigger cleanup work on Accept: {needle}"
            );
        }
    }

    let manifest = read(patrol.join("app.toml"));
    assert!(
        manifest.contains("name = \"finding_lifecycle\""),
        "paw-patrol should install finding_lifecycle"
    );

    let lifecycle = read(patrol.join("wasm/finding_lifecycle/src/lib.rs"));
    for needle in [
        "/tdata/Issues",
        "/tdata/WorkCycles",
        "/tdata/WorkerRuns",
        "QualityFinding",
        "SecurityFinding",
        "TemperPaw.PM.SetDescription",
        "TemperPaw.PM.MoveToTriage",
        "TemperPaw.Patrol.LinkPmIssue",
        "TemperPaw.Patrol.StartWork",
        "TemperPaw.Patrol.RequestHumanStartApproval",
        "cleanup WorkCycle for accepted finding",
        "requires_human_start_approval",
        "local_codex",
    ] {
        assert!(
            lifecycle.contains(needle),
            "finding_lifecycle should turn accepted findings into actionable work: {needle}"
        );
    }

    let policy = read(patrol.join("policies/patrol.cedar"));
    for needle in [
        "finding_lifecycle",
        "Action::\"Accept\"",
        "Action::\"LinkPmIssue\"",
        "Action::\"StartWork\"",
    ] {
        assert!(
            policy.contains(needle),
            "patrol.cedar should authorize finding cleanup actions: {needle}"
        );
    }
}

#[test]
fn accepted_finding_work_cycles_resolve_source_findings_on_completion() {
    let root = repo_root();
    let patrol = root.join("os-apps/paw-patrol");

    let work_cycle = read(patrol.join("specs/work_cycle.ioa.toml"));
    for needle in [
        "name = \"source_entity_type\"",
        "name = \"source_entity_id\"",
        "name = \"LinkSource\"",
        "params = [\"source_entity_type\", \"source_entity_id\"]",
        "name = \"work_cycle_completed\"",
        "module = \"work_cycle_lifecycle\"",
    ] {
        assert!(
            work_cycle.contains(needle),
            "WorkCycle should carry and trigger source finding closure: {needle}"
        );
    }

    let csdl = read(patrol.join("specs/model.csdl.xml"));
    for needle in [
        "<Property Name=\"SourceEntityType\" Type=\"Edm.String\"/>",
        "<Property Name=\"SourceEntityId\" Type=\"Edm.String\"/>",
    ] {
        assert!(csdl.contains(needle), "CSDL should expose {needle}");
    }

    let finding_lifecycle = read(patrol.join("wasm/finding_lifecycle/src/lib.rs"));
    for needle in [
        "TemperPaw.Patrol.LinkSource",
        "\"source_entity_type\"",
        "\"source_entity_id\"",
    ] {
        assert!(
            finding_lifecycle.contains(needle),
            "finding_lifecycle should link WorkCycle back to source finding: {needle}"
        );
    }

    let work_cycle_lifecycle = read(patrol.join("wasm/work_cycle_lifecycle/src/lib.rs"));
    for needle in [
        "handle_complete",
        "/tdata/QualityFindings",
        "/tdata/SecurityFindings",
        "TemperPaw.Patrol.Resolve",
        "source_entity_type",
        "source_entity_id",
        "ProofPacket",
    ] {
        assert!(
            work_cycle_lifecycle.contains(needle),
            "work_cycle_lifecycle should resolve source findings with proof: {needle}"
        );
    }

    let policy = read(patrol.join("policies/patrol.cedar"));
    for needle in ["Action::\"LinkSource\"", "Action::\"Resolve\""] {
        assert!(
            policy.contains(needle),
            "Cedar should authorize WorkCycle source closure action: {needle}"
        );
    }
}

#[test]
fn daily_brief_renders_visual_human_review_rollup() {
    let root = repo_root();
    let patrol = root.join("os-apps/paw-patrol");

    let manifest = read(patrol.join("app.toml"));
    for needle in [
        "name = \"daily_brief_lifecycle\"",
        "target = \"wasm32-unknown-unknown\"",
        "criticality = \"app-required\"",
        "startup_loading = \"lazy\"",
    ] {
        assert!(
            manifest.contains(needle),
            "app.toml should contain {needle}"
        );
    }

    let spec = read(patrol.join("specs/daily_brief.ioa.toml"));
    for needle in [
        "effect = [{ type = \"trigger\", name = \"daily_brief_started\" }]",
        "module = \"daily_brief_lifecycle\"",
        "on_failure = \"Fail\"",
        "name = \"Render\"",
        "visual_summary_url",
    ] {
        assert!(
            spec.contains(needle),
            "DailyBrief spec should contain {needle}"
        );
    }

    let wasm_root = patrol.join("wasm/daily_brief_lifecycle");
    assert!(
        wasm_root.join("Cargo.toml").is_file(),
        "daily_brief_lifecycle should have a standalone WASM Cargo manifest"
    );
    assert!(
        wasm_root.join("src/lib.rs").is_file(),
        "daily_brief_lifecycle should have a WASM entry point"
    );

    let lifecycle = read(wasm_root.join("src/lib.rs"));
    for needle in [
        "/tdata/ProofPackets",
        "/tdata/QualityFindings",
        "/tdata/SecurityFindings",
        "/tdata/WorkCycles",
        "TemperPaw.Patrol.Render",
        "data:image/svg+xml",
        "visual_daily_brief_svg",
        "human-readable daily brief",
        "open risks",
        "done items",
    ] {
        assert!(
            lifecycle.contains(needle),
            "daily_brief_lifecycle should contain {needle}"
        );
    }

    let policy = read(patrol.join("policies/patrol.cedar"));
    for needle in [
        "daily_brief_lifecycle",
        "DailyBrief",
        "Action::\"Start\"",
        "Action::\"Render\"",
        "Action::\"Publish\"",
    ] {
        assert!(
            policy.contains(needle),
            "patrol.cedar should authorize daily brief lifecycle with {needle}"
        );
    }
}

#[test]
fn paw_patrol_wasm_modules_have_startup_build_script() {
    let root = repo_root();
    let build_script = root.join("os-apps/paw-patrol/wasm/build.sh");

    assert!(
        build_script.is_file(),
        "paw-patrol should have a wasm/build.sh so fresh boots can load bundled modules"
    );

    let script = read(build_script);
    for needle in [
        "patrol_request_router",
        "signal_router",
        "worker_run_lifecycle",
        "review_gate_lifecycle",
        "repo_sweep_lifecycle",
        "daily_brief_lifecycle",
        "cargo build --target wasm32-unknown-unknown --release",
        "cp \"$source_file\" \"$SCRIPT_DIR/$module/$module.wasm\"",
    ] {
        assert!(script.contains(needle), "build.sh should contain {needle}");
    }
}

#[test]
fn current_state_audit_uses_live_proof_sources_instead_of_stale_heads() {
    let root = repo_root();
    let audit = read(root.join("docs/proofs/2026-05-05-paw-patrol-current-state-audit.md"));

    for needle in [
        "PR #218 body",
        "latest production preflight summary",
        "canonical moving evidence",
        "latest exact-head quick acceptance proof",
    ] {
        assert!(
            audit.contains(needle),
            "current-state audit should point readers at live proof source: {needle}"
        );
    }

    for stale in [
        "927d9844bdd22238e6eade507560a16fee7c1a0b",
        "/tmp/paw-patrol-acceptance-quick-927d9844-preflight-stamp",
        "25393554285",
    ] {
        assert!(
            !audit.contains(stale),
            "current-state audit should not pin stale proof evidence: {stale}"
        );
    }
}
