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

## Red-Green TDD (Mandatory)

All code changes MUST follow red-green TDD:

1. **Red** — Write a failing test first that defines the expected behavior.
2. **Green** — Write the minimum code to make the test pass.
3. **Refactor** — Clean up while keeping tests green.

No implementation code is written before a failing test exists for it. This applies to WASM integrations, triggers, Cedar policies, and entity specs alike.

## End-to-End Verification (Mandatory)

Coding agents MUST verify every implementation end-to-end before considering it complete. This means:

1. **Build and run** — Compile the full project, start the server, confirm it boots clean.
2. **Exercise the feature** — Manually invoke the new functionality (dispatch actions, hit endpoints, send messages through transports) and confirm correct behavior.
3. **Simulate real usage** — Walk through the user-facing flow as a real user would: send a Discord/Slack message, trigger a webhook, approve a plan — whatever the feature touches.
4. **Check state transitions** — Query entities via OData to confirm state machines moved through the expected states.
5. **Record results** — Capture output/logs as evidence in the `.proofs/` report.

Do NOT rely solely on unit tests passing. If you cannot run it and see it work, it is not done.

## Implementation Notes

- Prefer real integrations over mocks when credentials are available.
- Keep the entity model aligned with the `OpenPaw` namespace and the `Agent` / `Soul` / `Memory` / `Skill` names.
- Commit in small, reviewable increments with clear messages so parallel implementations are easy to compare.
