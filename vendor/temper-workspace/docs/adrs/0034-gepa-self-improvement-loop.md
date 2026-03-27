# ADR-0034: GEPA-Based Self-Improvement Loop

- Status: Proposed
- Date: 2026-03-18
- Deciders: Temper core maintainers
- Related:
  - ADR-0012: Integration architecture (schedule effects, adapter pattern)
  - ADR-0013: Evolution loop agent integration (sentinel, MCP methods)
  - ADR-0031: Agent orchestration OS app (HeartbeatRun, adapter dispatch)
  - ADR-0033: Platform-assigned agent identity (`agentTypeVerified`)
  - `.vision/EVOLUTION.md` (evolution engine vision)
  - `crates/temper-evolution/` (existing O-P-A-D-I record chain)
  - `crates/temper-wasm/` (WASM integration engine)

## Context

Temper captures entity-level trajectory data (action, success/failure, from/to status) via `TrajectoryEntry` but does NOT capture agent-level execution traces — the reasoning, tool call sequences, conversation history, and decision rationale that agents produce during their work. This means the platform can detect WHAT went wrong (via sentinel: error rates, guard rejections) but not WHY agents struggle or HOW to improve.

The gap: GEPA (Guided Evolution of Pareto-optimal Artifacts, arXiv:2507.19457) uses execution traces as "gradients" for evolutionary optimization. Without rich traces, we cannot close the self-improvement loop where agents build and evolve their own tooling.

Today's state:
- **Sentinel** detects anomalies (error_rate_spike >10%, guard_rejection_rate >20%, no_activity) and generates O-Records/I-Records
- **Evolution records** (O-P-A-D-I chain) exist but the P→A→D flow is manual
- **Agent adapters** (claude_code, codex, openclaw, http) exist for spawning LLM processes
- **WASM integrations** run sandboxed computation (blob_adapter, http-fetch)
- **Verification cascade** (L0-L3) validates every spec change
- **Cedar policies** gate all actions with `agentTypeVerified` attribute

What's missing: rich trajectory capture (OTS format), automated GEPA loop, WASM computation for evolution, and the rebranding of OS Apps to Skills.

## Decision

### Sub-Decision 1: OTS Trajectory Capture

Adopt the Open Trajectory Specification (OTS) format from `nerdsane/ots` as the agent-level trace format. Copy `ots-core` into the workspace as `crates/temper-ots/` with DST adaptations:

- `HashMap` → `BTreeMap` (deterministic iteration)
- `Uuid::new_v4()` → `sim_uuid()` (deterministic IDs)
- `Utc::now()` → accept `DateTime<Utc>` parameter (callers use `sim_now()`)

OTS captures what `TrajectoryEntry` cannot: full conversation history (`OTSMessage` with reasoning), tool call sequences (`OTSDecision` with alternatives, choice, consequence), and decision evaluation with credit assignment.

**Storage**: New `ots_trajectories` table in per-tenant Turso DB. JSON blob with indexed columns (trajectory_id, agent_id, session_id, outcome, timestamp). Per-tenant because trajectories contain agent reasoning about tenant-specific entities.

**Capture point**: Instrument `crates/temper-mcp/src/runtime.rs` with a `TrajectoryBuilder` that accumulates turns from each MCP `execute` call. On session close, finalize and POST to server.

**Why this approach**: OTS is a comprehensive 28-type model covering messages, decisions, annotations, and context. Building our own would duplicate effort. DST adaptations are straightforward (3 mechanical transforms).

### Sub-Decision 2: GEPA Algorithm — WASM Integrations + Rust Primitives

Implement GEPA as a combination of:

1. **Pure Rust primitives** in `crates/temper-evolution/src/gepa/` — Pareto frontier management, scoring, reflective dataset extraction, replay logic. Unit-testable in isolation.

2. **WASM modules** in `wasm-modules/gepa/` — four modules (replay, score, pareto, reflective) that orchestrate the computation steps. Hot-deployable, sandboxed, follows existing WASM integration model.

3. **One new generic host function** — `host_evaluate_spec(ioa_source, state, action, params)` that evaluates a single transition against any IOA spec via host-side `TransitionTable`. This is a platform capability, not GEPA-specific. Data access uses existing `host_http_call` to query OData endpoints.

4. **`claude_code` adapter** for LLM-creative steps (mutation proposal, candidate evaluation, crossover).

**Why WASM over native adapter**:
- Hot-deployable: change scoring logic without server redeploy
- Sandboxed: WASM bugs can't crash the server; fuel metering prevents infinite loops
- Temper-native: consistent with blob_adapter production precedent
- TransitionTable stays on host: WASM calls `host_evaluate_spec()`, host runs temper-jit

