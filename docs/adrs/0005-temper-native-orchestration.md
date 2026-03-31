# ADR-0005: Temper-Native Orchestration

## Status

Accepted

## Context

`webhooks.rs` grew to ~1400 lines of imperative Rust orchestrating the self-heal loop: agent spawning, completion watching via `tokio::spawn`, CI/CD closure (GitHub merge + deployment tracking), Datadog verification, and proactive reporting. This violated ADR-0001's principle that the daemon is a thin bootstrap layer with no agent-specific Rust code.

The orchestration ran outside Temper governance — no entity state machines, no Cedar policies, no audit trail for why agents were spawned or alerts were escalated. The entire webhook-to-resolution flow was invisible to the platform.

## Decision

### All orchestration uses entity state machines + WASM integrations

- Every stateful workflow is modeled as an entity with an IOA spec (`.ioa.toml`)
- Business logic on state transitions runs as WASM integrations
- Rust code is restricted to: triggers (protocol bridges), platform primitives, WASM host functions
- `crates/openpaw/` has NO business logic — it loads os-apps and starts triggers

### Triggers follow the ONE-ONE rule

- A trigger creates ONE entity and dispatches ONE action
- Everything after that first action is driven by WASM integrations on entity state transitions
- Adding a new external event source = creating a new route/config entity, not writing Rust

### Agents self-report via temper_action

- Agents dispatch actions on workflow entities (AlertCycle, WorkCycle) directly using their `temper_action` tool
- No external watchers polling for agent completion
- HeartbeatMonitor provides timeout safety net for crashed/stale agents

### External webhook processing is an auditable entity

- WebhookEvent entity with state machine: Received → Validated → Routed → Processed
- Full audit trail: every webhook is traceable to the entities and agents it spawned
- WebhookRoute entity configures source→destination routing without code changes

### Enforcement via Claude Code PreCommit hook

- An Architect Reviewer agent (`.claude/agents/temper-native-reviewer.md`) reviews all Rust diffs at commit time
- The reviewer is a proper agent with its own definition, not an ad-hoc prompt
- Blocks commits that introduce business logic in the trigger/binary layer

## Consequences

### Positive

- Every orchestration decision is auditable via entity state history
- Cedar policies govern all transitions
- New webhook sources added via config entities, no code changes required
- WASM integrations are hot-reloadable without binary restart
- Agents that build new workflows will naturally follow the entity-first pattern

### Negative

- WASM modules have more limited capabilities than Rust (no async, bounded execution time)
- Long-running operations need self-loop pattern (check_count/max_checks)
- Initial migration effort to move ~1300 lines of logic into WASM modules

### Risks

- Self-loop polling may need tuning for interval between iterations
- WASM `http_call` timeout limits may need adjustment for external API calls (GitHub, Datadog)
- The PreCommit hook adds latency to commits (spawns a Claude subagent)
