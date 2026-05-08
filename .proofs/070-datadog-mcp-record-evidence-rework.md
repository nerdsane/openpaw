# Datadog MCP RecordEvidence Rework Proof

Date: 2026-05-08

WorkCycle: `wc-019e05f1-151c-74c2-a7f3-9b669e5e4ea1`
PatrolRun: `en-019e05f1-0e0d-77e0-a94a-b2e5f65f298f`

## Reviewer Gap

The prior rework fixed the `Datadog MCP Patrol agent` classifier path, but live
state still showed the target PatrolRun in `Running` with no evidence payload.
That meant the Temper-native self-reporting contract was not yet proven for the
target run.

Reviewer follow-up also noted that `datadog-patrol-smoke.sh` was not
read-only-safe for local review, because it copied WASM artifacts into the
assigned worktree before booting the local control plane.

## Live Self-Report Evidence

The target PatrolRun was checked read-only before self-reporting:

- Status: `Running`
- WorkerRun: `en-019e05f1-1585-7892-a5f0-7248021ce1bf`
- Evidence JSON length: `0`
- ProofPacket: empty

After a fresh read-only Datadog MCP investigation, the agent dispatched the
intended Patrol action:

```text
PatrolRun.RecordEvidence -> HTTP 200
```

The target PatrolRun then reached:

```json
{
  "id": "en-019e05f1-0e0d-77e0-a94a-b2e5f65f298f",
  "status": "Complete",
  "evidence_json_len": 6529,
  "signal_ids": "[]",
  "observability_finding_ids": "[]",
  "factory_case_ids": "[]",
  "work_cycle_ids": "[]",
  "proof_packet_id": "en-019e0642-5d67-7991-8f76-d723d21de7e9",
  "completed_at": "2026-05-08T06:24:33Z"
}
```

The created proof packet reached:

```json
{
  "id": "en-019e0642-5d67-7991-8f76-d723d21de7e9",
  "status": "Ready",
  "worker_run_id": "en-019e05f1-1585-7892-a5f0-7248021ce1bf",
  "work_cycle_id": "",
  "summary_markdown_len": 3517,
  "proof_json_len": 6647,
  "residual_risks_len": 812
}
```

Read-only OData recheck on 2026-05-08 confirmed:

```json
{
  "patrol_run": {
    "status": "Complete",
    "worker_run_id": "en-019e05f1-1585-7892-a5f0-7248021ce1bf",
    "proof_packet_id": "en-019e0642-5d67-7991-8f76-d723d21de7e9",
    "evidence_json_len": 6529
  },
  "worker_run": {
    "status": "Done",
    "work_cycle_id": "wc-019e05f1-151c-74c2-a7f3-9b669e5e4ea1"
  },
  "proof_packet": {
    "status": "Ready",
    "summary_markdown_len": 3517,
    "proof_json_len": 6647
  },
  "work_cycle": {
    "status": "Failed"
  },
  "review_runs": [
    {
      "status": "ChangesRequested",
      "verdict": "request_changes"
    }
  ],
  "evaluation_runs": []
}
```

The evidence packet intentionally opened no duplicate Signals,
ObservabilityFindings, FactoryCases, or WorkCycles. A later completed patrol,
`en-019e0619-5807-71c3-9466-fc714f1087eb`, already opened current Datadog MCP
ObservabilityFindings for the same active issue set.

## Read-Only-Safe Local E2E

`datadog-patrol-smoke.sh` now prepares a temporary runtime app tree under
`/tmp/paw-patrol-datadog-smoke-runtime-*`, starts TemperPaw from that directory
with `cargo run --manifest-path "$ROOT/Cargo.toml"`, and leaves the assigned
worktree untouched by WASM artifact copies.

The script also refuses an inherited non-local `TEMPER_URL` unless
`ALLOW_REMOTE_TEMPER_URL=1` is set. Guard proof:

```text
TEMPER_URL=https://example.invalid crates/paw-codex-worker/scripts/datadog-patrol-smoke.sh
rc=1
[paw-patrol-datadog-smoke] refusing non-local TEMPER_URL=https://example.invalid; unset TEMPER_URL for local smoke or set ALLOW_REMOTE_TEMPER_URL=1 for an intentional remote run
```

Local E2E was then run with production OData env vars explicitly unset:

```text
env -u TEMPER_URL -u TEMPER_API_KEY -u TEMPER_TENANT \
  PROOF_DIR=/tmp/paw-patrol-datadog-smoke-proof-rework-local-20260508 \
  crates/paw-codex-worker/scripts/datadog-patrol-smoke.sh
```

Result:

