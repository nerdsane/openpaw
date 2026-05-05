# Paw Patrol Dark Factory Completion Audit

Date: 2026-05-05

Latest exact-head refresh after documentation/readability ratchets:

- PR #218 head: `83cb8966322cb8025f883f00f4a1ac461daa5ccb`
- GitHub CI: passed at
  <https://github.com/nerdsane/temperpaw/actions/runs/25403447886>
- current-head quick acceptance:
  `/tmp/paw-patrol-acceptance-quick-83cb8966` with 24/24 gates passed
- current-head production preflight:
  `/tmp/paw-patrol-production-preflight-83cb8966`, status `blocked`, with 12
  human-controlled blockers

The older live acceptance bundles below remain the full local E2E evidence for
the last functional code head; the latest current-head quick proof and CI cover
the subsequent docs/readability commits.

## Objective Restated

Make TemperPaw `paw-patrol` fully working and usable as the
Patrol-controlled Dark Factory:

- usable intake from humans, manager agents, Discord/Datadog/GitHub/webhooks,
  and schedules;
- local Mac mini Codex execution without OpenAI API-key billing for v1;
- repo-health sweeps that create quality and security findings;
- implementer, independent reviewer, evaluation, and proof gates;
- recurring repo sweeps and daily briefs;
- clear visual/human-facing ProofPackets and proof documents;
- risk lanes and Cedar controls that keep high-risk work human-gated;
- Temper-native implementation using entity specs, WASM integrations, and
  Cedar policies, with no separate factory/quality/harness app.

## Prompt-To-Artifact Checklist

