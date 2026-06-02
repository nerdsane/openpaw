# ADR 0065: Canonical Genesis Worker Pool

## Status

Accepted

## Context

TemperPaw currently contains both platform worker code and several Temper-native app bundles. Directed Evolution review made this confusing: the user wants Genesis to be the canonical home for app bundles, while TemperPaw remains the worker/agent platform that can install pinned app refs and run worker processes.

The Codex worker also had a V1-only `MAX_CONCURRENT_RUNS == 1` restriction. That blocks real simulated-user and evaluator concurrency, and it makes local Codex look like a singleton instead of a pool of Temper-claimed worker slots.

## Decision

- Production app bundles move to Genesis. TemperPaw keeps platform code, worker crates, test fixtures, bootstrap/install references, and docs.
- The shared execution app is published as `temperpaw/paw-orchestration`. Its
  bundle-local app name remains `paw-orchestration`, and it owns
  `WorkerProvider`, `WorkerAgent`, `WorkItem`, and `WorkerRun`.
- Paw Patrol no longer owns generic worker concepts long-term. It links to shared `WorkItemId` and `WorkerRunId` for patrol-specific work.
- `paw-agent` keeps native TemperPaw session entities. `WorkerRun.SessionId` is optional and only set for TemperPaw-native agent execution.
- Local Codex concurrency is modeled as multiple worker processes. Each process registers as one `WorkerAgent` slot and claims one `WorkItem` at a time.
- Each local Codex process uses an explicit worker profile. `PAW_CODEX_MODEL` pins the model used by `codex exec`, and `PAW_CODEX_EXEC_TIMEOUT_SECS` bounds each run. This keeps process-pool behavior reproducible and makes WorkerRun provenance say which model profile was actually used.
- Selector and promoter work uses exclusive keys so it is serialized per episode/organism. Observer, simulated-user, and evaluator work can run concurrently.
- Directed Evolution observer and telemetry evaluator roles require structured Datadog evidence. Missing Datadog query/window/result/interpretation evidence is a failure, not an informational warning.

## Consequences

- Users can scale local Codex by starting more worker slots instead of relying on hidden concurrency inside one process.
- Execution provenance uses worker/provider/run language everywhere, avoiding `BrainRun` as an entity name.
- TemperPaw app directories become migration/bootstrap surfaces until their canonical Genesis refs are published and pinned.
- Existing local proof data may still contain old names; worker and UI code should remain tolerant while presenting new language.
