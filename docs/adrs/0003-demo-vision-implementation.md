# ADR-0003: Demo Vision Implementation

## Status

Accepted

## Context

OpenPaw has a working agent loop (Scout → Developer → PR) proven in Proofs 006-008, but the loop is manually triggered via OData calls and Python scripts. The vision requires: human talks to Paw on Discord, says "manage deep-sci-fi", and the system autonomously monitors, triages, fixes, and reports. This ADR documents the phased approach to close that gap.

## Decision

### 1. Phased implementation with proof gates

Each phase produces a committed proof report in `.proofs/` before the next phase begins. No phase is considered complete without end-to-end verification. Phases 1-6 require no human interaction — they're fully provable via curl/OData/Python scripts.

### 2. Webhook ingestion in the openpaw crate

External alerts (Logfire, Datadog, GitHub) enter through `POST /webhooks/ingest` — a new route in the openpaw binary, nested alongside the platform router. This keeps the single-binary deployment model from ADR-0001. The webhook handler is an OData client internally — it dispatches entity actions the same way the Discord transport does.

**Rationale:** Adding a separate service would complicate deployment. The webhook handler is stateless — it just translates external payloads into OData actions.

### 3. Logfire-first observability strategy

Logfire (by Pydantic, built on OpenTelemetry) is the first observability platform. Datadog follows later. Both support webhook-triggered alerts with the same integration pattern: monitor fires → webhook → OpenPaw ingests → AlertCycle created → Scout triages.

**Rationale:** Logfire is simpler to set up, has a friendlier API for developers, and the tool_runner already has a `query_logfire` tool.

### 4. Scout auto-spawn on alert

When the webhook handler creates an AlertCycle, it also spawns a Scout agent to triage it. This closes the gap between "alert exists" and "someone is investigating." The Scout uses the same soul and tool pipeline already proven in Proof 007.

**Rationale:** Manual Scout spawning via OData was the biggest friction point. Auto-spawn makes the system truly reactive.

### 5. Guided-but-flexible soul instructions

Paw's soul describes available entity types and their purpose but does not prescribe rigid step-by-step workflows. Paw is an intelligent agent — it uses judgment about what to set up based on what the human asks.

**Rationale:** Hardcoding flows defeats the purpose of an intelligent agent system. The soul is guidance, not a script.

## Phases

1. **Webhook ingestion** — `POST /webhooks/ingest` creates AlertCycle from external payloads
2. **Scout auto-spawn** — webhook handler spawns Scout on AlertCycle creation
3. **Paw orchestration** — Paw handles "manage project" via OData channel (curl-provable)
4. **E2B self-heal** — full Scout → Developer → PR in E2B sandbox
5. **PM integration** — Scout creates PM Issues when triaging alerts
6. **Proactive reporting** — Paw sends summaries via Channel after self-heal completes
7. **Discord e2e** — prove Discord DM flow (requires human)
8. **Full demo** — complete end-to-end via Discord (requires human)

## Consequences

### Positive
- Each phase is independently verifiable
- Phases 1-6 can be automated in CI
- Webhook ingestion enables real production monitoring
- Paw orchestration via OData channel means the same code path works for Discord and any future transport

### Negative
- Webhook handler in the binary means recompile for ingestion changes (acceptable: changes are rare)
- Scout auto-spawn means every alert gets a triage agent (cost concern at scale — addressed by monitor tuning)

### Risks
- Paw's flexible soul instructions may not reliably produce the right entity setup on every invocation — mitigated by proof testing
- Logfire API changes could break the integration — mitigated by the tool_runner's existing abstraction