**Why not all in WASM**: LLM-creative steps (mutation, evaluation) require spawning external processes (Claude CLI). This is what adapters do. WASM handles computation; adapters handle external I/O.

### Sub-Decision 3: EvolutionRun Entity — IOA Spec on Temper

The GEPA loop is orchestrated by an `EvolutionRun` IOA entity with 12 states:

`Created → Selecting → Evaluating → Reflecting → Proposing → Verifying → Scoring → Updating → AwaitingApproval → Deploying → Completed | Failed`

Each GEPA step maps to an entity action with an integration:
- LLM steps: `[[integration]] type = "adapter" adapter = "claude_code"`
- Computation steps: `[[integration]] type = "wasm" module = "gepa-*"`

**Verification retry loop**: When L0-L3 cascade rejects a proposed mutation, the entity transitions `Verifying → Reflecting` (not `Failed`). Verification errors become part of the reflective dataset fed back to the LLM. Budget: `max_mutation_attempts` (default: 3) before `Failed`.

**Why IOA entity, not standalone Rust**: Governance. Cedar policies gate who can approve mutations. Entity state transitions are verifiable (L0-L3 cascade on the EvolutionRun spec itself). Telemetry captures every step. The entity IS the audit trail.

### Sub-Decision 4: Autonomy Slider via Cedar Policies

Three autonomy levels, controlled by Cedar policies on `EvolutionRun`:

1. **Full-human** (default): Only principals with `agent_type == "Human"` can approve
2. **Supervised**: Verified agents (`agentTypeVerified == true`) can approve low-risk mutations (`resource.risk_level == "low"`)
3. **Full-auto**: Any verified agent can approve (entity field `autonomy_level == "auto"`)

Self-approval prohibition in all modes: `forbid` when `resource.proposer_agent_id == principal.id`.

**Why this approach**: Reuses existing `agentTypeVerified` attribute from ADR-0033. No Cedar engine changes needed — just policy definitions per tenant.

### Sub-Decision 5: Sentinel Triggering — Agent-Initiated + Self-Scheduling Entity

**v1**: Agent (Claude Code) calls `check_sentinel()` on demand, creates `EvolutionRun` if high-priority alerts exist. Zero new infrastructure.

**v2**: `SentinelMonitor` entity using self-scheduling pattern (ADR-0012):

```
Active → [CheckSentinel] → Checking → [AlertsFound] → Triggering → [CreateEvolutionRun] → Active
                                      ↘ [NoAlerts] → Active
effect = [{ type = "schedule", action = "CheckSentinel", delay_seconds = 300 }]
```

The entity IS the cron job. Model-checkable, deterministic, verifiable.

New sentinel rule: `ots_trajectory_failure_cluster` — >5 OTS failures on same entity type in last hour. Reads from `ots_trajectories` table.

**Why not `tokio::time::interval`**: Breaks DST compliance. The self-scheduling pattern is the Temper way — schedule effects are model-checked, deterministic, and governed.

### Sub-Decision 6: OS Apps → Skills Rebranding

Rename "OS Apps" to "Skills" throughout the codebase:

- `os-apps/` → `skills/`
- `install_app()` → `install_skill()`
- `installed_apps` → `installed_skills` (Turso schema)
- API routes: `GET /api/skills` (old `/api/apps` kept as alias)

Each skill gets a `skill.md` with TOML frontmatter (`+++` delimited) for machine-parseable metadata and Markdown body for agent-readable guidance:

```markdown
+++
name = "project-management"
entity_types = ["Issue", "Project", "Cycle", "Comment", "Label"]
dependencies = []
+++

## When to use
...
## Available actions
...
## Example workflows
...
```

**Why TOML frontmatter + Markdown**: TOML = machine-parseable for indexing (consistent with IOA TOML). Markdown = LLM-readable natural language. Matches EvoSkill research pattern (SKILL.md with structured headers).

**Why rename**: "Skills" reflects the vision — agents build, evolve, and consume these capabilities. "OS Apps" implies developer-authored static applications.

### Sub-Decision 7: `host_evaluate_spec` — Generic Platform Capability

New WASM host function: `host_evaluate_spec(ioa_source, state, action, params) → result`

This is a generic platform capability, not GEPA-specific. Any WASM module can validate a transition against an IOA spec. Host-side implementation builds `TransitionTable::from_ioa_source()` (temper-jit) and evaluates the transition.

Data access for WASM modules uses existing `host_http_call` to query OData endpoints — no new host function needed for data.

**Why not a GEPA-specific host function**: Generic host functions benefit all future WASM modules. Testing modules, validation modules, simulation modules all need spec evaluation.

## Rollout Plan

