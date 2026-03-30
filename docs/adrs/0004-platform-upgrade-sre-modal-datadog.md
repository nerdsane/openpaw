# ADR-0004: Platform Upgrade for SRE, Modal, Datadog, and CI/CD Closure

## Status

Accepted

## Context

OpenPaw's self-healing path has outgrown the original demo assumptions. The current branch already proves governed agent loops, webhook ingestion, and PM linkage, but the platform narrative and implementation still mix old terminology and earlier infrastructure choices:

- the monitoring persona is still described inconsistently across the repo
- the historical sandbox story still points at E2B instead of the desired platform-governed Modal path
- monitoring references have drifted across multiple observability backends
- project bootstrap does not yet model automated monitor generation as a first-class entity
- `AlertCycle` stops too early for the full PR -> merge -> deploy -> verify story

The upgrade needs one coherent direction so parallel implementation work converges on the same runtime model.

## Decision

### 1. Use `SRE` as the canonical monitoring persona

The alert-triage soul and all workflow references should use `SRE`. This better reflects the role: operational triage, remediation coordination, monitor tuning, and post-fix verification.

### 2. Standardize on Datadog for monitoring and alerting

OpenPaw's monitor entity, webhook ingestion path, and query tooling should target Datadog:

- `Monitor.dd_query` stores the Datadog query or identifying expression
- monitor linkage continues to use `dd_monitor_id`
- the agent tool surface uses `datadog_query`
- webhook normalization treats Datadog as the primary external alert source

Historical references to older observability choices should be removed from the product path so the repo tells one story.

### 3. Add `MonitorScan` as a first-class bootstrap entity

Automated monitor generation is modeled explicitly with a `MonitorScan` entity rather than being hidden in prompt text or ad hoc scripts. This gives the platform a governed record of:

- which project harness was scanned
- whether the scan was a bootstrap or PR-delta run
- which commit was targeted
- how many monitors were created or updated
- whether the scan failed and why

### 4. Extend `AlertCycle` for CI/CD closure

`AlertCycle` does not end at "PR opened." The state machine is extended so the governed lifecycle can capture:

- `Fixed`
- `Merging`
- `Deploying`
- `Verifying`
- `Resolved`

This keeps the self-healing loop aligned with the real operator expectation: not just "a fix exists," but "the fix shipped and the monitor recovered."

### 5. Target Modal for governed remote sandboxes

The long-term remote sandbox provider is Modal. The architectural direction is:

- sandbox provisioning remains inside the governed WASM path
- the agent runtime should talk to Modal from WASM via Connect/gRPC-compatible calls rather than through an out-of-band Python bridge
- provider credentials live in the OpenPaw config/vault surface, not in ad hoc script state

This preserves Cedar-governed control over who can provision a sandbox and under which project context.

## Consequences

### Positive

- The repo, specs, souls, and proof drivers use one operational vocabulary.
- Datadog becomes the primary monitoring contract for both ingestion and investigation.
- Monitor bootstrap is auditable through a governed entity instead of hidden side effects.
- The self-heal story can naturally extend through merge, deploy, and verification.

### Negative

- The rename and provider shift touch prompts, specs, webhook handling, docs, and proof artifacts.
- Historical proof language has to be updated or clearly marked as legacy.
- Modal integration is deeper than a secret swap because the current remote sandbox path was designed around a different provider.

### Risks

- Partial migration leaves the repo in a split-brain state where the docs and runtime disagree.
- CI/CD closure work can create state-machine drift if webhook handlers are not updated together with the spec.
- Modal integration remains risky until the governed WASM provisioning path is proven against real credentials.
