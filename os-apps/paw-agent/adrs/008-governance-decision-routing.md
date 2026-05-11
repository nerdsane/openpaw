# ADR-008: Governance Decision Routing

- Status: Accepted
- Date: 2026-05-11
- Deciders: TemperPaw maintainers
- Related:
  - `os-apps/paw-agent/wasm/monty_repl/src/dispatch.rs`
  - `os-apps/paw-agent/wasm/request_approval/src/lib.rs`
  - Temper ADR-0080: Agent-Governed Mutation Denials

## Context

TemperPaw agents call Temper tools through Monty. Temper owns Cedar authorization and records `PendingDecision` records when a denied action is eligible for human approval. TemperPaw's role is to notice a decision-bearing denial, pause the active `Session`, and route the approval request to the user's channel.

Some Paw helpers still used cross-tenant or nonexistent decision endpoints. That made a valid pending decision hard to inspect from an agent session and could produce unrelated authorization failures such as `GET /api/decisions` returning cross-tenant 403.

Temper now creates agent/session-scoped pending decisions for governed mutation denials, including WASM module upload/delete. Temper also exposes tenant-scoped decision lookup and owner-filtered tenant decision listing so the agent/session that caused a decision can inspect it without cross-tenant privileges.

## Decision

TemperPaw does not authorize Temper tools. It only performs tool availability checks, invokes Temper, parses decision-bearing denials, pauses sessions, and routes approval notifications.

Monty governance helpers must use tenant-scoped decision APIs:

- `temper.get_decisions()` calls `/api/tenants/{tenant}/decisions?status=pending`.
- `temper.poll_decision(id)` calls `/api/tenants/{tenant}/decisions/{id}`.
- Batchable decision reads use the same tenant-scoped routes.

The existing denial parser remains compatible with all Temper decision response shapes, including top-level `decision_id`.

## Consequences

### Positive

- Agents no longer need cross-tenant decision visibility for normal approval workflows.
- Approval routing remains unified: Temper creates decisions, TemperPaw routes them.

### Negative

- TemperPaw depends on Temper exposing tenant-scoped decision lookup and owner-filtered tenant decision listing.

## Non-Goals

- Do not add a separate Cedar policy engine or authorization layer to TemperPaw.
- Do not special-case WASM approvals in Paw.
- Do not change approval button semantics in this ADR.

## Rollback Policy

Revert Monty helper route changes if the Temper tenant-scoped decision APIs are rolled back.