```json
{
  "statuses": {
    "patrol_run": "Complete",
    "worker_run": "Done",
    "observability_finding_source": "datadog_mcp",
    "work_cycle": "AwaitingHumanStartApproval",
    "proof_packet": "Ready"
  },
  "counts": {
    "signals": 1,
    "observability_findings": 1,
    "factory_cases": 1,
    "work_cycles": 1
  },
  "entities": {
    "patrol_run": "en-019e064a-b5f3-7a91-8c0d-02a92255e7f4",
    "worker_run": "en-019e064a-ba1a-7dc1-a5c7-f8728b7a2ff2",
    "signal": "en-019e064a-c1fe-7482-8c90-72d03b2250e0",
    "observability_finding": "en-019e064a-c237-7f73-b1d9-4b1cd54c0683",
    "factory_case": "en-019e064a-c253-7c41-9c24-4fd088db8f5b",
    "work_cycle": "wc-019e064a-c27a-7310-948e-7a598a54c50b",
    "proof_packet": "en-019e064a-c2d8-7932-be50-1fa2ea1dedbf"
  },
  "runtime": {
    "root": "/tmp/paw-patrol-datadog-smoke-runtime-4133-cZb3VN",
    "os_apps": "/tmp/paw-patrol-datadog-smoke-runtime-4133-cZb3VN/os-apps"
  }
}
```

Proof bundle: `/tmp/paw-patrol-datadog-smoke-proof-rework-local-20260508`

The first post-change smoke attempt inherited a production `TEMPER_URL` before
the guard existed and created production PatrolRun
`en-019e0647-3e79-7c60-9591-f7a43e4f7156` plus fixture fanout entities. No
Datadog, code, or production configuration was mutated, but the extra Patrol
entities are a production audit artifact and should be treated as cleanup
requiring human approval if cleanup is desired.

## Datadog Evidence Scope

- Monitors: current active/warn monitor search found `[Temper] Profiler Uploads
  Stalled` and `[Temper] State Timeout Reset Rate Drop`; warning search found
  `[Temper] Log Error Rate Spike`; no-data search found `[Temper] Startup Time
  Regression` and other stale/deep-sci-fi monitors. The initial combined status
  query was rejected by Datadog monitor search syntax and was retried as
  separate status queries.
- Logs: Datadog Logs SQL and pattern clustering for
  `service:openpaw status:error` over the last hour returned zero rows/patterns.
  Log-derived metrics were still visible over the inspected four-hour window:
  `openpaw.logs.errors` sum `17` and `openpaw.logs.warns` sum `11871`.
- Traces/APM: `service:openpaw status:error` returned `1221` spans over four
  hours. Top resources were `OpenAICodexAuth.EnsureFresh` (`705`),
  `dispatch.handle_wasm_failure` (`117`), OData `POST` (`113`),
  `wasm:finalize_spawned_session` (`71`), and `Session.RecordResult` (`41`).
  A bounded sample showed transition errors and OData `409` spans. AppSec query
  `@appsec.security_activity:* service:openpaw` returned zero buckets.
- Metrics: WASM default-timeout fallback metric summed `4691` over the inspected
  four-hour window; integration silent-exit metric summed `329` with latest bins
  at zero; active actor samples dipped to `0` in one bin then recovered. Direct
  profiler upload and Executing state-timeout reset metric queries returned no
  data, matching the active monitor concerns.
- Incidents/events: no active or stable Datadog incidents were returned. Recent
  relevant Datadog events were monitor notifications from alert/datadog sources,
  including Active Entities Drop trigger/recovery, WASM default-timeout warning,
  profiler upload stalled, state-timeout reset drop, and recovered silent-exit
  events.
- Dashboards/services: dashboard `mn4-k3k-i66` (`TemperPaw - Platform Overview`)
  exists and covers OpenPaw platform, APM, runtime, persistence, Authz/WASM,
  state liveness, session, actor, and profiler surfaces. Service catalog lists
  `openpaw` and `temperpaw`.

## Verification

```text
git diff --check
PASS

cargo fmt --check -p paw-codex-worker
PASS

bash -n crates/paw-codex-worker/scripts/datadog-patrol-smoke.sh
PASS

cargo test -p paw-codex-worker datadog_patrol_classifier_ignores_followup_and_rework_prompts -- --nocapture
PASS: 1 passed

cargo test --locked -p paw-codex-worker
PASS: 44 passed

cargo test --locked -p temperpaw --test paw_patrol_foundation datadog_patrol_smoke_is_worktree_read_only_safe -- --nocapture
PASS: 1 passed

cargo test -p temperpaw --test paw_patrol_foundation datadog_observability_patrol_run_uses_temper_state_and_creates_work -- --nocapture
PASS: 1 passed

cargo test --locked -p temperpaw --test paw_patrol_foundation -- --nocapture
PASS: 52 passed

env -u TEMPER_URL -u TEMPER_API_KEY -u TEMPER_TENANT PROOF_DIR=/tmp/paw-patrol-datadog-smoke-proof-rework-local-20260508 crates/paw-codex-worker/scripts/datadog-patrol-smoke.sh
PASS: PatrolRun Complete, WorkerRun Done, ProofPacket Ready, one Signal/Finding/Case/WorkCycle fanout
```

## Residual Risks

- Raw Datadog Logs returned no `service:openpaw status:error` rows in the last
  hour, so four-hour APM spans and log-derived metrics remain the stronger
  current evidence for runtime error patterns.
- Railway infrastructure health remains inferred indirectly from OpenPaw service
  telemetry; OpenPaw service-tagged host inventory rows were not returned.
- The inherited-production-URL smoke attempt created extra fixture Patrol
  entities in production before the guard was added. Cleanup should be
  human-approved because it would mutate production Patrol state.
- The proof/local-smoke change does not alter architecture. No ADR is required.
