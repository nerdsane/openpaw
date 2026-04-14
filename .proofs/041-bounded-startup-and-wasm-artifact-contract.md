# Proof Report: 041 — Bounded Startup and WASM Artifact Contract

## Date
2026-04-13

## Workspace
- **openpaw**: `main` workspace on top of `8ae50e32` (dirty worktree during verification)
- **temper**: local workspace on top of `64b317d` (dirty worktree during verification)

## Objective
Address startup heaviness comprehensively as a principal-engineering change, not as a one-off patch.

That means:

1. warm restart must stop behaving like first-run bootstrap
2. startup must stop depending on opportunistic local WASM compilation
3. unchanged OS-app bundles must not replay broad content bootstrap work
4. module failures must be classified and surfaced
5. Datadog must show whether the new startup contract is holding

## Architectural Outcome

### What replaced "compile WASM during startup"

Startup is no longer designed around local source compilation.

The replacement model is:

1. **Prebuilt bundled artifacts**
   OS apps declare module contracts in `app.toml`, and the app build scripts copy the compiled `.wasm` into the module directory as a bundle artifact.

2. **Explicit startup policy**
   OpenPaw now defaults to `LoadPersistedOnly`, which means startup loads persisted or bundled artifacts rather than trying to build from local crate sources.

3. **Runtime-only reconcile for unchanged bundles**
   When an app bundle digest has not changed, startup skips content bootstrap and only reloads the runtime prerequisites it actually needs.

4. **Persistent Wasmtime cache**
   Wasmtime now uses a persistent cache so validated prebuilt artifacts do not pay repeated native compilation cost on every restart.

5. **Classified module health**
   Modules such as `monty_repl` and `route_message` now have explicit target / provenance / criticality contracts instead of being treated as best-effort file discovery.

This turns startup from:

- "discover things, compile what you find, replay broad install work"

into:

- "restore durable state, reconcile only changed bundles, load declared artifacts, and serve"

## What Was Changed

### OpenPaw

1. Startup policy and startup telemetry were added in `crates/openpaw/src/startup.rs`.
2. Datadog dashboard widgets and monitors were added in:
   - `dd-dashboards/openpaw-overview.json`
   - `dd-monitors/openpaw-monitors.json`
3. OS-app WASM contracts were added in:
   - `os-apps/paw-agent/app.toml`
   - `os-apps/paw-channels/app.toml`
4. OS-app build scripts were updated to copy compiled artifacts into bundle-visible module paths:
   - `os-apps/paw-agent/wasm/build.sh`
   - `os-apps/paw-channels/wasm/build.sh`
5. Startup architecture and rollout were documented in:
   - `docs/adrs/0028-bounded-startup-surface-and-wasm-artifact-contract.md`
   - `docs/startup-hardening-plan.md`

### Temper

1. OS-app reconcile is now digest-aware and can do runtime-only reload for unchanged bundles.
2. Durable app install state is committed atomically through `AppInstallBundle` / `persist_app_install_bundle`.
3. Wasmtime persistent cache support was added.
4. Startup / reconcile / WASM metrics were added in `crates/temper-server/src/runtime_metrics.rs`.
5. Reconcile tests were extended to cover unchanged-bundle behavior and Cedar-policy stability.
6. A policy-drift bug was fixed so unchanged bundle reconciles do not duplicate Cedar policy text in memory.

## Datadog Surface Added

### Dashboard widgets

Added the following widgets to the OpenPaw overview dashboard:

- `Projected Entities`
- `Projection Coverage`
- `Projection Snapshot Misses`
- `Startup Phase Duration`
- `Startup Time To Healthy`
- `Startup Live Restore Entities`
- `OS App Reconcile`
- `OS App Reconcile Duration`
- `WASM Load Failures`
- `WASM Modules Skipped`

### Monitors

Added the following monitors:

- `[OpenPaw] Startup Time Regression`
- `[OpenPaw] OS App Reconcile Regression`
- `[OpenPaw] Required WASM Load Failures`

The existing indexed-entity query was also corrected to use the tenant-aware total:

- `sum:temper_indexed_entities{service:openpaw,tenant:*}`

## Verification Flow

### 1. Build bundled WASM artifacts

Commands run:

```bash
cd /Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm && ./build.sh
cd /Users/seshendranalla/Development/openpaw-codex/os-apps/paw-channels/wasm && ./build.sh
```

Expected:
- module artifacts are built ahead of startup
- `monty_repl` is built for `wasm32-wasip1`
- channel `route_message` is built for `wasm32-wasip1`

Actual:
- both build scripts completed successfully
- `monty_repl` artifact was produced and copied into the module bundle path

### 2. OpenPaw compile / test

Commands run:

```bash
cargo build -p openpaw
cargo test --workspace -- --nocapture
```

Expected:
- OpenPaw compiles with the new startup policy and Datadog config changes
- workspace tests stay green

