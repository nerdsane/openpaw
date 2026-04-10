# Open Paw Project Instructions

## Foundational Context

OpenPaw is built on [Temper](https://github.com/nerdsane/temper). Development of both projects happens in tandem — architectural decisions must be clean across both codebases. Sometimes this means making changes to Temper itself to unblock or properly support OpenPaw features.

OpenPaw is Temper-native: all functionality MUST be built using Temper primitives (Temper apps — entity specs, WASM integrations, Cedar policies). There is no separate orchestration layer. If Temper doesn't support what you need, the answer is to extend Temper, not to work around it.

## Worktree Discipline

- Work only on your assigned feature branch in a git worktree.
- Never push or commit directly to `main`.
- Treat `.env` as shared local state; do not commit it or rewrite teammate credentials.

## Proof Requirement

- Every completed verification step must produce a committed report in `.proofs/`.
- Use `.proofs/TEMPLATE.md` as the format for all reports.
- A task is not considered complete until the verification flow is executed and its results are recorded.

## Temper-Native Rule

All stateful orchestration MUST use entity state machines + WASM integrations. See `AGENTS.md` for the full guide and ADR-0005 for the rationale.

- Rust code is ONLY for: triggers (protocol bridges in `crates/paw-triggers/`), WASM host functions, platform primitives (`crates/temper/`).
- `crates/openpaw/` has NO business logic — it loads os-apps and starts triggers.
- A trigger creates ONE entity and dispatches ONE action. Everything after that is WASM.
- Agents self-report outcomes via `temper_action`. No background watchers.
- Test: if your Rust code creates entities or dispatches actions in a loop, it should be a WASM integration instead.

## Implementation Notes

- Prefer real integrations over mocks when credentials are available.
- Keep the entity model aligned with the `OpenPaw` namespace and the `Agent` / `Soul` / `Memory` / `Skill` names.
- Commit in small, reviewable increments with clear messages so parallel implementations are easy to compare.
