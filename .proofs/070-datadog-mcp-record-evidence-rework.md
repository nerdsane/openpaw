# Datadog MCP RecordEvidence Rework Proof

Date: 2026-05-08

WorkCycle: `wc-019e05f1-151c-74c2-a7f3-9b669e5e4ea1`
PatrolRun: `en-019e05f1-0e0d-77e0-a94a-b2e5f65f298f`

## Reviewer Gap

The prior rework fixed the `Datadog MCP Patrol agent` classifier path, but live
state still showed the target PatrolRun in `Running` with no evidence payload.
That meant the Temper-native self-reporting contract was not yet proven for the
target run.

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

The evidence packet intentionally opened no duplicate Signals,
ObservabilityFindings, FactoryCases, or WorkCycles. A later completed patrol,
`en-019e0619-5807-71c3-9466-fc714f1087eb`, already opened current Datadog MCP
ObservabilityFindings for the same active issue set.

## Datadog Evidence Scope

- Monitors: current active/warn monitor search found `[Temper] Profiler Uploads
  Stalled`, `[Temper] Log Error Rate Spike`, `[Temper] State Timeout Reset Rate
  Drop`, and `[Temper] Active Entities Drop`.
- Logs: Datadog Logs MCP remained unavailable for this org/query path. DDSQL
  returned Datadog-side `500`; pattern clustering returned `503 unavailable`.
  Log-derived metrics were still visible: `openpaw.logs.errors` sum `17` and
  `openpaw.logs.warns` sum `11896` over the inspected four-hour window.
- Traces/APM: `service:openpaw status:error` showed
  `OpenAICodexAuth.EnsureFresh` transition errors, WASM failure handling errors,
  and OData POST `409` spans. AppSec query
  `@appsec.security_activity:* service:openpaw` returned zero buckets.
- Metrics: WASM default-timeout fallback metric summed `4691` over the inspected
  four-hour window; integration silent-exit metric summed `335` with latest bins
  at zero; active actor samples stayed at least `1` in the sampled one-hour
  window. Direct profiler upload and Executing state-timeout reset metric
  queries returned no data, matching the active monitor concerns.
- Incidents/events: no active or stable Datadog incidents were returned. Recent
  relevant Datadog events were monitor notifications from alert/datadog sources.
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

cargo test -p paw-codex-worker datadog_patrol_classifier_ignores_followup_and_rework_prompts -- --nocapture
PASS: 1 passed

cargo test -p temperpaw --test paw_patrol_foundation datadog_observability_patrol_run_uses_temper_state_and_creates_work -- --nocapture
PASS: 1 passed
```

## Residual Risks

- Raw Datadog Logs root-cause proof is still limited by Datadog-side MCP errors.
- Railway infrastructure health remains inferred indirectly from OpenPaw service
  telemetry; OpenPaw service-tagged host inventory rows were not returned.
- The proof-only change does not alter architecture. No ADR is required.
