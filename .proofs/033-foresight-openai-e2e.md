# Proof Report: 033 — Foresight Engine E2E with OpenAI GPT-5

## Date
2026-04-07

## Branch / Commit
main @ af707551..1727cc37

## What Was Done

Full end-to-end Foresight Engine run on Deep Sci-Fi (arni-labs/deep-sci-fi) using GPT-5 via OpenAI Codex Max subscription. 3 Probes, 2 steps, event-driven architecture.

## Run Configuration

- **Model**: GPT-5 via OpenAI Codex (chatgpt.com/backend-api/codex/responses)
- **Probes**: 3 (Probe-Alpha, Probe-Beta, Probe-Gamma)
- **Steps**: 2 (day 1, day 3)
- **Target**: arni-labs/deep-sci-fi
- **Daemon**: port 3471, Turso file DB

## Entity Counts

| Entity | Count |
|--------|-------|
| Sessions | 14 (probe + convergence analyst) |
| Agents | 8 (3 probes + 5 convergence analysts) |
| Observations | 26 (step 0: 12, step 1: 10, step 2: 4) |
| Directions | 9 (3 archived step 0, 3 active step 1, 3 active step 2) |
| Steps completed | 2 |

## Direction Evolution

### Step 0 — Initial Exploration (Archived)

**Probe-Alpha** (019d69c0-1144):
> **Open the Platform: Public World API + Agent SDK with Domain Tracing**
> Expose a stable World/Dweller API and ship an Agent SDK that emits canonical domain events. Enable third-party builders and agents to inhabit worlds.

**Probe-Beta** (019d69c0-1151):
> **Trace-Native Simulation: Event-Sourced Worlds + Timeline Explorer**
> Instrument core simulation primitives as spans, persist append-only event log, build Timeline Explorer UI. Make worlds observable by design.

**Probe-Gamma** (019d69c0-115d):
> **Ship Canon Timeline: event-sourced, trace-correlated story runs**
> Record every agent/world mutation as an immutable event carrying request_id and trace/span linkage. Surface in-app Timeline with replay and fork.

### Step 1 — Focused MVPs (Active)

**Probe-Alpha** revised to:
> **Events-first Platform: Signed Story Event Stream + Minimal Commands; defer SDK/registry**
> Publish Story/World event schemas via SSE and Webhooks (HMAC-signed). Provide minimal idempotent commands. Defer plugin registry.

**Probe-Beta** revised to:
> **Ship P0 Timeline: Trace-Sourced World History (No-Frills UI)**
> Standardize span schema, persist append-only event log, expose minimal Next.js Timeline view. Ship fast to prove value.

**Probe-Gamma** revised to:
> **Double down: Ship Canon Timeline MVP (event log + trace-linked runs)**
> Add append-only Event table. Emit events from all world/agent mutations. Minimal Timeline viewer. Signed webhook for future plugins.

### Step 2 — Further Refinement (Active)

**Probe-Alpha** revised to:
> **Narrow the Opening: Event-First Interface (Ingest + Read-Only Streams), Defer Broad Write APIs/SDK**
> Define canonical domain event contract. Ship validated ingest endpoint + read-only event streams. Beta-gate via allowlist.

**Probe-Beta** revised to:
> **Ship Span-First Timeline Explorer (Make Traces User-Visible)**
> Expose existing spans as minimal Timeline Explorer in Next.js. Define stable span schema. Deep links to Datadog traces.

**Probe-Gamma** revised to:
> **Canon Timeline v0: minimal append-only Event Log with trace correlation**
> Append-only events table, middleware stamps request_id, /timeline API, basic Timeline viewer, signed webhooks.

## Convergence Pattern

All 3 probes independently converged on the same thesis across 2 steps:
**The telemetry/observability infrastructure should become the product's external interface — event streams and timelines, not a broad SDK.**

Step 0 → Step 1 evolution: Broad architectural visions narrowed to focused MVPs.
Step 1 → Step 2 evolution: MVPs refined with clearer scope boundaries ("defer", "beta-gate", "out of scope for v0").

## Known Issues

### 1. Probes only see telemetry signals
The knowledge graph is dominated by GitHub activity (commits, PRs) and Datadog signals. For deep-sci-fi, recent development was observability-focused, so probes naturally gravitated there. They don't know:
- What the product actually does (README truncated to 2000 chars)
- Where it's deployed or what real content exists
- The product vision beyond what's in commit messages

**Fix needed**: Include full README, deployment URLs, product description, and optionally the deployed content in the knowledge graph.

### 2. Convergence Analyst doesn't fire ConvergenceComplete
GPT-5 runs 65+ turns of analysis but doesn't call the required `temper.action("Projections", id, "ConvergenceComplete", {...})` before completing. Root causes:
- **Namespace mismatch**: `temper.action()` prepends `Temper.` but the CSDL expects `OpenPaw.Foresight.ConvergenceComplete`
- **Instruction buried**: PHASE 3 (the callback) is at the end of a 47-line prompt
- **PHASE 2 dependency**: needs file_id from file upload, which may fail

**Workaround used**: Manual `ConvergenceComplete` API calls to advance steps.

### 3. Direction count grows (should stabilize)
max_steps=2 produced 9 directions (3 per step × 3 steps including step 2 from manual advance). Direction versioning works (parent_direction_id links, old ones Archived) but the count grows rather than replacing.

## What Worked

- GPT-5 via OpenAI Codex subscription (SSE streaming, tool calling, multi-turn)
- Event-driven step chain (ProbeStepDone → handle_probe_done → Convergence Analyst)
- Direction versioning (Archive old → create revision with parent_direction_id)
- 3 independent probes producing genuinely different perspectives
- Convergence toward a common thesis across steps
- Daemon stable for 30+ minutes on port 3471
- SSE streaming with format-agnostic deframing in Temper host
- 4MB HTTP buffer handles large LLM responses

## Architecture Diagram

```
seed_foresight.py → ProductModel.Seed → seed_model WASM
                                         ↓
                                    knowledge.json (GitHub + Datadog)
                                         ↓
Projection.Start → spawn_probes WASM → 3 Agent+Session (GPT-5)
                                         ↓
                        Probes run independently, create Obs + 1 Dir each
                                         ↓
                        ProbeStepDone × 3 → handle_probe_done WASM
                                         ↓ (all reported)
                        Convergence Analyst session (GPT-5, 65+ turns)
                                         ↓
                        ConvergenceComplete → handle_convergence WASM
                                         ↓
                        Respawn probes with episodic memory + projected state
                                         ↓
                        AdvanceStep (increment counter)
                                         ↓
                        ... repeat for step 1 ...
                                         ↓
                        Projection.Complete
```
