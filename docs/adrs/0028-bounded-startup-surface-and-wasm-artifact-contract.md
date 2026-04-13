# ADR-0028: Bounded Startup Surface and WASM Artifact Contract

**Status:** Accepted
**Date:** 2026-04-13
**Related:** ADR-0001 (Open Paw architecture), ADR-0005 (Temper-native orchestration), ADR-0026 (durable query plane and bounded actor residency)

## Context

ADR-0026 separated the truth plane, query plane, and execution plane. That fixes one major source of startup memory pressure: query-plane repair no longer needs to hydrate actors just to answer collection filters.

That is necessary, but not sufficient.

OpenPaw startup is still heavier than it should be for two separate reasons:

1. **Startup still performs too much imperative install/reconcile work.**
   Today, startup runs a blanket OS-app install pass, then bootstraps app entities, APP.md files, agents, skills, system files, and local WASM registration before the system is considered booted. Much of that work is idempotent but still expensive. On warm restarts, the platform should not pay the same cost as first-run bootstrap.

2. **WASM module loading has no explicit artifact contract.**
   The current loader discovers module directories, looks for whatever `.wasm` file happens to exist, and tries to compile it. This is why `monty_repl` can emit a warning like `__wbindgen_describe has not been defined`: the runtime is attempting to load an artifact whose ABI/build provenance is not explicit enough. Optional module failures should not silently ride along in the hot startup path.

The result is a startup path that is:

- heavier than necessary on warm restarts
- harder to reason about operationally
- too dependent on local build artifacts and best-effort module loading

This is especially risky on smaller instances, where startup CPU and memory spikes translate directly into long cold starts, instability, and health-check flapping.

## Decision

OpenPaw and Temper will make startup bounded, phase-aware, and artifact-driven.

### 1. Warm restart must be a bounded restore path

Warm restart is not allowed to behave like first-run bootstrap.

On restart, the platform should:

- load durable registry/query-plane/runtime state
- restore only the live working set
- serve traffic as soon as the runtime is coherent

It should **not** re-run broad OS-app content seeding and module build/discovery work as a prerequisite for health unless a versioned reconcile step explicitly requires it.

### 2. OS-app install becomes versioned reconcile, not unconditional startup work

OS-app lifecycle work will be split into two categories:

- **runtime restore**
  Required to make the currently installed system coherent and serve requests
- **bundle reconcile**
  Required only when an app bundle digest, schema version, or artifact manifest changes

Bundle reconcile should be driven by durable version metadata and, where appropriate, by Temper-native entities/WASM integrations rather than by large imperative startup loops.

### 3. Content seeding must be incremental and digest-aware

Bootstrapping APP.md, skills, system files, agent definitions, and similar content should not rewrite the world every time the process starts.

Instead, each seeded unit should have:

- a durable digest/version
- explicit ownership
- idempotent reconcile semantics

If the installed digest matches the bundled digest, startup skips the work.

### 4. WASM modules require an explicit artifact contract

Each OS-app module will declare artifact metadata that includes at least:

- module name
- target (`wasm32-unknown-unknown` vs `wasm32-wasip1`)
- ABI expectations/import class
- digest
- required vs optional classification
- build provenance (prebuilt artifact vs dev-local)

Startup/load logic will validate the artifact against this contract before attempting to compile or register it.

### 5. Production startup consumes prebuilt artifacts, not local crate builds

In production-like startup paths, the platform should load prebuilt `.wasm` artifacts that are packaged with the app bundle or already persisted durably.

Local source builds during startup are a development convenience only, and must be explicitly enabled. They are not part of the normal runtime boot contract.

### 6. Module failure handling is classified, not ad hoc

WASM module failures will be handled by classification:

- **platform-required**
  Failure blocks startup because the platform cannot operate safely
- **app-required**
  Failure marks the app degraded and blocks only the affected capability surface
- **optional**
  Failure is skipped, metered, and surfaced in app/module health without poisoning global startup

`monty_repl` is not platform-required. If it is unavailable, the agent execution surface is degraded, but the platform control plane should still boot cleanly and report that degradation explicitly.

### 7. Startup must be observable by phase and by reconcile reason

The platform must emit metrics that answer:

- how long each startup phase took
- which apps were reconciled, skipped, or repaired
- which modules were loaded, skipped, or rejected
- how long it took from process start to healthy

Without this, we cannot safely tighten cold-start budgets or prove that later changes improved anything.

## Consequences

### Positive

- Warm restarts become materially faster and less bursty.
- Small instances spend less memory and CPU on redundant startup work.
- OS-app/content reconcile work becomes explicit and explainable.
- WASM loading becomes deterministic and debuggable.
- Optional module failures stop looking like mysterious startup noise.

### Negative

- We need durable metadata for app bundle digests and seeded content state.
- The loader becomes stricter, which may expose latent packaging issues immediately.
- We introduce a more formal distinction between restore, reconcile, and degraded capability modes.

### Risks

- If reconcile/version metadata is incomplete, startup may incorrectly skip necessary work.
- Some current boot behavior may be masking missing reconcile steps that will need to be modeled explicitly.
- Stricter artifact validation could temporarily surface more module load failures until packaging is cleaned up.

## Non-Goals

- This ADR does not remove the query plane introduced in ADR-0026.
- This ADR does not attempt to redesign every OS-app bootstrap behavior in one change.
- This ADR does not require every optional module failure to become fatal.

## Implementation Direction

The follow-on implementation will:

1. instrument startup by phase, app, and module
2. introduce durable app-bundle/content digests
3. split warm restart from bundle reconcile
4. move module loading to an artifact-manifest contract
5. classify module failures by capability criticality
6. progressively move bulky startup reconcile work out of the hot path
