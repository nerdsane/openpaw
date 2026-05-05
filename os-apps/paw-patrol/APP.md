# paw-patrol

Operational control plane for TemperPaw maintaining itself.

All external requests and machine signals enter Patrol first. Patrol decides
whether the input becomes real work, links or creates paw-pm Issues when useful,
and runs the implementation loop through Temper-visible state transitions.

## Entity Types

### PatrolRequest
Human or manager-agent request submitted into Patrol. OpenClaw, Discord, or a
human dashboard submits here instead of writing directly to paw-pm.

### Signal
Machine-observed event from Discord, Datadog, GitHub, schedules, or repo sweeps.
`Signal.Ingest` routes actionable failures through `signal_router`; obvious
noise is archived visibly.

### FactoryCase
The operational case that groups one request or signal, its risk floor, linked
paw-pm Issues, WorkCycle, worker runs, reviews, evaluations, and proofs.

### WorkCycle
The Patrol-owned implementation contract. It replaces the need for a separate
paw-harness app for this factory slice and tracks planning, implementation,
testing, review, proof, and completion. L3 work pauses in
`AwaitingHumanStartApproval` before any WorkerRun is queued, then pauses again
in `AwaitingHumanCompletionApproval` after reviewer, evaluator, and ProofPacket
gates pass.

### WorkerRun
One execution attempt by a registered worker. The first worker type is the Mac
mini local Codex worker. It claims queued work from Railway Temper over SSE,
starts local Codex with ChatGPT auth, and self-reports results.

### ReviewRun
Independent reviewer pass over the implementer's diff and proof. The reviewer
must inspect the implementation, rerun relevant checks, and run live or E2E
verification when the touched surface requires it.

### EvaluationRun
Automated gate execution and result capture for tests, proof requirements,
policy gates, architecture checks, and targeted live verification evidence.

### ProofPacket
Human-readable and machine-readable proof. The human view should include a
visual one-page summary, state-transition diagram, changed-files map, test
matrix, reviewer verdict, residual risks, PR links, entity links, and trace/log
links.

### RiskRule
Explicit rule that sets a minimum risk lane from concrete evidence. Agents may
raise risk but cannot silently lower the rule-derived floor.

### RepoGraphSnapshot
Recurring codebase/dependency graph snapshot for routing, quality cleanup,
security sweeps, and agent orientation.

### QualityFinding
Readable code health finding: giant modules, duplicated logic, TODO/HACK
band-aids, missing proofs, hidden orchestration, polling loops, and related
cleanup debt.

### SecurityFinding
Security or authorization finding: Cedar drift, secret handling, dangerous tool
surface, dependency risk, or risky deploy/billing/provider change.

### DailyBrief
Daily visual and textual summary of completed work, open risks, new findings,
proof packets, and escalations.

### PatrolSchedule
Recurring Patrol job that schedules repo sweeps and daily briefs from Temper
state transitions. It uses `schedule_at` to fire `Trigger`, then
`patrol_schedule_lifecycle` creates RepoGraphSnapshot and DailyBrief entities.
Patrol seeds `patrol-default-daily-maintenance` as an active daily schedule so a
fresh install has the recurring maintenance loop visible in Temper immediately.

## Intake

Submit everything to Patrol first:

```text
You / OpenClaw / Discord / Datadog / GitHub / schedule
        |
        v
PatrolRequest or Signal
        |
        v
FactoryCase + risk floor
        |
        +--> optional paw-pm Issue
        |
        v
WorkCycle -> WorkerRun -> ReviewRun -> EvaluationRun -> ProofPacket
        |
        v
Review + evaluation + proof gates close WorkCycle before human escalation
```

High-risk work follows the same visible state graph, but with human approval
gates:

```text
L3 WorkCycle.Planned
        |
        v
AwaitingHumanStartApproval
        |
        | ApproveHumanStart
        v
WorkerRun -> ReviewRun -> EvaluationRun -> ProofPacket
        |
        v
AwaitingHumanCompletionApproval
        |
        | ApproveHumanCompletion
        v
Complete
```

Webhook intake is provided by `paw-ingest`, with routes seeded by Patrol:

```text
POST /triggers/webhook/patrol-request
  -> WebhookEvent -> PatrolRequest.Submit

POST /triggers/webhook/patrol-signal
POST /triggers/webhook/patrol-datadog
POST /triggers/webhook/patrol-github
POST /triggers/webhook/patrol-discord
  -> WebhookEvent -> Signal.Ingest
```

Use `patrol-request` for human or manager-agent asks. Use the signal routes for
observed failures, alerts, traces, GitHub events, and Discord incidents.

## Default Schedule

Patrol seeds a default daily maintenance schedule:

```text
PatrolSchedule: patrol-default-daily-maintenance
        |
        v
ActivateComplete schedules next Trigger
        |
        v
Trigger creates RepoGraphSnapshot + DailyBrief
        |
        v
TriggerComplete schedules the next daily Trigger
```

Pause or edit that PatrolSchedule in Temper if production should wait before
daily repo sweeps and briefs begin.

## Mac Mini Worker

The Mac mini runs `paw-codex-worker` under launchd. The worker connects outbound
to Railway TemperPaw's `/tdata/$events`, watches for `WorkerRun.Queued`, claims
work through Cedar, starts local Codex, then reports completion back to Temper.
Patrol sets `WorkerRun.allowed_worker_id` from `local_codex_worker_id` so only
the registered local worker principal can claim the queued run.

For L3 requests, no `WorkerRun.Queued` event exists until a human or supervisor
approves `WorkCycle.ApproveHumanStart`. After proof gates pass, Patrol waits for
`WorkCycle.ApproveHumanCompletion` before marking the WorkCycle and FactoryCase
complete.

## OpenClaw

OpenClaw may act as a manager surface: summarize events, route approvals, and
submit PatrolRequests. It should not write code or mutate repo files in v1.
