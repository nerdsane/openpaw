# ADR-0001: Patrol-Controlled Dark Factory

Date: 2026-05-05

Status: Accepted

## Context

TemperPaw needs to maintain itself and its tightly coupled Temper dependency
through visible, reviewable, mostly autonomous work loops. The loop needs to
accept human requests, manager-agent requests, Discord/Datadog/GitHub signals,
and recurring repo-health sweeps without creating a second orchestration system
beside Temper.

Earlier designs used names like factory, quality, harness, and paw-heal. For
TemperPaw v1, splitting those concerns into separate apps would make the flow
harder for humans and agents to read. The operator should be able to inspect
Temper state and understand intake, risk, implementation, review, evaluation,
proof, cleanup, and daily briefs as one Patrol-owned state graph.

The AGENTS.md architecture guide requires material app architecture changes to
be recorded in app-scoped ADRs and requires the system to remain
Temper-native: state changes are entities, logic on state changes is WASM, and
authorization is Cedar.

## Decision

Build the Dark Factory as the `paw-patrol` Temper app.

`paw-patrol` owns these Temper entities:

- `PatrolRequest`
- `Signal`
- `FactoryCase`
- `WorkCycle`
- `WorkerRun`
- `ReviewRun`
- `EvaluationRun`
- `ProofPacket`
- `RiskRule`
- `RepoGraphSnapshot`
- `QualityFinding`
- `SecurityFinding`
- `DailyBrief`
- `PatrolSchedule`

`paw-pm` remains the durable project memory app for Issues, Projects, and
Cycles. New work enters Patrol first. Patrol creates or links `paw-pm` Issues
only after triage decides a request, signal, or finding is real work.

`paw-ingest` remains the external trigger boundary. Webhook triggers create a
`WebhookEvent`, then WASM routes it into either `PatrolRequest.Submit` or
`Signal.Ingest`. Rust trigger code returns after the initial action and does
not own the business workflow.

`paw-codex-worker` is the local Mac mini executor. It connects outbound to the
Railway TemperPaw control plane, claims queued `WorkerRun`s only when Cedar
allows its registered worker principal, runs local Codex with ChatGPT/Codex
auth, and self-reports through `WorkerRun.ReportDone` or
`WorkerRun.ReportFailed`. It may also run independent reviewer and evaluation
passes when the queued Patrol state requires them.

Risk is controlled by explicit `RiskRule` floors stored in Temper. Agents may
raise risk based on evidence but cannot lower the rule-derived floor. L3 work
is human-gated before implementation starts and again after review, evaluation,
and proof gates pass.

Every implementation must produce a `ProofPacket` with machine-readable fields
and human-readable proof. Factual diagrams and SVG summaries are derived from
structured proof state, not from free-form narrative alone.

## Architecture

```text
PatrolRequest / Signal / RepoGraphSnapshot / QualityFinding / SecurityFinding
        |
        v
FactoryCase + RiskRule floor
        |
        +--> optional paw-pm Issue
        |
        v
WorkCycle
        |
        +--> WorkerRun
        +--> ReviewRun
        +--> EvaluationRun
        |
        v
ProofPacket
        |
        v
Complete, request changes, fail, or human-gated escalation
```

The production control plane is the Railway TemperPaw instance with embedded
Temper. Local development Temper is for staging and tests. The Mac mini talks
to Railway over HTTPS/OData/event streams; it does not share a production
database directly.

## Consequences

- Humans and agents can audit the maintenance loop by reading Patrol entity
  transitions and OData links.
- The current codebase cleanup effort is part of Patrol, not a side app:
  repo sweeps create `QualityFinding` and `SecurityFinding` entities, accepted
  findings become `WorkCycle`s, and completed cleanup resolves source findings.
- Review happens before human review. The reviewer inspects the implementer's
  diff and proof, reruns relevant checks, and can require changes or escalate.
- Automated `EvaluationRun`s capture test suites, targeted commands, and live
  or E2E evidence when relevant.
- Recurring `PatrolSchedule`s can create repo sweeps and `DailyBrief`s through
  Temper-visible state transitions.
- Cedar policy is part of the design surface: worker claims, reviewer verdicts,
  and human-gated risk lanes must remain authorization-visible.

## Rejected Alternatives

### Separate factory, quality, or harness apps

Rejected for v1. Separate apps would split one operational loop across too many
namespaces. `paw-harness` can remain as historical or future reusable template
work, but TemperPaw's self-maintenance flow lives in `paw-patrol`.

### A Rust orchestration daemon that owns the workflow

Rejected. Rust may host triggers or the local worker process, but workflow
state and business decisions must remain in Temper entities, WASM integrations,
and Cedar policies.

### Codex Cloud as the default executor

Rejected for v1 cost and control reasons. Codex Cloud can be manual overflow
after Temper approval, but the default executor is local Codex on the Mac mini.

## Verification

The architecture is ratcheted by
`crates/temperpaw/tests/paw_patrol_foundation.rs` and by the one-command
acceptance harness in
`crates/paw-codex-worker/scripts/paw-patrol-acceptance.sh`.

The proof trail is recorded in:

- `docs/proofs/2026-05-04-paw-patrol-dark-factory-foundation.md`
- `docs/proofs/2026-05-05-paw-patrol-completion-audit.md`
- `docs/runbooks/paw-patrol-production-cutover.md`
