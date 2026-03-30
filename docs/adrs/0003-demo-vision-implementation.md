# ADR-0003: Demo Vision Implementation

## Status

Accepted

## Context

Open Paw already has the governed agent primitives needed for a self-healing demo, but the operator experience is still fragmented. The platform can run SRE -> Developer remediation loops, yet the loop is manually kicked off through OData and ad hoc proof scripts instead of starting from a real external signal or an operator message.

The target demo is a tighter story:

- a human tells Paw to manage a project
- Open Paw creates the project-management structure
- external alerting enters through a webhook
- SRE is spawned automatically
- Developer remediation and PR creation happen inside the governed loop
- the human gets a proactive update back through the same channel

The implementation also needs to be provable phase by phase, because two independent implementations are being compared and the gaps need to be auditable.

## Decision

### 1. Implement the demo in phases with proof gates

The work is split into ordered phases, and each phase is only considered done when it has a corresponding proof driver and proof report. This keeps the implementation comparable across parallel branches and prevents “wired but unproven” claims from slipping into the demo story.

### 2. Keep webhook ingestion inside the `openpaw` daemon

Webhook handling lives in `crates/openpaw/src/webhooks.rs` and is mounted directly onto the daemon router. We are not creating a separate ingest service. External payloads are translated back into the platform by making internal OData calls to the same entity sets and actions that the rest of the product uses.

This keeps the workflow governed by the existing entities and avoids creating a second orchestration plane.

### 3. Use a Datadog-first observability story

The primary alert path is a generic external alert webhook that resolves a `Monitor` by its external monitor identifier and records `AlertFired` plus `AlertCycle.Open`. We keep the product language centered on monitors and alert cycles, but the ingest contract is shaped around Datadog-style alerts first because that is the simplest autonomous proof path for the demo.

GitHub merge webhooks are handled as a second path so PR completion can feed back into `WorkCycle` state.

### 4. Auto-spawn SRE from webhook-created alerts

When a webhook opens a real alert, the daemon should immediately create/configure/provision a SRE agent if enough project context is available. SRE is responsible for triage, PM issue creation, remediation handoff, and final alert closure.

This keeps the alert loop autonomous and makes the webhook path the real entrypoint into the self-heal system instead of a passive event recorder.

## Consequences

### Positive

- The demo story becomes coherent: channel orchestration, alert ingress, remediation, and reporting all run through one daemon.
- External webhooks reuse the governed OData surface instead of bypassing it.
- Each milestone has a replayable proof script, which makes progress honest and comparable.

### Negative

- The daemon now owns more orchestration responsibility, including background reporting after SRE completes.
- Some later phases still depend on external credentials or a human operator, so not every proof can be executed in a fully isolated environment.

### Risks

- If webhook payloads do not contain enough project context, SRE auto-spawn may open an alert without enough information to remediate.
- Proactive channel reporting depends on resolving the right channel/thread context; the fallback heuristics are good for the demo but not yet a fully modeled entity relationship.
- GitHub merge updates are only as reliable as the PR metadata included in the payload or already stored on the `WorkCycle`.