| Requirement | Artifact inspected | Evidence | Status |
| --- | --- | --- | --- |
| Build as `paw-patrol`, not separate factory/quality/harness apps | `os-apps/paw-patrol/app.toml`, `crates/temperpaw/tests/paw_patrol_foundation.rs` | Foundation test `paw_patrol_owns_the_dark_factory_entities_without_extra_factory_apps` passes and asserts the named Patrol entity set | Done |
| Patrol owns the required entities | `os-apps/paw-patrol/specs/*.ioa.toml`, `specs/model.csdl.xml` | Specs exist for PatrolRequest, Signal, FactoryCase, WorkCycle, WorkerRun, ReviewRun, EvaluationRun, ProofPacket, RiskRule, RepoGraphSnapshot, QualityFinding, SecurityFinding, DailyBrief, and PatrolSchedule | Done |
| Use `paw-pm` Issues only after Patrol triage | `patrol_request_router`, `signal_router`, `finding_lifecycle` | Routers create/link PM Issues after accepting work; tests assert PM linkage and Patrol routing | Done |
| Intake from humans/agents/webhooks/signals | `os-apps/paw-ingest/app.toml`, `os-apps/paw-ingest/wasm/process_webhook/src/lib.rs`, `os-apps/paw-patrol/seed-data/webhook_routes.toml`, `webhook-intake-smoke.sh` | `process_webhook` builds typed `PatrolRequest.Submit` and `Signal.Ingest` params; live webhook smoke posts to the trigger listener and reaches PatrolRequest Linked plus Datadog, GitHub, and Discord Signals Linked | Done |
| Local worker execution on Mac mini | `crates/paw-codex-worker/src/main.rs`, `README.md`, launchd template | Worker has event-stream watching with OData polling fallback, boot polling, claim/start/report actions, doctor, launchd-plist renderer, and execution toggle; worker tests pass | Done locally |
| Avoid pure API billing for v1 | `README.md`, worker config | Worker uses local Codex CLI path and ChatGPT/Codex auth; fake fixture allows no-billing E2E smoke | Done locally |
| Codex Cloud manual overflow only | `worker_run.ioa.toml`, worker tests | WorkerRun encodes `RequestCloudOverflow`; tests assert manual cloud overflow semantics | Done |
| Risk lanes via explicit rules; agents cannot lower risk | `risk_rules.toml`, `factory_case.ioa.toml`, Patrol tests | `risk_rules_set_a_floor_that_agents_cannot_silently_lower` passes; router risk regression tests pass | Done |
| High-risk work pauses before start and before completion | `work_cycle.ioa.toml`, `work_cycle_lifecycle`, `review_gate_lifecycle` | Foundation test `high_risk_work_requires_human_start_and_completion_approval` passes | Done |
| Worker claims are bound to the registered Mac mini identity and a worktree assignment | `worker_run.ioa.toml`, `patrol.cedar`, Patrol WASM modules, `paw-codex-worker` | `WorkerRun.allowed_worker_id` is configured from `local_codex_worker_id`; Cedar only permits matching worker principals to claim; worker refuses mismatched queued runs and refuses to claim local Codex work without a `branch_name` or `worktree_path`; foundation test `worker_claims_are_bound_to_the_configured_local_worker` and worker unit `local_worker_claims_only_configured_local_codex_runs` pass | Done |
| Independent reviewer before user review | `worker_run_lifecycle`, `review_gate_lifecycle`, `paw-codex-worker` reviewer path | Worker completion queues ReviewRun; worker reviewer requires explicit verdict; live smoke reached ReviewRun Approved before ProofPacket Ready | Done |
| Automated evaluation gates | `evaluation_run.ioa.toml`, `review_gate_lifecycle`, worker evaluation commands | Worker can run configured local commands; live smoke EvaluationRun Passed with `test -f .paw-fake-codex-implementation`; EvaluationRun Start/Pass/Fail is Cedar-bound to the claimed `evaluator_id` | Done |
| Live/E2E evidence is a required completion gate | `work_cycle.ioa.toml`, `review_gate_lifecycle`, deterministic smoke proof | `WorkCycle.Complete` now requires `e2e_ok`; `EvaluationRun.Pass` records `WorkCycle.ReportE2e` before `PassEvaluation`; latest deterministic proof JSON contains `"e2e_gate": "passed"` and the proof diagram includes `EvaluationRun --> WorkCycle: ReportE2e recorded live evidence` | Done |
| Human gates are human-gated in Cedar | `patrol.cedar`, `crates/temperpaw/tests/paw_patrol_foundation.rs` | Actual Cedar regression test `patrol_cedar_human_gate_approvals_are_not_available_to_system_agents` denies `ApproveHumanStart` and `ApproveHumanCompletion` to `agent_type = system`, while allowing `agent_type = human` | Done |
| Visual ProofPacket | `proof_packet.ioa.toml`, `worker_run_lifecycle`, `review_gate_lifecycle` | Live smoke ProofPacket reached Ready with final `data:image/svg+xml` visual summary, reviewer verdict, residual risk text, Mermaid state diagram, OData links, and log evidence. The final proof no longer carries stale pending-review/pending-evaluation draft labels | Done |
| Proof changed-files map comes from worker evidence | `crates/paw-codex-worker/src/execution.rs`, `worker_run_lifecycle`, `review_gate_lifecycle` | Local Codex success summaries now include fenced `git-status` and `git-diff-stat` evidence; `worker_run_lifecycle` extracts changed files into `ProofPacket.changed_files_map`; `review_gate_lifecycle` preserves concrete file lists when marking proof ready. Latest deterministic proof contains `changed_files: [".paw-fake-codex-implementation"]` | Done |
| Repo-health sweeps | `repo_graph_snapshot.ioa.toml`, `repo_sweep_lifecycle`, worker repo scan, `repo-sweep-brief-smoke.sh` | Live repo-sweep smoke reached RepoGraphSnapshot Ready, WorkCycle Complete, ReviewRun Approved, EvaluationRun Passed, ProofPacket Ready, and produced 51 Quality/Security findings in `repo-graph.json` | Done |
| Repo-health scanner covers the claimed signal classes | `crates/paw-codex-worker/src/repo_health.rs`, `crates/paw-codex-worker/src/tests.rs` | The worker scan now emits concrete findings, stable `fingerprint` values, and summary counters for duplicate logic candidates, sleep-based polling loops, Cargo/npm dependency risk, and missing WASM test coverage, in addition to giant modules, TODO/HACK band-aids, broad Cedar permits, and hidden Rust orchestration markers. Worker test `repo_health_scan_emits_quality_and_security_findings` ratchets these signal classes | Done locally |
| Accepted findings become cleanup WorkCycles and resolve on completion | `finding_lifecycle`, `work_cycle_lifecycle` | `accepted_findings_queue_cleanup_work_cycles` and `accepted_finding_work_cycles_resolve_source_findings_on_completion` pass | Done |
| Recurring sweeps and daily briefs | `patrol_schedule.ioa.toml`, `patrol_schedule_lifecycle`, `daily_brief_lifecycle`, `repo-sweep-brief-smoke.sh` | Foundation tests assert PatrolSchedule recurrence; live repo-sweep/brief smoke rendered DailyBrief Ready with `daily-brief.svg`, ready ProofPacket IDs, done items, and open risk JSON | Done |
| Fresh installs have a default daily maintenance schedule | `os-apps/paw-patrol/seed-data/default_schedules.toml`, `repo-sweep-brief-smoke.sh` | `patrol-default-daily-maintenance` is seeded through PatrolSchedule `Configure` + `Activate`; live repo-sweep/brief smoke captured `patrol-schedule.json` with status `Active` and `next_run_at = 2026-05-06T12:26:12Z` | Done |
| Human-readable proof docs | `docs/proofs/2026-05-04-paw-patrol-dark-factory-foundation.md`, this audit | Proof doc includes diagrams, commands, E2E IDs, and remaining caveats | Done |
| Material Patrol architecture is recorded in an app ADR | `os-apps/paw-patrol/adrs/0001-patrol-controlled-dark-factory.md`, `crates/temperpaw/tests/paw_patrol_foundation.rs` | Added an app-scoped accepted ADR covering the Patrol-owned entity set, Temper-native trigger/WASM/Cedar boundaries, Mac mini worker, risk gates, proof requirements, rejected separate factory/quality/harness apps, and verification trail. Test `paw_patrol_dark_factory_architecture_is_recorded_in_app_adr` ratchets this AGENTS.md requirement | Done |
| Temper Cedar supports resource ABAC needed by Patrol policies | Temper worktree `crates/temper-authz/src/engine/*`; TemperPaw `crates/temperpaw/Cargo.toml` | `test_resource_attribute_access_in_policy` passes; resource attributes are now attached to Cedar resource entities. The Temper fix is pushed at `557db7f30814801ad42d28e92725d007c6ce7732`, rebased on current Temper main, and TemperPaw is pinned to that portable git revision | Done in sibling Temper branch |
| Temper dependency handoff is portable | TemperPaw `crates/temperpaw/Cargo.toml`, `Cargo.lock` | The temporary local path patch was removed. TemperPaw now resolves Temper crates from `https://github.com/nerdsane/temper.git` at `557db7f30814801ad42d28e92725d007c6ce7732`; `cargo check --locked -p temperpaw -p paw-codex-worker` passes | Done |
| Worker runbook is usable from a worktree | `crates/paw-codex-worker/README.md` | Local test and deterministic smoke commands use `REPO_ROOT="$(pwd)"` and fixture paths under the current checkout; README calls out `jq`, fake Codex, stop/cleanup, doctor, launchd-plist flow, and the worker invariant that local Codex work must have a Patrol-assigned `branch_name` or `worktree_path` | Done |
| One-command acceptance proof is available | `crates/paw-codex-worker/scripts/paw-patrol-acceptance.sh`, `crates/paw-codex-worker/README.md` | Acceptance harness has `quick` and `live` modes; quick collects syntax, CI action runtime, fmt, diff, cargo check, foundation, worker-test, production-preflight, Railway-discovery preflight, GitHub PR cutover preflight, and preflight-diff evidence into `index.html`, `summary.json`, `proof.md`, `operator-handoff.md`, and `acceptance.log`; live also runs deterministic, webhook, repo-sweep/brief, production-readiness, and production observe-only smokes into stable subdirectories and embeds available SVG proof visuals in the browser-readable index | Done |
| Acceptance proof is tied to the exact checkout | `paw-patrol-acceptance.sh`, `summary.json`, `proof.md`, `index.html` | Acceptance summary/proof/index include `git_head`, `git_branch`, `git_status_short`, and `git_clean` so agents and humans can verify which checkout produced the proof bundle | Done |
| Live smoke scripts avoid local port collisions | `deterministic-smoke.sh`, `webhook-intake-smoke.sh`, `repo-sweep-brief-smoke.sh`, `production-readiness-smoke.sh`, `production-observe-only-smoke.sh` | Acceptance found an actual collision on the implicit webhook trigger port. The scripts now choose a base port only when both the OData port and `PORT + 12` webhook trigger port are free; foundation test `live_smoke_scripts_choose_non_colliding_odata_and_webhook_ports` passes | Done |
| Deterministic smoke can be run as one command | `crates/paw-codex-worker/scripts/deterministic-smoke.sh` | Script boots local TemperPaw, submits a PatrolRequest, starts fake local worker, polls WorkCycle/FactoryCase/Review/Evaluation/Proof states, writes a proof bundle with `summary.json`, `proof.json`, `proof.md`, and `proof.svg`, prints JSON entity summary, and cleans up the temporary worktree/branch | Done |
| Webhook intake smoke can be run as one command | `crates/paw-codex-worker/scripts/webhook-intake-smoke.sh`, `crates/paw-codex-worker/README.md` | Script boots local TemperPaw, posts to `/triggers/webhook/patrol-request`, `/triggers/webhook/patrol-datadog`, `/triggers/webhook/patrol-github`, and `/triggers/webhook/patrol-discord`, waits for WebhookEvent Processed plus PatrolRequest/Signal Linked states, and writes a visual intake proof bundle | Done |
| Repo sweep and daily brief smoke can be run as one command | `crates/paw-codex-worker/scripts/repo-sweep-brief-smoke.sh`, `crates/paw-codex-worker/README.md` | Script boots local TemperPaw, starts RepoGraphSnapshot.StartScan, runs the local worker repo scan, waits for review/evaluation/proof closeout, starts DailyBrief, and writes `summary.json`, `repo-graph.json`, `proof.json`, `proof.md`, `proof.svg`, and `daily-brief.svg` | Done |
| Mac mini production activation is checkable | `crates/paw-codex-worker/scripts/production-readiness.sh`, `production-readiness-smoke.sh`, `README.md` | Script builds the release worker, runs `paw-codex-worker doctor`, renders launchd only with `WRITE_LAUNCHD_PLIST=1`, installs launchd only with `INSTALL_LAUNCHD=1`, defaults execution off, and does not print `WORKER_TOKEN`. Live readiness smoke proved doctor OData/event-stream checks and plist rendering against local TemperPaw without loading launchd | Done locally; production inputs still human-blocked |
| Production human blockers are machine-readable and visual | `crates/paw-codex-worker/scripts/production-preflight.sh`, `crates/paw-codex-worker/scripts/production-preflight-railway-discovery-smoke.sh`, `crates/paw-codex-worker/scripts/paw-patrol-acceptance.sh`, `docs/runbooks/paw-patrol-production-cutover.md` | Non-mutating preflight writes `summary.json`, `proof.md`, `operator-handoff.md`, `gates.tsv`, `preflight.svg`, and `railway-candidates.json`; the latest Railway-enabled read-only run proves Railway CLI login works but the checkout is not linked to a Railway project/service, and captures 3 visible project/service candidates. It records current `human_blockers`, including missing `TEMPER_URL`, missing `WORKER_TOKEN`, missing `PATROL_OPERATOR_TOKEN`, missing webhook secrets, launchd not loaded, Railway project not linked, Temper PR #216 ready for review but unmerged, and TemperPaw PR #218 clean/green but unmerged without `CONFIRM_TEMPERPAW_PR_OK=1` | Done locally; blockers require human input |
| Production preflight gates the Patrol PR itself | `production-preflight.sh`, `production-preflight-github-smoke.sh`, `paw-patrol-acceptance.sh`, `docs/runbooks/paw-patrol-production-cutover.md` | Red-green test added `production-preflight-github-smoke.sh`. The smoke uses fake GitHub state to prove clean/green but unmerged PR #218 blocks production cutover unless `CONFIRM_TEMPERPAW_PR_OK=1`; quick/live acceptance now include this proof and the real preflight records `github:temperpaw_pr_218` as a blocker while this PR remains unmerged | Done locally; blocker requires human merge/approval |
| Preflight reruns are diffable before cutover | `crates/paw-codex-worker/scripts/production-preflight-diff.sh`, `crates/paw-codex-worker/scripts/production-preflight-diff-smoke.sh`, `docs/runbooks/paw-patrol-production-cutover.md` | Non-mutating diff compares two preflight `summary.json` files and writes `summary.json`, `proof.md`, and `preflight-diff.svg` with resolved blockers, new blockers, unchanged blockers, changed gates, and Railway candidate drift; smoke proves resolved/new/unchanged blocker detection and candidate-added detection | Done locally |
| Production observe-only proof is executable | `crates/paw-codex-worker/scripts/production-observe-only.sh`, `production-observe-only-smoke.sh`, `README.md`, `docs/runbooks/paw-patrol-production-cutover.md` | Guarded script refuses production writes unless `ALLOW_PRODUCTION_WRITE=1` and `CONFIRM_PAW_CODEX_ENABLE_EXECUTION_0=1`; local smoke booted TemperPaw, ran the worker in `PAW_CODEX_ENABLE_EXECUTION=0`, created a RepoGraphSnapshot, waited for WorkerRun Done, ReviewRun Approved, EvaluationRun Passed, ProofPacket Ready, DailyBrief Ready, and wrote `summary.json`, `proof.md`, `observe-only.svg`, `proof-packet.svg`, and `daily-brief.svg` | Done locally; production run still needs human tokens/launchd |
| Mac mini Codex auth/session is checkable before launchd | `crates/paw-codex-worker/src/doctor.rs`, `production-readiness.sh`, `production-readiness-smoke.sh`, `README.md`, `docs/runbooks/paw-patrol-production-cutover.md` | `PAW_CODEX_DOCTOR_EXEC_SMOKE=1` makes `paw-codex-worker doctor` run a tiny `codex exec --skip-git-repo-check` prompt in a temporary directory before launchd is rendered/installed. The guarded local readiness smoke now proves `codex_exec_smoke: "doctor pass"` while `PAW_CODEX_ENABLE_EXECUTION=0` remains observe-only | Done locally; production real-Codex smoke still needs Railway token/user approval |
| Production cutover blockers are mapped to gates | `docs/runbooks/paw-patrol-production-cutover.md`, `crates/paw-codex-worker/README.md` | Runbook gives a visual cutover map, required human inputs, Railway/Cedar/launchd/webhook gates, exact commands, evidence to capture, and rollback; foundation test `production_cutover_runbook_maps_every_human_blocker_to_a_gate` passes | Done |
| CI covers Patrol worker and WASM gates | `.github/workflows/ci.yml`, `crates/temperpaw/tests/paw_patrol_foundation.rs` | CI now checks `paw-codex-worker` clippy/test/check, validates worker smoke script syntax, and builds `paw-ingest` plus `paw-patrol` WASM modules. Foundation test `ci_covers_paw_patrol_worker_and_wasm_gates` passes | Done locally |
| CI action runtime deprecation is ratcheted | `.github/workflows/*.yml`, `crates/paw-codex-worker/scripts/ci-actions-runtime-smoke.sh` | GitHub workflows use `actions/checkout@v6.0.2` and `actions/setup-node@v6.4.0`; CI and quick acceptance run a smoke that fails if checkout/setup-node drift back to deprecated v1-v4 majors | Done locally |
| Final proof readiness has direct unit coverage | `.github/workflows/ci.yml`, `os-apps/paw-patrol/wasm/review_gate_lifecycle/src/lib.rs` | `review_gate_lifecycle` unit tests assert final ProofPacket summaries remove draft pending labels, final visuals say Review/Evaluation passed, and the changed-files/dependency map no longer carries pending placeholders. CI now runs this WASM test manifest | Done locally |
| Worker source is readable enough for Patrol to maintain | `crates/paw-codex-worker/src/*.rs`, `crates/temperpaw/tests/paw_patrol_foundation.rs` | The worker was split into topical source files; `paw_codex_worker_sources_stay_under_giant_module_budget` asserts every worker source file stays below the repo-health giant-module threshold | Done |
| Fresh build does not rely on generated clutter | `.gitignore`, build scripts, status cleanup | WASM build scripts exist; generated `target` dirs and transient lockfiles are removed from the patch surface | Done |
| Actual production Railway/Mac mini activation | Railway URL/token, Mac mini launchd environment, production webhook secrets | Not available in this thread. I did not load launchd or wire production endpoints without credentials/approval | Human-blocked |

