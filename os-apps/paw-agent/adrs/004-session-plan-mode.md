# ADR-004: Session Plan Mode

**Status:** Accepted
**Scope:** integrations, entity-specs
**Author:** seshendra
**Date:** 2026-04-09

## Context

Agents have Plan entities and a planning phase in WorkCycle/Issue lifecycles, but nothing enforces that an agent actually stays in "planning mode." Agent instructions say "plan first, then implement" but the agent has full write access the whole time and can skip planning entirely.

Claude Code's plan mode demonstrates the value of a real mode boundary: when planning, the agent physically cannot make changes. It can only read, explore, research, and produce a Plan artifact. When ready, it switches to execute mode and gets full tools — all within the same session, same sandbox, same conversation context.

The infrastructure already supports this:
- `tools_enabled` is a CSV field on Session that `monty_repl` enforces on every tool call via `ensure_method_enabled()`. Changing it mid-session takes effect immediately.
- `SwitchProvider` is a self-loop action that modifies session state mid-run — the exact pattern for `SwitchMode`.
- `assemble_system_prompt()` runs on every `call_llm` invocation (not cached). It reads entity state each time, so mode-conditional prompt injection works naturally.
- All three LLM providers (Anthropic, OpenRouter, OpenAI) are stateless — system prompt changes are picked up on the next call with no provider-specific code needed.

## Decision

### 1. `session_mode` field + `SwitchMode` self-loop action

Add a `session_mode` string field to Session (`"plan"` or `"execute"`, default `"execute"`). A `SwitchMode` self-loop action (identical pattern to `SwitchProvider`) updates `session_mode` and `tools_enabled` atomically. The monty_repl already re-reads `tools_enabled` on every tool call, so the restriction takes effect on the very next call after the switch.

Supporting fields:
- `pre_plan_tools_enabled` — stashes the original `tools_enabled` CSV so it can be restored on switch to execute
- `active_plan_id` — links to the Plan entity being worked on

### 2. Tool enforcement in plan mode

Block tools whose primary purpose is mutation (`write`, `edit`, `temper_submit_specs`, `temper_upload_wasm`, etc.). Allow general-purpose tools needed for exploration (`bash`, `read`, `temper_get`, `temper_list`, `temper_read`, `temper_web_search`, `temper_web_fetch`). Also allow `temper_create`, `temper_action`, and `temper_write` so agents can create/update Plan entities and write plan documents to TemperFS.

Bash can technically be used destructively, but the dedicated write tools being infrastructure-blocked provides the enforcement boundary — same approach as Claude Code.

### 3. Mode-conditional prompt injection

Plan-mode instructions are NOT a system skill (per ADR-003, system skills are unconditionally fully injected into every prompt). Instead, they live at `/system/mode-instructions/plan.md` and are conditionally injected by `assemble_system_prompt()` as step 3b (between skills and memory) only when `session_mode=plan`.

When the agent switches to execute mode with an `active_plan_id`, the Plan entity's content is injected as an `<active_plan>` section so the agent has its plan as context.

### 4. Two flavors of mode switching

**Self-directed** — Agent calls `temper.switch_mode({"mode": "plan"})` or `temper.switch_mode({"mode": "execute"})` at will. No approval gate.

**Approval-gated** — Agent creates Plan, submits for review, pauses (`PauseForPlanApproval` → `WaitingForApproval`). On `Plan.Approve`, a WASM integration dispatches `ResumeWithPlanApproval` on the session, switching to execute mode.

Both use the same `SwitchMode` infrastructure. The difference is who triggers it (agent vs. system WASM via `plan_approval_handler`).

### 5. File-backed plan content

Follow the Soul (`content_file_id`) and Agent (`instructions_file_id`) pattern: plan body lives as a TemperFS markdown file via `plan_file_id`, not crammed into an entity string field. Exploration notes live in a separate `exploration_file_id`. This supports extensive plans without bloating entity state, and lets the dashboard render rich markdown.

Plan entities gain `UpdatePlan` and `AddExplorationNote` self-loop actions in Draft state for iterative refinement, each with an increment counter.

### 6. Structured plan formats

Two formats defined in the plan-mode instructions:

- **Focused plans** (single feature/fix) — 7 sections: Context, Exploration Summary, Approach, File Manifest, Verification, Risks, Open Questions
- **Multi-phase plans** (projects, refactors) — Extended format with Work Streams (parallel tracks with owners/deps/steps), Phase Gates (sync checkpoints), and Dependency Graphs

Exploration notes are kept in a separate TemperFS file with structured findings, verified assumptions, and file references.

## Consequences

### Positive

- Agents can't skip planning when mode is set — enforcement is infrastructure-level, not instruction-level
- Same session context preserved across mode switches — no re-provisioning, no lost conversation state
- Plan entity becomes a first-class planning artifact with iterative editing and structured review
- Leverages existing infrastructure (`tools_enabled`, `SwitchProvider`, `WaitingForApproval`) — minimal new machinery
- Both self-directed and approval-gated flows use the same mechanism
- Multi-phase plan format supports long-running, multi-agent work

### Negative

- Plan-mode instructions consume prompt tokens when active (mitigated: only injected when `session_mode=plan`)
- `bash` remains available in plan mode and can technically be used for writes (same trade-off as Claude Code — blocking dedicated tools provides the meaningful boundary)
- Agents must learn the `temper.switch_mode()` API — requires skill/instruction updates
- The `plan_approval_handler` WASM module adds a new cross-entity integration pattern