1. **Phase 0** — ADR-0034 (this document)
2. **Phase 1** — `temper-ots` crate (copy + DST adapt OTS types)
3. **Phase 2** — MCP trace capture (instrument runtime.rs, OTS Turso table)
4. **Phase 3** — GEPA core (Rust primitives + host function + WASM modules)
5. **Phase 4** — Evolution entity (EvolutionRun + SentinelMonitor IOA specs)
6. **Phase 5** — Sentinel bridge (OTS rule, suggested_evolution_target)
7. **Phase 6** — Apps → Skills rebrand + skill.md format
8. **Phase 7** — E2E integration test (flawed PM skill → evolution → fix → verify)

Phases 1, 3a, 3b, 5a, 6a can proceed in parallel after this ADR.

## Readiness Gates

- `temper-ots` types serialize/deserialize correctly with BTreeMap/sim_uuid
- `host_evaluate_spec` WASM host function passes round-trip tests
- EvolutionRun IOA spec passes L0-L3 verification cascade
- SentinelMonitor IOA spec passes L0-L3 verification cascade
- GEPA WASM modules invoke successfully with mock context
- E2E test: flawed spec → failures → sentinel → evolution → mutation → verify → deploy → retry succeeds
- `cargo test --workspace` passes

## Consequences

### Positive
- Agents can self-improve their tooling through the GEPA loop
- Full execution traces captured for analysis, replay, and RL training (OTS format)
- Evolution is governed: Cedar policies enforce autonomy levels
- All computation is Temper-native: WASM for computation, adapters for LLM, entities for orchestration
- `host_evaluate_spec` is a generic platform capability benefiting all future WASM modules
- Skills are hot-deployable: WASM modules and spec mutations deploy without server restart

### Negative
- Complexity: ~40 new files across multiple crates
- OTS crate is a copy, not a dependency — must manually sync upstream changes
- WASM modules require separate compilation step (`cargo build --target wasm32-unknown-unknown`)
- Apps → Skills rename touches many files and documentation

### Risks
- LLM-proposed mutations may fail verification repeatedly (mitigated: 3-attempt budget, verification errors fed back to LLM)
- OTS trajectory storage could grow large in production (mitigated: per-tenant, retention policies, JSON blob only loaded on demand)
- Self-scheduling entity (SentinelMonitor) could consume resources if check interval is too low (mitigated: configurable delay_seconds, default 300s)

### DST Compliance

- `temper-ots`: All constructors accept `DateTime<Utc>` (callers use `sim_now()`), `sim_uuid()` for IDs, `BTreeMap` for deterministic iteration
- GEPA Rust primitives: Pure functions with `BTreeMap`, no I/O, no randomness
- `host_evaluate_spec`: Uses `TransitionTable` which is DST-compliant (temper-jit)
- EvolutionRun entity: Standard IOA entity, model-checked by L0-L3
- SentinelMonitor entity: Uses schedule effects (DST-compliant per ADR-0012)
- WASM modules: Fuel-metered, memory-limited, deterministic execution

## Non-Goals

- OpenClaw or TemperAgent trace capture (future work)
- RL fine-tuning with OTS exports (OTS supports Unsloth export, but training is out of scope)
- Vector embedding / similarity search for skill retrieval (future phase)
- Production background sentinel cron (v2 SentinelMonitor entity covers this)

## Alternatives Considered

1. **GEPA as a standalone Rust crate (no WASM)** — Algorithm logic as direct Rust function calls from entity handlers. Rejected: not hot-deployable, computation outside the integration model, inconsistent with platform philosophy.

2. **GEPA via custom native adapter** — New `gepa` adapter registered in AdapterRegistry. Rejected: adapters are for external I/O (spawning processes, HTTP calls). In-process computation is better served by WASM which provides sandboxing and hot-deployment.

3. **GEPA-specific host functions** — `host_load_ots_trajectories`, `host_pareto_check`. Rejected in favor of generic `host_evaluate_spec` + existing `host_http_call` for data access. Generic functions benefit all future WASM modules.

4. **`tokio::time::interval` for sentinel scheduling** — Background timer like optimization_loop. Rejected: breaks DST compliance. Self-scheduling entity pattern (ADR-0012) is model-checkable and deterministic.

5. **YAML frontmatter for skill.md** — Common in Jekyll/Hugo. Rejected: IOA specs use TOML, consistency favors TOML frontmatter (`+++` delimited).

## Rollback Policy

- `temper-ots` crate can be removed without affecting existing functionality (new crate, no existing deps)
- WASM modules can be unregistered from WasmModuleRegistry
- EvolutionRun/SentinelMonitor entities can be uninstalled via skill removal
- Apps → Skills rename can be reverted via git (alias routes preserved for backward compat)
- `host_evaluate_spec` host function is additive (existing WASM modules unaffected)
- OTS Turso table can be dropped without affecting existing trajectory data