## Current Verification Output

Fresh commands run during this audit:

```text
cargo fmt --check --all
  passed

git diff --check
  passed

bash -n crates/paw-codex-worker/scripts/deterministic-smoke.sh
  passed

bash -n crates/paw-codex-worker/scripts/repo-sweep-brief-smoke.sh
  passed

bash -n crates/paw-codex-worker/scripts/webhook-intake-smoke.sh
  passed

bash -n crates/paw-codex-worker/scripts/production-readiness.sh
  passed

bash -n crates/paw-codex-worker/scripts/production-preflight.sh
  passed

bash -n crates/paw-codex-worker/scripts/production-preflight-diff.sh
  passed

bash -n crates/paw-codex-worker/scripts/production-preflight-diff-smoke.sh
  passed

bash -n crates/paw-codex-worker/scripts/production-preflight-railway-discovery-smoke.sh
  passed

bash -n crates/paw-codex-worker/scripts/production-observe-only.sh
  passed

bash -n crates/paw-codex-worker/scripts/production-observe-only-smoke.sh
  passed

bash -n crates/paw-codex-worker/scripts/production-readiness-smoke.sh
  passed

bash -n crates/paw-codex-worker/scripts/ci-actions-runtime-smoke.sh
  passed

crates/paw-codex-worker/scripts/ci-actions-runtime-smoke.sh
  passed
  Output: GitHub action runtime versions are current enough

crates/paw-codex-worker/scripts/paw-patrol-acceptance.sh quick
  passed
  Proof bundle: /tmp/paw-patrol-acceptance-quick-worktree-guard-current
  Browser index: /tmp/paw-patrol-acceptance-quick-worktree-guard-current/index.html
  Passed gates: 24
  Production preflight visual:
    /tmp/paw-patrol-acceptance-quick-worktree-guard-current/production-preflight/preflight.svg
  Production preflight operator handoff:
    /tmp/paw-patrol-acceptance-quick-worktree-guard-current/production-preflight/operator-handoff.md
  Railway discovery candidates:
    /tmp/paw-patrol-acceptance-quick-worktree-guard-current/production-preflight-railway-discovery-smoke/railway-candidates.json
  Preflight diff visual:
    /tmp/paw-patrol-acceptance-quick-worktree-guard-current/production-preflight-diff-smoke/preflight-diff.svg
  GitHub preflight gate:
    /tmp/paw-patrol-acceptance-quick-worktree-guard-current/production-preflight-github-smoke/summary-without-confirm.json
    /tmp/paw-patrol-acceptance-quick-worktree-guard-current/production-preflight-github-smoke/summary-with-confirm.json

crates/paw-codex-worker/scripts/paw-patrol-acceptance.sh live
  passed
  Proof bundle: /tmp/paw-patrol-acceptance-live-worktree-guard-current
  Browser index: /tmp/paw-patrol-acceptance-live-worktree-guard-current/index.html
  Passed gates: 29
  Visuals embedded: deterministic-smoke/proof.svg,
    webhook-intake-smoke/webhook-intake.svg,
    repo-sweep-brief-smoke/proof.svg,
    repo-sweep-brief-smoke/daily-brief.svg,
    production-preflight/preflight.svg,
    production-preflight-railway-discovery-smoke/preflight.svg,
    production-preflight-diff-smoke/preflight-diff.svg,
    production-observe-only/observe-only.svg
  GitHub webhook evidence: webhook-intake-smoke/github-webhook-event.json,
    webhook-intake-smoke/github-signal.json, GitHub Signal Linked
  Default schedule evidence:
    repo-sweep-brief-smoke/patrol-schedule.json,
    PatrolSchedule Active

cargo check --locked -p temperpaw -p paw-codex-worker
  passed

cargo test --locked -p temperpaw --test paw_patrol_foundation -- --nocapture
  33 passed

cargo test --locked -p paw-codex-worker -- --nocapture
  20 passed

cargo test --locked -p paw-codex-worker local_worker_claims_only_configured_local_codex_runs -- --nocapture
  passed; proves local Codex WorkerRuns without a branch/worktree assignment are not claimable

cargo test --locked -p paw-codex-worker worker_proof_text_does_not_call_assigned_worktree_current_checkout -- --nocapture
  passed; proves assigned worktree proof/review text does not label the main checkout

cargo test --manifest-path os-apps/paw-patrol/wasm/worker_run_lifecycle/Cargo.toml -- --nocapture
  1 passed

cargo test --manifest-path os-apps/paw-patrol/wasm/review_gate_lifecycle/Cargo.toml -- --nocapture
  3 passed

cargo clippy --locked -p temperpaw -p paw-codex-worker --all-targets -- -D warnings
  passed

cargo test --manifest-path os-apps/paw-patrol/wasm/patrol_request_router/Cargo.toml -- --nocapture
  2 passed

env -u TEMPER_URL -u WORKER_TOKEN crates/paw-codex-worker/scripts/production-readiness.sh
  failed cleanly with: [paw-codex-production] TEMPER_URL is required

crates/paw-codex-worker/scripts/deterministic-smoke.sh
  passed
  Allowed worker: mac-mini-codex-prod
  FactoryCase: en-019df8aa-8af4-7362-ad1b-68925d1474e0 Complete
  WorkCycle: wc-019df8aa-8afc-7fc2-b585-7983298013a7 Complete
  WorkerRun: en-019df8aa-8b04-7721-8965-ce8994acf1bb Done
  ReviewRun: en-019df8aa-9663-7cf1-9520-a93328a7b7b1 Approved
  EvaluationRun: en-019df8aa-966b-7572-b27e-42ce3c0ffca3 Passed
  ProofPacket: en-019df8aa-965a-7042-b0a0-3d06ccddb39f Ready
  Proof JSON includes: "e2e_gate": "passed"
  Proof bundle: /tmp/paw-patrol-smoke-e2e-gates-current

crates/paw-codex-worker/scripts/repo-sweep-brief-smoke.sh
  passed
  Allowed worker: mac-mini-codex-prod
  Default PatrolSchedule: patrol-default-daily-maintenance Active
  Default PatrolSchedule next_run_at: 2026-05-06T15:04:42Z
  RepoGraphSnapshot: en-019df8ab-8509-7ce1-ae37-e34da6338de5 Ready
  WorkCycle: wc-019df8ab-8620-7d42-9486-48f33891327a Complete
  WorkerRun: en-019df8ab-863b-7f60-9ba3-38cdf17572ae Done
  ReviewRun: en-019df8ab-8de4-70c0-9ddd-043b8dca5d3d Approved
  EvaluationRun: en-019df8ab-8ded-7e02-a8fb-bb2ffab0a5b9 Passed
  ProofPacket: en-019df8ab-8ddc-7331-ab5c-17c543e65590 Ready
  DailyBrief: en-019df8ab-9205-7b41-93a2-4774110807d7 Ready
  Findings: 51
  Proof bundle: /tmp/paw-patrol-repo-smoke-e2e-gates-current

crates/paw-codex-worker/scripts/webhook-intake-smoke.sh
  passed
  Request WebhookEvent: en-019df7cf-4fc0-7420-a807-582342f3d09c Processed
  PatrolRequest: en-019df7cf-52a1-7a32-9818-3d0e01fa8cc4 Linked
  Datadog WebhookEvent: en-019df7cf-5853-7c60-b9e6-68dbe34b436a Processed
  Datadog Signal: en-019df7cf-5887-72d3-b7c3-16c07238b9cd Linked
  GitHub WebhookEvent: en-019df7cf-5cbe-7de0-b106-c36bb7d100e4 Processed
  GitHub Signal: en-019df7cf-5cf2-73a0-84aa-56d8b0929410 Linked
  Discord WebhookEvent: en-019df7cf-6124-7d73-9349-11d76b1272ae Processed
  Discord Signal: en-019df7cf-6157-7773-bb73-1a6c957756ed Linked
  Request WorkCycle: wc-019df7cf-55be-7592-9f23-937c5daf8277
  Datadog WorkCycle: wc-019df7cf-5a2a-7f93-93ec-25ccf4a64c70
  GitHub WorkCycle: wc-019df7cf-5d3f-7373-b71f-811cac54a315
  Discord WorkCycle: wc-019df7cf-61a4-7c93-ba5d-3c9cbc9096fb
  Proof bundle: /tmp/paw-patrol-webhook-smoke-proof-4396-56013

crates/paw-codex-worker/scripts/production-readiness-smoke.sh
  passed
  Temper URL: http://127.0.0.1:4336
  Worker ID: mac-mini-codex-prod
  Execution enabled: false
  Doctor checks: OData pass, event_stream pass, fake Codex pass, codex_exec_smoke pass
  Launchd rendered: /tmp/paw-patrol-production-readiness-proof-4336-96560/com.temperpaw.paw-codex-worker.plist
  Launchd installed: false
  Token printed to readiness log: false
  Proof bundle: /tmp/paw-patrol-production-readiness-proof-4336-96560

crates/paw-codex-worker/scripts/production-preflight.sh
  passed as a non-mutating readiness inventory
  Status: blocked
  Human blockers: 12
  Railway candidates captured: 3
  Proof bundle: /tmp/paw-patrol-production-preflight-current-railway
  Visual summary: /tmp/paw-patrol-production-preflight-current-railway/preflight.svg
  Operator handoff: /tmp/paw-patrol-production-preflight-current-railway/operator-handoff.md
  Candidate list: /tmp/paw-patrol-production-preflight-current-railway/railway-candidates.json
  Key blockers recorded: missing TEMPER_URL, missing WORKER_TOKEN,
    missing PATROL_OPERATOR_TOKEN, unconfirmed local_codex_worker_id, missing
    production webhook secrets, launchd plist not rendered, launchd worker not
    loaded, Railway project not linked, Temper PR #216 ready for review but
    unmerged, and TemperPaw PR #218 clean/green but unmerged without
    CONFIRM_TEMPERPAW_PR_OK

crates/paw-codex-worker/scripts/production-observe-only-smoke.sh
  passed
  RepoGraphSnapshot: en-019df804-0260-7642-8515-54014444eece Ready
  WorkCycle: wc-019df804-0386-7e91-ac2d-d4345885e989 Complete
  WorkerRun: en-019df804-038f-7080-840f-9d8e0a0198fa Done
  ReviewRun: en-019df804-05cd-7ad1-8e1a-a42c713f4746 Approved
  EvaluationRun: en-019df804-05d4-74c3-b78f-96f48d2a6bc4 Passed
  ProofPacket: en-019df804-05c4-7030-8799-11dfabfdfde4 Ready
  DailyBrief: en-019df804-0b71-78e3-8791-d3b6d2540ce6 Ready
  Worker execution enabled: false
  Proof bundle: /tmp/paw-patrol-observe-smoke-proof-4886-21698
  Visual summary: /tmp/paw-patrol-observe-smoke-proof-4886-21698/observe-only.svg

codex exec --skip-git-repo-check "PAW_CODEX_DOCTOR_EXEC_SMOKE: ..."
  passed under the current Mac user
  Output included: PAW_CODEX_DOCTOR_EXEC_OK

git ls-remote --heads origin codex/cedar-resource-attrs
  557db7f30814801ad42d28e92725d007c6ce7732 refs/heads/codex/cedar-resource-attrs
```

