# TemperPaw — Claude Code Project Guide

> Synchronized with `AGENTS.md` (Codex) — identical rules. When you change one, mirror the other. Global rules live in `~/.claude/CLAUDE.md` / `~/AGENTS.md`; this file adds what is TemperPaw-specific.

## Foundational Context

TemperPaw is built on [Temper](https://github.com/nerdsane/temper). Development of both projects happens in tandem — architectural decisions must be clean across both codebases. Sometimes this means making changes to Temper itself to unblock or properly support TemperPaw features.

TemperPaw is Temper-native: all functionality MUST be built using Temper primitives (Temper apps — entity specs, WASM integrations, Cedar policies). There is no separate orchestration layer. If Temper doesn't support what you need, the answer is to extend Temper, not to work around it.

You are an autonomous agent running on the Temper platform. This guide defines how you should build and extend the platform. Read this before making architectural decisions.

## Naming & Sources of Truth

- **OpenPaw was rebranded to TemperPaw** — same project, not a separate brand. Don't coin new names; ask when unsure.
- **Genesis is the source of truth for TemperPaw apps.** Everything installs from Genesis — no GitHub-sourced, Docker-baked, or local-catalog app sources in production. After merging to `main` on GitHub, **publish to Genesis and verify the installed pinned ref (`owner/app@hash`) is up to date**. On divergence, Genesis wins; sync is bidirectional.
- When reporting git status, state branch AND remote host — `origin` is not always GitHub here.

## Worktree Discipline

- Work only in a git worktree branched from up-to-date `main` (`codex/<short-task-name>`). Never push or commit directly to `main`; never touch existing dirty checkouts.
- State which repo/worktree/branch you're on before mutating anything. Open a **draft PR as soon as changes begin**; exactly **one PR per repo per effort**.
- **TemperPaw PRs target `nikstern/temperpaw` with base `main`, never `nerdsane/temperpaw`.** Before creating or editing a PR, verify that repository and its default branch, and pass `--repo nikstern/temperpaw` explicitly. Never infer the PR target from a remote named `origin` or from the branch's upstream.
- Multi-repo efforts merge in dependency order: Genesis → Temper → TemperPaw → Katagami.
- Treat `.env` as shared local state; do not commit it or rewrite teammate credentials.

## The Entity-First Rule

If state changes, it's an entity. If logic runs on a state change, it's a WASM integration.

Never write orchestration in imperative code (Rust, Python scripts, background tasks). Instead:

1. Define the state machine (IOA spec in `.ioa.toml`)
2. Wire WASM integrations on the actions that need logic
3. Use Cedar policies for authorization

- Rust code is ONLY for: triggers (protocol bridges in `crates/paw-triggers/`), WASM host functions, platform primitives (`crates/temper/`).
- `crates/temperpaw/` has NO business logic — it loads os-apps and starts triggers.
- Test: if your Rust code creates entities or dispatches actions in a loop, it should be a WASM integration instead. See ADR-0005 for the rationale.

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

## Architecture Decision Records (Mandatory)

Any material architecture change MUST be captured in an ADR before implementation is considered complete. This includes changes to Temper apps, entity specs, WASM integrations, Cedar policies, storage/provenance models, deployment behavior, triggers, and agent capability surfaces.

Write app-scoped decisions in `os-apps/<app>/adrs/`. Write platform-wide decisions in `docs/adrs/`. If a change is deliberately too small for an ADR, record that judgement in the proof or PR notes.

## Red-Green TDD (Mandatory)

All code changes MUST follow red-green TDD:

1. **Red** — Write a failing test first that defines the expected behavior.
2. **Green** — Write the minimum code to make the test pass.
3. **Refactor** — Clean up while keeping tests green.

No implementation code is written before a failing test exists for it. This applies to WASM integrations, triggers, Cedar policies, and entity specs alike.

## End-to-End Verification (Mandatory)

Coding agents MUST verify every implementation end-to-end before considering it complete:

1. **Build and run** — Compile the full project, start the server, confirm it boots clean.
2. **Exercise the feature** — Manually invoke the new functionality (dispatch actions, hit endpoints, send messages through transports) and confirm correct behavior.
3. **Simulate real usage** — Walk through the user-facing flow as a real user would: send a Discord/Slack message, trigger a webhook, approve a plan — whatever the feature touches.
4. **Check state transitions** — Query entities via OData to confirm state machines moved through the expected states.
5. **Record results** — Capture output/logs as evidence in the `.proofs/` report (use `.proofs/TEMPLATE.md`). A task is not complete until the verification flow is executed and recorded.
6. **Verify deployed** — After merge, hot-deploy / publish to Genesis and verify live on Railway. **Find out exactly what is deployed — never guess.** Use **Datadog** to confirm behavior in production.

Do NOT rely solely on unit tests passing. If you cannot run it and see it work, it is not done. Hand over PR links, merge commits, deployment links, and live test results.

## Root Cause & Operations

- When something "keeps happening" or "didn't use to happen": find **what changed** (read the code, read Datadog), fix the root cause, and explain the causal story — why it started, what the fix is, how you verified. Never stack fixes on fixes.
- **Datadog first** for production diagnosis.
- **Every error, failure, and Cedar policy denial must surface to the human channel (Discord DMs).** Silent failure is itself a bug. Recoverable errors (re-auth, retries) must be operable by the human entirely from Discord — TemperPaw is standalone; needing a side agent to recover is a UX failure.
- Prefer event-driven designs over polling.
- Provider auth uses the Codex subscription/OAuth flow, not raw API keys, unless told otherwise.
- Batch pipeline jobs run at most **10 concurrent**.
- Long-running plans get recorded as Temper **goals** ("implement as a goal") so they survive sessions and context loss.

## Brand Voice (for any TemperPaw-facing copy or creative work)

- Anchors: **"Warm but hard. Old and existential."** Y2K / Ghost in the Shell / Evangelion vibe is load-bearing.
- Temper is a **machine tool — a machine for building machines**; incorporate that profoundly, not decoratively.
- Vibey, not religious. No bones imagery. Paw = mecha-animal (locked).
- Compiled brand documents stand on their own — never name other agent projects or prior-art brands in them.
- Down to earth, no drama, no literary devices, no vanity metrics.

## Implementation Notes

- Prefer real integrations over mocks when credentials are available.
- Keep the entity model aligned with the `TemperPaw` namespace and the `Agent` / `Soul` / `Memory` / `Skill` names.
- Commit in small, reviewable increments with clear messages so parallel implementations are easy to compare.

## Reference

- **ADR-0001**: Temper Paw Architecture — os-app pattern, thin daemon
- **ADR-0005**: Temper-Native Orchestration — entity-first, trigger boundary, self-reporting
- **`os-apps/paw-agent/wasm/`** — reference WASM module implementations
- **`os-apps/paw-channels/specs/channel.ioa.toml`** — reference entity + WASM integration pattern
