# Agent Architecture Guide

## Foundational Context

TemperPaw is built on [Temper](https://github.com/nerdsane/temper). Development of both projects happens in tandem — architectural decisions must be clean across both codebases. Sometimes this means making changes to Temper itself to unblock or properly support TemperPaw features.

TemperPaw is Temper-native: all functionality MUST be built using Temper primitives (Temper apps — entity specs, WASM integrations, Cedar policies). There is no separate orchestration layer. If Temper doesn't support what you need, the answer is to extend Temper, not to work around it.

You are an autonomous agent running on the Temper platform. This guide defines how you should build and extend the platform. Read this before making architectural decisions.

## The Entity-First Rule

If state changes, it's an entity. If logic runs on a state change, it's a WASM integration.

Never write orchestration in imperative code (Rust, Python scripts, background tasks). Instead:

1. Define the state machine (IOA spec in `.ioa.toml`)
2. Wire WASM integrations on the actions that need logic
3. Use Cedar policies for authorization

## The Trigger Boundary

External events enter Temper through triggers. A trigger:

- Creates ONE entity
- Dispatches ONE action
- Returns immediately

Everything after that first action is WASM integrations reacting to state transitions. If you need to add a new external event source, create a config entity (like WebhookRoute), not Rust code.

## Self-Reporting

When your work is done (or failed), YOU dispatch the appropriate action on the workflow entity via `temper_action`:

- `AlertCycle.HealComplete` — you fixed the issue
- `AlertCycle.TuneComplete` — you tuned the monitor (noise)
- `AlertCycle.Escalate` — you cannot safely remediate
- `WorkCycle.PassTests` / `WorkCycle.Fail` — your code fix succeeded/failed

Don't rely on external watchers to detect your completion. HeartbeatMonitor handles timeout safety for crashed agents.

## Anti-Patterns

| Don't | Do Instead |
|-------|------------|
| `tokio::spawn` for business logic | WASM integration on entity action |
| Polling in Rust (`sleep` + `loop`) | Self-loop action with `check_count` / `max_checks` |
| Creating entities in a Rust loop | WASM integration creating entities on state transitions |
| Calling external APIs from Rust | WASM with secrets from `[integration.config]` |
| Background watchers for agent completion | Agents self-report; HeartbeatMonitor handles timeouts |
| Orchestration in `crates/temperpaw/` | Orchestration in `os-apps/*/wasm/` |

## The Audit Test

Ask: **"Can someone understand this entire flow by reading entity state transitions alone?"**

If the answer is no, some logic is hiding in imperative code. Refactor it into entities + WASM integrations.

## Where Things Live

| Layer | What | Where |
|-------|------|-------|
| **Triggers** (Rust) | HTTP listener, WebSocket gateway, cron | `crates/paw-triggers/` |
| **Platform** (Rust) | OData API, WASM runtime, Cedar engine | `crates/temper/` |
| **Entity specs** (TOML) | State machines, action definitions | `os-apps/*/specs/` |
| **Integrations** (WASM) | Business logic on state transitions | `os-apps/*/wasm/` |
| **Agent definitions** | Agent identities and instructions | `os-apps/paw-agent/agents/` |
| **Skill definitions** | Reusable knowledge for agent prompts | `os-apps/paw-agent/skills/` |
| **Policies** (Cedar) | Authorization rules | `os-apps/*/policies/` |

## Red-Green TDD (Mandatory)

All code changes MUST follow red-green TDD:

1. **Red** — Write a failing test first that defines the expected behavior.
2. **Green** — Write the minimum code to make the test pass.
3. **Refactor** — Clean up while keeping tests green.

No implementation code is written before a failing test exists for it. This applies to WASM integrations, triggers, Cedar policies, and entity specs alike.

## End-to-End Verification (Mandatory)

Every implementation MUST be verified end-to-end before it is considered complete. Agents must:

1. **Build and run** — Compile the full project, start the server, confirm it boots clean.
2. **Exercise the feature** — Manually invoke the new functionality (dispatch actions, hit endpoints, send messages through transports) and confirm correct behavior.
3. **Simulate real usage** — Walk through the user-facing flow as a real user would: send a Discord/Slack message, trigger a webhook, approve a plan — whatever the feature touches.
4. **Check state transitions** — Query entities via OData to confirm state machines moved through the expected states.
5. **Record results** — Capture output/logs as evidence in the `.proofs/` report.

Passing unit tests alone is NOT sufficient. If you cannot run the system and observe the feature working, it is not done.

## Reference

- **ADR-0001**: Temper Paw Architecture — os-app pattern, thin daemon
- **ADR-0005**: Temper-Native Orchestration — entity-first, trigger boundary, self-reporting
- **`os-apps/paw-agent/wasm/`** — reference WASM module implementations
- **`os-apps/paw-channels/specs/channel.ioa.toml`** — reference entity + WASM integration pattern