The latest live local E2E proof bundle at
`/tmp/paw-patrol-acceptance-live-worktree-guard-current` booted local TemperPaw with
`TEMPERPAW_WASM_STARTUP_POLICY=build`, submitted a PatrolRequest, ran the fake
local Codex worker, and observed:

```json
{
  "patrol_request": "Linked",
  "worker_run": "Done",
  "review_run": "Approved",
  "evaluation_run": "Passed",
  "proof_packet": "Ready",
  "work_cycle": "Complete",
  "factory_case": "Complete"
}
```

The deterministic proof inside that bundle also records the concrete
changed-files map from worker git evidence:

```json
{
  "branch_name": "codex/paw-patrol-7fdf9743",
  "changed_files": [".paw-fake-codex-implementation"],
  "evidence_source": "WorkerRun result_summary git-status block",
  "review_status": "approved",
  "evaluation_status": "passed",
  "proof_status": "ready"
}
```

## Coverage Judgment

The local software loop is implemented and usable:

- users or manager agents can submit PatrolRequests;
- machine signals and webhooks can become Signals;
- Patrol creates PM Issues, FactoryCases, WorkCycles, WorkerRuns, reviews,
  evaluations, ProofPackets, findings, repo snapshots, schedules, and briefs;
- the local worker can claim, execute, review, evaluate, and self-report;
- proof artifacts are human-readable and visual;
- quality/security cleanup is part of the Patrol loop;
- high-risk work is gated by entity state and Cedar policy.

