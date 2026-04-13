# Startup Hardening Plan

## Goal

Make OpenPaw startup bounded on warm restarts, keep first-run bootstrap explainable, and eliminate ambiguous WASM loading behavior such as the current `monty_repl` warning.

This plan implements ADR-0028 alongside ADR-0026.

## Success Criteria

The work is complete when all of the following are true:

- Warm restart reaches healthy without re-running broad OS-app/content bootstrap work unnecessarily.
- Startup restore work is limited to coherent runtime recovery plus live-set restore.
- OS-app/content reconcile is digest-aware and runs only when something actually changed.
- WASM modules load from explicit artifact manifests rather than from best-effort file discovery.
- Optional or app-scoped module failures are surfaced as degraded capability, not as global startup ambiguity.
- Datadog shows phase durations, reconcile counts, module load failures, and time-to-healthy.

## Current Reality

Today the heavy startup path is dominated by two behaviors:

- `Phase 6` installs every Paw OS app and bootstraps APP.md, agents, skills, and system files every run in [startup.rs](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/startup.rs:621).
- OS-app module loading compiles/registers whatever artifacts are found and logs warnings on failure in [mod.rs](/Users/seshendranalla/Development/temper/crates/temper-platform/src/os_apps/mod.rs:1468).

The `monty_repl` warning is a concrete symptom of the second problem:

- startup treats module artifacts as discoverable files rather than contract-validated build outputs
- `monty_repl` is special-cased to use `wasm32-wasip1` in [startup.rs](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/startup.rs:2125)
- the app build helper also treats it as a special-case WASI build in [build.sh](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/build.sh:22)

## Design Principles

### 1. Restore is not reconcile

Warm startup should restore runtime coherence, not perform all bundle installation work from scratch.

### 2. Reconcile must be versioned

If app bundle content or a module artifact did not change, startup should be able to prove that and skip the work.

### 3. Capability degradation must be explicit

If `monty_repl` is unavailable, the system should say "agent execute capability is degraded" rather than silently attempting a load and leaving an operational mystery behind.

### 4. Production startup must not depend on local source builds

Source-based startup builds are development-only behavior.

### 5. Metrics are part of the design

Every optimization phase must add or use measurements in Datadog, not just local logs.

## Workstreams

## Workstream A: Startup Telemetry

### Deliverables

- Add metrics:
  `temper_startup_phase_duration_ms`
- Add metrics:
  `temper_startup_time_to_healthy_ms`
- Add metrics:
  `temper_os_app_reconcile_total`
- Add metrics:
  `temper_os_app_reconcile_duration_ms`
- Add metrics:
  `temper_wasm_module_load_failures_total`
- Add metrics:
  `temper_wasm_module_skipped_total`
- Add metrics:
  `temper_startup_live_restore_entities_total`

### Datadog updates

- Add a startup hardening section to the OpenPaw dashboard:
  phase durations
  time to healthy
  reconcile count by app/result
  WASM load failures by module/reason
- Add monitors for:
  startup time regression
  module load failure spikes for required/app-required modules

## Workstream B: Digest-Aware App Reconcile

### Deliverables

- Add durable app bundle digest/version metadata.
- Add durable seeded-content digests for:
  APP.md
  agent definitions
  skills
  system files
- Skip reconcile when installed digest matches bundled digest.

### Tests

- Restart with unchanged app bundle does not rewrite seeded content.
- Restart with changed app bundle reconciles only the changed app.
- APP.md/skills/system files remain idempotent across repeated restart cycles.

## Workstream C: Split Startup Modes

### Deliverables

- Define explicit startup modes:
  cold bootstrap
  warm restart
  repair/reconcile
- Warm restart does:
  storage/spec recovery
  policy/WASM registry recovery
  query-plane recovery
  live-set restore
- Cold bootstrap may additionally do initial install/seeding.
- Repair/reconcile may run after the server is already healthy.

### Tests

- Warm restart reaches healthy without broad install churn.
- Cold bootstrap still provisions a fresh tenant correctly.
- Repair mode reconciles drift without requiring process restart.

