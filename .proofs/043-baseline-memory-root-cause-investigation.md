# Proof Report: 043 — Baseline Memory Root Cause Investigation

## Date
2026-04-14

## Workspace
- **openpaw**: `/Users/seshendranalla/Development/openpaw-codex`
- **temper**: `/Users/seshendranalla/Development/temper`

## Objective
Determine what still drives the high baseline memory footprint after fixing the startup query-plane hydration problem.

The specific question was:

- if actors are now bounded and passivating, why does a mostly idle OpenPaw process still sit hundreds of megabytes above zero?

## Conclusion
The remaining baseline memory problem is real, but it is **not** caused by runaway actor hydration.

The evidence points to three primary contributors:

1. a large default startup surface in OpenPaw
2. eager OS-app install / bootstrap work during startup
3. eager load + compile + cache of every persisted WASM module during startup

The data does **not** support the theory that the process is holding the full entity corpus as hydrated actors.

## Evidence Summary

### 1. Release builds improve boot time more than memory

A clean matrix was run with isolated `HOME` directories and a single OpenPaw process per case.

Measured cases:

- `debug + OTEL on`
- `debug + OTEL off`
- `release + OTEL off`

Observed results:

- `debug + OTEL on`
  - boot to healthy: `40.81s`
  - RSS: `660.06 MB`
  - macOS physical footprint: `284.9 MB`
- `debug + OTEL off`
  - boot to healthy: `41.58s`
  - RSS: `664.97 MB`
  - macOS physical footprint: `270.2 MB`
- `release + OTEL off`
  - boot to healthy: `10.98s`
  - RSS: `634.08 MB`
  - macOS physical footprint: `261.1 MB`

Interpretation:

- switching from debug to release materially reduces startup time
- it does **not** materially reduce the baseline RSS floor
- OTEL enablement has negligible effect on the RSS floor

So the residual baseline memory is not primarily a debug-build artifact and not primarily an OTEL exporter problem.

### 2. The boot corpus is small

The isolated release boot database contained only a small catalog:

- total catalog rows: `136`
- dominant entity types:
  - `File = 45`
  - `Directory = 38`
  - `Taxonomy = 15`
  - `App = 15`
  - `Soul = 10`

Interpretation:

- the baseline memory floor is not explained by a huge entity corpus

### 3. Default OpenPaw startup installs a large app surface

OpenPaw currently auto-installs the `PAW_OS_APPS` set in [startup.rs](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/startup.rs:27):

- `paw-agent`
- `paw-channels`
- `paw-fs`
- `paw-pm`
- `paw-compute`
- `paw-harness`
- `paw-heal`
- `paw-ingest`
- `paw-research`
- `paw-foresight`
- `koto-learn`
- `koto-tutor`
- `koto-wiki`
- `dsf-harness`
- `katagami-commons`
- `katagami-curation`

During install, Temper boots much more than specs. The OS-app bundle model includes:

- specs
- Cedar policies
- WASM modules
- agent definitions
- skills
- ADRs
- system files
- seed data

That surface is defined in [os_apps/mod.rs](/Users/seshendranalla/Development/temper/crates/temper-platform/src/os_apps/mod.rs:100).

Install then performs all of these steps:

- compile/register WASM modules
- bootstrap `App` entities
- bootstrap agents
- bootstrap skills
- bootstrap system files
- bootstrap ADRs
- create seed instances

Those steps run in [os_apps/mod.rs](/Users/seshendranalla/Development/temper/crates/temper-platform/src/os_apps/mod.rs:1450).

Interpretation:

- even after the warm-start contract improvements, the default app surface is still broad
- that broad install/bootstrap surface contributes directly to the baseline process footprint

### 4. OS-app install scans and reads compiled WASM artifacts eagerly

OS-app discovery scans each app's `wasm/` subdirectories and reads the release artifact bytes into memory during install in [os_apps/mod.rs](/Users/seshendranalla/Development/temper/crates/temper-platform/src/os_apps/mod.rs:581).

This is the right contract for bundled artifacts, but it still means:

- every default-installed app pays discovery cost
- every discovered module is a startup concern unless the app itself is removed from the default set

### 5. Persisted WASM modules are eagerly compiled and cached on boot

Temper startup loads all persisted modules and compiles them through `WasmEngine::compile_and_cache()` in [state/persistence/mod.rs](/Users/seshendranalla/Development/temper/crates/temper-server/src/state/persistence/mod.rs:199).

`compile_and_cache()` then:

- builds a `wasmtime::Module`
- pre-instantiates/link-prepares it
- stores it in the in-memory engine cache

That behavior is in [engine/mod.rs](/Users/seshendranalla/Development/temper/crates/temper-wasm/src/engine/mod.rs:160).

Measured persisted module state in the isolated release boot database:

- module count: `47`
- raw persisted bytes: `17,583,304` (`~17.6 MB`)

Largest persisted modules:

- `monty_repl`: `6,308,967` bytes
- `llm_caller`: `615,736` bytes
- `route_message`: `408,474` bytes

Interpretation:

- raw module bytes alone do not explain a `~634 MB` RSS floor
- but eager compilation + pre-instantiation of all persisted modules is a real contributor
- heavyweight tool/developer modules such as `monty_repl` are especially suspect for idle baseline cost

### 6. The workload delta is much smaller than the baseline floor

From the isolated heavy workload proof in [042-isolated-memory-entity-actor-investigation.md](/Users/seshendranalla/Development/openpaw-codex/.proofs/042-isolated-memory-entity-actor-investigation.md:1):

- baseline RSS before load: `~656 MB`
- peak RSS under load: `~702.6 MB`
- RSS after idle: `~692.5 MB`

So the heavy workload only added roughly `~46 MB` at peak over the already-large baseline.

Interpretation:

- the dominant issue is baseline runtime footprint
- the incremental memory cost of the workload is comparatively modest

## What This Means

The root cause has moved.

We already fixed the earlier architectural problem:

- startup query-plane rebuild no longer needs to hydrate actors just to recover projections

The remaining memory issue is now mostly about:

- how much platform/app surface we choose to install by default
- how much startup eagerly materializes into memory
- how many persisted modules we compile and cache up front

## Principal-Engineer Recommendation

The next reduction pass should focus on startup surface and module loading policy, not actor leak hunting.

Priority order:

1. reduce the default `PAW_OS_APPS` set to true platform-core apps only
2. move reference/demo/teaching apps out of the default boot set
3. separate "required at startup" WASM modules from "tooling / developer / optional" modules
4. lazy-load or demand-load heavyweight optional modules such as `monty_repl`
5. keep the dashboard focused on:
   - `temper_indexed_entities`
   - `temper_projected_entities`
   - `temper_active_actors`
   - `process_resident_memory_bytes`

That is the clean path to shrinking the idle footprint without regressing the architectural work we already did.