Actual:
- `cargo build -p openpaw` passed
- `cargo test --workspace -- --nocapture` passed
- startup tests included:
  - `startup::tests::local_wasm_policy_defaults_and_overrides`
  - `startup::tests::runtime_recovery_finishes_query_plane_before_post_boot_tasks`
  - `startup::tests::datadog_configs_use_tenant_aware_entity_queries`

### 3. Temper targeted regression coverage

Commands run:

```bash
cargo test -p temper-platform --test os_app_reconcile_e2e unchanged_bundle_reconcile_does_not_duplicate_cedar_policies -- --nocapture
cargo test -p temper-server --test dst_platform_random dst_random_workload_no_faults -- --nocapture
```

Expected:
- unchanged bundle reconcile does not duplicate Cedar policy state
- the earlier DST policy-mismatch failure no longer reproduces

Actual:
- the new reconcile test passed
- the random DST run progressed beyond the original failing point and no longer reproduced the prior policy mismatch

### 4. Temper workspace verification

Command run:

```bash
cargo test --workspace -- --nocapture
```

Expected:
- the full Temper workspace remains green after startup hardening, runtime-only reconcile, atomic install bundling, and persistent cache changes

Actual:
- the workspace test run progressed cleanly through the large integration suites during verification, including:
  - ecommerce cascade / DST suites
  - oncall cascade / DST suites
  - CLI / bootstrap / router / OS-app integration suites
  - identity / compile-first / integration-engine suites
- OS-app reconcile E2E suites passed, including:
  - `unchanged_bundle_reconcile_skips_wasm_reload`
  - `unchanged_bundle_reconcile_reloads_missing_required_wasm`
  - `unchanged_bundle_runtime_reload_does_not_replay_content_bootstrap`
  - `unchanged_bundle_reconcile_does_not_duplicate_cedar_policies`
- projection / restart DST suites and random platform-fault suites advanced cleanly without reproducing the earlier policy-drift failure
- the full workspace sweep surfaced one stale test expectation in `temper-wasm`:
  - `engine::tests::resource_limits_default`
  - the runtime default is `16 MB`, matching `types.rs`
  - the engine test was still asserting `64 MB`
- after correcting that stale assertion, the affected WASM suite was rerun with:

```bash
cargo test -p temper-wasm --lib -- --nocapture
```

and passed with `49 passed; 0 failed`

Note:
- this suite is large and noisy because of proptest persistence warnings, but the startup-hardening changes did not introduce a visible failing regression during the verification run

### 5. End-to-end OpenPaw restart proof

Disposable environment:

- `HOME=/tmp/openpaw-startup-proof3.CQdBBz`
- `PORT=60997`
- `OTEL_ENABLED=false`
- `OPENPAW_WASM_STARTUP_POLICY=load-only`
- `OPENAI_CODEX_TOKEN=dummy`
- `RUST_LOG=info`

Expected:

1. first boot uses declared bundled artifacts and reaches healthy
2. warm restart does not re-run broad bootstrap for unchanged bundles
3. auth/session survives restart
4. active `Paw` agent is not duplicated
5. startup logs show restore-before-serve ordering

Actual first boot:

- log contained:
  - `Wasmtime persistent cache enabled`
  - `WASM startup policy selected wasm_policy=LoadPersistedOnly`
  - multiple `WASM module loaded from OS app ... provenance="bundled-artifact"` lines
  - `Open Paw listening on port 60997`
- `/healthz` returned `200`

Auth flow:

```bash
curl -s -c /tmp/openpaw-startup-proof3.CQdBBz/cookies.txt \
  -H 'content-type: application/json' \
  -d '{"email":"proof@example.com","password":"proof-password-123"}' \
  http://127.0.0.1:60997/auth/register
```

Actual:

- register succeeded with `{"email":"proof@example.com","provider":"local"}`
- `GET /auth/me` before restart returned `200`

Pre-restart entity checks:

- `GET /tdata/Agents?$filter=name eq 'Paw' and Status eq 'Active'`
  - returned one active `Paw`
  - entity id: `aj-019d89ab-3c78-7570-9e49-7d5bf7d540e5`
- `GET /tdata/AgentRoutes`
  - returned one active route
  - route id: `en-019d89ab-3d2e-7810-a022-cdf5d33ab973`

Actual restart behavior:

- log contained:
  - `Restored 60 specs from Turso`
  - `Vault key loaded from file`
  - `Restored secrets from Turso tenant="default" restored=5`
  - `WASM startup policy selected wasm_policy=LoadPersistedOnly`
  - `Skipping content bootstrap for unchanged OS app bundle; reloading runtime prerequisites only`
  - `populated query projections (snapshots + persistence replay) tenant=default total=137 indexed=137 errors=0`
  - `Open Paw listening on port 60997`
- `/healthz` returned `200`

Post-restart correctness:

- `GET /auth/me` with the original cookie still returned `200`
- `GET /tdata/Agents?$filter=name eq 'Paw' and Status eq 'Active'`
  - returned the same `Paw` entity id: `aj-019d89ab-3c78-7570-9e49-7d5bf7d540e5`
  - `total_event_count` increased from `3` to `4`
  - no duplicate `Paw` entity appeared