## Workstream D: WASM Artifact Contract

### Deliverables

- Add an artifact manifest per module with:
  module name
  target
  ABI/import class
  digest
  required/app-required/optional classification
  provenance
- Loader validates the artifact before compile/register.
- Production startup consumes prebuilt artifacts only.
- Development-only local compile path is gated explicitly.

### `monty_repl` resolution

The `monty_repl` warning is addressed by making its contract explicit:

- declare it as `wasm32-wasip1`
- validate import expectations before compile
- package its prebuilt artifact in the bundle
- classify failure as app-required, not platform-required

That means:

- the platform still boots if `monty_repl` is bad
- `paw-agent` execution mode is marked degraded
- Datadog and app/module health show the failure clearly

### Tests

- Optional invalid artifact is skipped and metered.
- App-required invalid artifact degrades only the affected app capability.
- Valid `monty_repl` artifact loads without warning.
- Production startup does not invoke local crate compilation.

## Workstream E: Move Heavy Reconcile off the Hot Path

### Deliverables

- Convert bulky post-install/bootstrap work into explicit reconcile jobs.
- Favor Temper-native entities/WASM for ongoing reconcile where state changes need tracking.
- Keep the hot startup path focused on:
  runtime recovery
  query-plane recovery
  live-set restore
  listener startup

### Tests

- Warm restart does not create duplicate app/bootstrap entities.
- Deferred reconcile can run after health without changing observable runtime correctness.
- Startup CPU and RSS drop relative to the current baseline on the same seeded DB.

## Phases

## Phase 0: Instrument Before Changing Behavior

### Purpose

Measure exactly where startup time and churn are going.

### Scope

- Add startup phase timers.
- Add OS-app reconcile/load result metrics.
- Add WASM load failure metrics.
- Add dashboard/monitor slices for the new metrics.

### Exit Criteria

- Datadog shows phase durations and module load failures by name.
- We can separate warm-restart cost from first-run cost.

## Phase 1: Skip No-Op Reconcile

### Purpose

Stop repeating bundle/content work when nothing changed.

### Scope

- Add app/content digests.
- Skip APP.md, skills, agent definitions, and system-file bootstrap when digests match.

### Exit Criteria

- Warm restart of an unchanged tenant shows near-zero reconcile writes for unchanged apps.

## Phase 2: Formalize Module Artifacts

### Purpose

Eliminate ambiguous WASM loading and fix the `monty_repl` class of issue structurally.

### Scope

- Introduce artifact manifests.
- Require prebuilt artifacts in production startup.
- Validate target/import class before compile/register.

### Exit Criteria

- `monty_repl` no longer emits the current warning in a valid packaged build.
- Invalid artifacts fail with clear classification and metrics.

## Phase 3: Split Warm Restart from Reconcile

### Purpose

Bound startup by serving from restored state first, then reconciling changed apps separately.

### Scope

- Separate warm-restart path from repair/reconcile work.
- Keep listener start gated on coherence, not on every no-op app reinstall.

### Exit Criteria

- Warm restart reaches healthy before any non-essential reconcile work begins.

## Phase 4: Move Bulky Reconcile into Explicit Control Plane Work

### Purpose

Remove the remaining imperative startup loops that still behave like orchestration.

### Scope

- Model reconcile where useful as explicit Temper-native jobs/entities.
- Keep startup Rust code thin and deterministic.

### Exit Criteria

- Startup behavior is understandable as:
  restore
  live-set recovery
  optional deferred reconcile

## Recommended Next Slice

The highest-leverage next implementation slice is:

1. add startup phase/module/reconcile metrics and wire them to Datadog
2. add durable app/content digests and skip no-op reconcile
3. introduce a WASM artifact manifest for `monty_repl` first
4. classify `monty_repl` as app-required and surface degraded capability explicitly

That gets us immediate operational clarity, removes redundant work on warm restarts, and fixes the current `monty_repl` ambiguity without taking a giant migration step all at once.
