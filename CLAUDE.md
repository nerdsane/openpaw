# Open Paw Project Instructions

## Worktree Discipline

- Work only on your assigned feature branch in a git worktree.
- Never push or commit directly to `main`.
- Treat `.env` as shared local state; do not commit it or rewrite teammate credentials.

## Proof Requirement

- Every completed verification step must produce a committed report in `.proofs/`.
- Use `.proofs/TEMPLATE.md` as the format for all reports.
- A task is not considered complete until the verification flow is executed and its results are recorded.

## Implementation Notes

- Prefer real integrations over mocks when credentials are available.
- Keep the entity model aligned with the `OpenPaw` namespace and the `Agent` / `Soul` / `Memory` / `Skill` names.
- Commit in small, reviewable increments with clear messages so parallel implementations are easy to compare.