The full production loop is not activated in this thread because it requires
human-provided production inputs:

- Railway TemperPaw URL, worker token, and Patrol operator token;
- confirmation of the worker principal/identity to register in production;
- Mac mini launchd installation approval;
- production Datadog/Discord/GitHub webhook secret configuration;
- confirmation of the Railway project/service from
  `/tmp/paw-patrol-production-preflight-current-railway/railway-candidates.json`;
- merging the sibling Temper Cedar fix so TemperPaw can eventually return from
  the temporary git-revision pin to the normal Temper mainline.
- merging TemperPaw PR #218, or explicitly approving its current clean/green
  head for production cutover with `CONFIRM_TEMPERPAW_PR_OK=1`.

## Audit Decision

Do not mark the overall goal complete yet if "fully working and usable" means
production Railway plus always-on Mac mini operation. The local implementation
is complete and verified, but production activation is human-blocked.

Next human input needed:

1. The production TemperPaw/Railway URL.
2. The worker token or approved way to mint/register it.
3. The Patrol operator token for guarded production observe-only proof.
4. Confirmation that the production worker principal is
   `mac-mini-codex-prod`.
5. Production Datadog, Discord, and GitHub webhook secrets.
6. The Railway project/service to link this checkout to, now narrowed to the
   candidates in
   `/tmp/paw-patrol-production-preflight-current-railway/railway-candidates.json`.
7. Approval to render and load the generated launchd plist on the Mac mini.
8. Decision on when to merge the Temper `codex/cedar-resource-attrs` fix and
   remove the temporary git-revision pin from TemperPaw.
9. Decision on when to merge TemperPaw PR #218, or explicit approval to deploy
   its clean/green head before merge with `CONFIRM_TEMPERPAW_PR_OK=1`.
