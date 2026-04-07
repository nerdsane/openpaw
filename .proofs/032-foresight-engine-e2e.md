# Proof Report: 032 — Foresight Engine End-to-End

## Date
2026-04-07

## Branch / Commit
main @ d9e59ae7 (fix: foresight session wiring — sandbox bypass, agent_id linkage, failure guard)

## What Was Done

Ran the complete Foresight Engine pipeline end-to-end: ProductModel seeding, Projection lifecycle with 2 Probe agents across 5 steps, Convergence Analyst spawning at each step, and Observation/Direction creation.

## Verification Flow

1. Built daemon + all 3 WASM modules (seed_model, spawn_probes, advance_step)
2. Started daemon on port 3469 with fresh Turso DB
3. Verified Soul bootstrap (Probe soul Active)
4. Seeded ProductModel for arni-labs/deep-sci-fi (GitHub + Datadog signals)
5. Configured Projection with 2 sonnet Probes, step_schedule=[1,3,7], max_steps=12
6. Started Projection — triggering spawn_probes WASM
7. Monitored Sessions through completion across multiple steps
8. Verified Convergence Analyst agents spawned and ran
9. Checked Observations, Directions, and Confirmed Observations

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| ProductModel seeds to Active | Active with knowledge graph | Active, model_snapshot_file_id set, knowledge graph JSON with 7 keys | PASS |
| Projection starts and spawns 2 Probes | 2 Agent+Session pairs created | 2 Agents (Probe-1, Probe-2) + 2 Sessions with agent_id set | PASS |
| Sessions provision without TensorLake | sandbox_url="none" bypasses provisioning | Sessions go Provisioning -> SandboxReady -> Thinking | PASS |
| Probes call Anthropic API and think | Sessions reach Thinking with turns > 0 | Both sessions completed: 19 turns + 13 turns | PASS |
| Probes create Observations | At least 3 per Probe | 10 Observations from step 0 alone (5 per Probe) | PASS |
| Probes create Directions | At least 2 per Probe | 6 Directions from step 0 (3 per Probe) | PASS |
| advance_step detects all Probes done | Finds sessions via agent_id filter | Correctly detected 2/2 done, advanced step | PASS |
| Convergence Analyst spawned per step | 1 analyst agent per completed step | 5 Convergence Analysts created (steps 0-4) | PASS |
| Convergence Analyst confirms Observations | Dispatches Confirm action on converging pairs | 6 Observations transitioned to Confirmed status | PASS |
| Probes respawned for next step | New Sessions created for existing Agents | Respawned sessions visible across 5 steps | PASS |
| Multi-step execution | Steps advance 0 -> 1 -> 2 -> ... | Projection completed at step 5 with status=Complete | PASS |
| All-failed guard prevents cascade | If all probes 429, Projection fails cleanly | Tested separately: Projection -> Failed, only 2 sessions created | PASS |
| PollProbes doesn't increment counter | Poll checks don't runaway | current_step stayed at expected values | PASS |

## What Worked

- Full entity lifecycle: ProductModel (Created -> Seeding -> Active), Projection (Created -> Running -> Complete)
- spawn_probes WASM creates Agent+Session pairs with correct agent_id linkage
- advance_step WASM correctly polls probe session status via OData filter
- Convergence Analyst spawning: 5 analysts created (one per step), each a task agent with no soul
- Semantic convergence detection: 6 Observations confirmed by the step-0 analyst
- Multi-step execution: 5 steps completed with probe respawning at each step
- PollProbes/StepComplete scheduling loop works correctly (15s intervals, no counter increment)
- sandbox_url="none" bypasses TensorLake provisioning cleanly
- All-failed guard prevents infinite session cascade on rate-limit errors

## What Didn't Work

- GitHub API returned 401 for all signal fetches (token issue) — knowledge graph had structure but zero signals
- Probes focused heavily on "zero signals" meta-observation rather than product direction (because the knowledge graph was mostly empty)
- ConfirmationNote field on Confirmed Observations is empty (Confirm action dispatched but note not persisted)
- ObservationCount/DirectionCount counters on Projection remained at 0 (these are not automatically incremented by entity creation)

## Limitations

- API rate limit sharing: daemon and Claude Code session share the same API key, causing 429 errors during concurrent use
- Knowledge graph quality depends on GitHub/Datadog API access — without valid tokens, probes have little to work with
- max_steps=12 with step_schedule=[1,3,7] means steps 3+ extrapolate the last schedule entry (7 days)

## What Still Doesn't Work

- Probes reading each other's Observations is blocked by instruction, but the API doesn't enforce it (no Cedar rule)
- DirectionFeedback workflow untested (no reviewer probes configured)
- Branch action on Projection untested

## Artifacts

### Entity Counts (final)
- ProductModels: 1 (Active)
- Projections: 1 (Complete, step 5)
- Agents: 7 (2 Probes + 5 Convergence Analysts)
- Sessions: 19 (18 Completed, 1 Thinking)
- Observations: 72 (6 Confirmed, 66 Created)
- Directions: 43 (all Proposed)
- DirectionFeedback: 0

### Sample Directions (from step 0)
1. "World-Building Engine: AI-Powered Sci-Fi Universe Construction"
2. "Sci-Fi Corpus Intelligence: Deep Analysis of the Genre's Knowledge Base"
3. "Sci-Fi as an AI Evaluation Domain: Open Benchmark for Speculative Reasoning"
4. "Become the Worldbuilding Engine for Sci-Fi Creators"
5. "Build a Science-Grounded Speculative Fiction Research Tool"
6. "Narrow to a Single Sci-Fi Universe as a Vertical Demo, Then Expand"

### Sample Observations (confirmed by Convergence Analyst)
- Both probes independently observed zero signals in the ProductModel
- Both probes independently observed the product name semantics ("deep" + "sci-fi")
- Both probes independently observed Datadog configured but with zero monitors

## Architecture Diagram
```text
seed_foresight.py
    |
    v
[ProductModel] --Seed--> seed_model WASM --> knowledge.json in TemperFS
    |                                          |
    v                                          |
[Projection]  --Start--> spawn_probes WASM <---+
    |                      |
    |                      +--> Agent(Probe-1) + Session --> llm_caller --> Anthropic API
    |                      +--> Agent(Probe-2) + Session --> llm_caller --> Anthropic API
    |                                                                |
    |  <--ProbesReady-- [probe_agent_ids set]                       |
    |                                                                v
    +--advance_step WASM                              Observations + Directions
    |   |                                             created via temper_create
    |   +-- checks all Probe sessions (OData query)
    |   |   if not all done: StepComplete -> PollProbes (15s)
    |   |   if all done:
    |   |     +-- spawn Convergence Analyst --> Session --> llm_caller
    |   |     +-- respawn all Probes for next step
    |   |     +-- return AdvanceStep (increments counter)
    |   |
    |   +-- if all failed: return Fail
    |   +-- if current_step >= max_steps: return Complete
    |
    v
[Projection.Complete]  (5 steps, 72 observations, 43 directions)
```