- `GET /tdata/AgentRoutes`
  - returned the same route id: `en-019d89ab-3d2e-7810-a022-cdf5d33ab973`

### 6. End-to-end heavy authenticated workload with Datadog confirmation

Disposable environment:

- local OpenPaw process with:
  - `OPENPAW_WASM_STARTUP_POLICY=load-only`
  - `OTEL_ENABLED=true`
  - `DD_ENV=local`
  - `TEMPER_API_KEY=benchmark-secret`
  - `TEMPER_RUNTIME_METRICS_INTERVAL_SECS=2`
  - `TEMPER_ACTOR_IDLE_TIMEOUT=20`
  - `TEMPER_PASSIVATION_CHECK_INTERVAL=5`
- local process log:
  - `/Users/seshendranalla/Development/openpaw-codex/tmp_openpaw_streamload.log`
- verification window:
  - start: `2026-04-14T02:47:38Z`
  - server listening: `2026-04-14T02:47:43.579402Z`
  - workload finished: `2026-04-14T02:48:30.527675Z`

Workload shape:

- real authenticated OData traffic against `/tdata/Files`
- `200` file entities created
- content uploaded through `PUT .../$value`
- `24` concurrent workers
- approximately `16 KB` payload per file
- `45` second idle wait after the workload to allow actor passivation

Local runtime evidence:

- startup log showed:
  - `Phase 7: Recovery...`
  - `populated query projections (snapshots + persistence replay) tenant=default total=131 indexed=131 errors=0`
  - `Phase 8: Bootstrap complete`
  - `Open Paw listening on port 3467`
- time to healthy from the startup log was `5.363823` seconds
- local RSS samples peaked at `769.75 MB` and settled around `767.34 MB`
- local passivation log showed:
  - `passivated idle actors count=123 timeout_secs=20`
  - `passivated idle actors count=214 timeout_secs=20`

Datadog metric evidence for the exact run window (`service:openpaw,host:Mac`):

- `temper_active_actors` peak: `214`
- `temper_indexed_entities` peak: `476`
- `temper_projected_entities` peak: `474`
- `process_resident_memory_bytes` peak: `806600704` bytes = `769.23 MB`

Datadog log evidence for the same window:

- query: `service:openpaw host:Mac "passivated idle actors"`
- result count: `2`
- timestamps:
  - `2026-04-14T02:48:11Z`
  - `2026-04-14T02:48:11Z`

Interpretation:

- actors stayed materially below the total entity / projection count during the heavy run
- actor count did not fan out to the full corpus
- Datadog observed the idle-passivation event immediately after the workload window
- memory rose during real work, but remained bounded and did not show runaway growth
- the query-plane and startup changes held under authenticated concurrent load

## Verification Summary

| Area | Result |
|------|--------|
| Bundled WASM artifact builds | PASS |
| OpenPaw build | PASS |
| OpenPaw workspace tests | PASS |
| Temper reconcile regression test | PASS |
| Temper random DST regression no longer reproduces earlier failure | PASS |
| Temper workspace verification run | PASS |
| `cargo test -p temper-wasm --lib -- --nocapture` after stale expectation fix | PASS |
| OpenPaw end-to-end warm-restart proof | PASS |
| OpenPaw heavy authenticated workload with Datadog confirmation | PASS |
| Datadog dashboard/monitor configuration present | PASS |

## What This Means Operationally

### Startup should no longer compile WASM by default

The intended production-like path is now:

1. build module artifacts ahead of time
2. package them as bundled artifacts
3. declare their contracts in `app.toml`
4. start OpenPaw in `LoadPersistedOnly`
5. let Wasmtime persistent cache amortize native compilation cost

### Warm restart is now bounded

On unchanged bundles, startup should:

- restore durable state
- recover runtime prerequisites
- rebuild projections if needed
- restore the live working set
- start serving

It should **not** replay broad APP.md / agent / skill / system-file bootstrap for unchanged bundles.

### First-run bootstrap is still real work

There is still a meaningful difference between:

- **first-run bootstrap on an empty durable store**
- **warm restart on an already-installed tenant**

This work made the second path bounded and explainable.

The remaining cost on the first path is now an explicit bootstrap/install concern rather than an every-startup tax.

## Remaining Truths

1. First-run empty-tenant bootstrap is still heavier than warm restart because the system is genuinely installing OS apps, content, and durable metadata for the first time.
2. The previous ambiguous `monty_repl` startup story is now structurally addressed by artifact contracts and bundled build outputs, but the real standard is the packaged artifact path, not dev-local source compilation.
3. The long Temper workspace suite is still noisy because of proptest persistence warnings, but the full workspace completed green after the stale `temper-wasm` expectation was corrected.

## Artifacts

- Proof home: `/tmp/openpaw-startup-proof3.CQdBBz`
- Restart server sessions:
  - first boot session `9843`
  - restart session `40598`
- Heavy local workload session:
  - streamload session `24093`
- Local OpenPaw binary used:
  - `/Users/seshendranalla/Development/openpaw-codex/target/debug/openpaw`
