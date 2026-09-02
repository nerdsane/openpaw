# paw-patrol

Operational control plane for TemperPaw maintaining itself.

All external requests and machine signals enter Patrol first. Patrol decides
whether the input becomes real work, links or creates paw-pm Issues when useful,
and runs the implementation loop through Temper-visible state transitions.

## Entity Types

### WorkRequest
WorkRequest means human or manager-agent intent: "do this work", "clean this
up", or "investigate this thing." OpenClaw, Discord, or a human dashboard
submits here instead of writing directly to paw-pm.

### Intent
Intent is WorkRequest renamed to the SDLC vocabulary (ARN-441, stage 3 phase 4):
the same intake shape. `Accept` creates an `Effort`. Triage ("worth
doing?") is a different lifecycle than execution ("done right?"), so Intent stays
the intake. Additive during the shadow phase — WorkRequest stays live for the
paw-codex-worker and dashboard until the phase-3 flip, then retires.

### PatrolRequest
Legacy name for request intake. New intake must use WorkRequest; PatrolRequest
stays readable only for old entity history and compatibility tests.

### Signal
Signal means observed evidence: a machine-observed event from Discord, Datadog,
GitHub, schedules, or repo sweeps. `Signal.Ingest` routes actionable failures
through `signal_router`; obvious noise is archived visibly.

### PatrolRun
PatrolRun means active investigation. Risk Patrol creates a PatrolRun for
agent-driven sweeps such as `datadog_observability` and `github_repository`;
the run queues a capable WorkerRun, records evidence, opens durable
findings/cases/work, and completes or escalates through Temper actions.

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

Plans are visible WorkCycle state, not private worker scratch. Intake WASM
modules write a structured plan-mode markdown plan, and the local Codex worker
runs a read-only Codex Plan Mode pass before implementation. That pass revises
the WorkCycle plan through `WorkCycle.RevisePlan`, increments
`plan_revision_count`, and injects the approved plan into the mutating Codex
run.

### Effort
Effort is WorkCycle EXTENDED to the full SDLC lifecycle (ARN-441, stage 3 phase
4): born at intent and carried to a verified deploy. The implementer is a
harness Agent: edits are native, recorded acts go through Temper MCP, machine
work is Computer.Exec. It begins `Intended` → `Specified` → `Planned` →
`Building` → `InReview` → `Proving` → `Merged` → `Deploying` → `Verified`.
`Stalled` is recoverable; `Abandoned` is the give-up terminal. The row holds
the chain (intent_id, review_run_ids, proof_packet_ids, ask_ids, deployment_id,
pm_issue_id). Ask is the inbox (decide / do / fyi). `spec_ref` / `plan_ref` / `intent_ref` /
`decisions_ref` are git paths (`docs/efforts/<issue>/*.md`). AttachSpec /
AttachPlan / AttachDecisions / AttachIntentFile run `chain_github_ready`
(the path must be a file on GitHub at repo@branch). Review and proof stay
Temper Files. `Merge` is the Cedar door (L0/L1 permit; L2+ denies and surfaces as
MCP elicitation). `Deploy` records the id of a project tool — create `DsfDeploy` or
`TemperDeploy`, `Effort.Deploy` with that id, then `Request` the child.
Those tools report back:
Healthy → `MarkDeployVerified` → Verified; RolledBack → Merged; Failed →
Stall only if rollback did not finish. Additive until the phase-3 flip —
WorkCycle stays live until then.

### DsfDeploy
Deep Sci-Fi deploy tool. Merge the PR on the named computer, watch
`health_url` for the merge sha, git-revert if it never becomes healthy.
Reuses `release_run_lifecycle`. Live `ReleaseRun` rows are not renamed.

### TemperDeploy
TemperPaw image deploy tool. Upsert Railway `IMAGE_TAG`, redeploy, poll
`/paw/version` until the live sha matches, restore the previous tag on
rollback. Railway project/service ids are pinned in the trigger config.

### WorkerRun
One execution attempt by a registered worker. The first worker type is the Mac
mini local Codex worker. It claims queued work from Railway Temper over SSE,
starts local Codex with ChatGPT auth, and self-reports results.

### ReviewRun
One panel agent's pass (Grok, Codex, or Claude), not the whole panel. Each
agent writes their own ReviewRun. `Effort.review_run_ids` lists them.
`PassReview` is called after three ReviewRuns are attached (one per panel
agent). Each run is written with `RecordPanel` (Requested → Recorded) and
may hang a Temper File. `Approve` still fans into WorkCycle for the old
loop and is not the Effort path. The kernel door is `panel_started` (at
least one attach); three Recorded runs is the implementer rule. The
reviewer
inspects the diff and proof, reruns relevant checks, and returns approve,
request changes, or escalate. Findings live on the ReviewRun row. A Temper
File (HTML or JSON) can hang off the run as the readable artifact; GitHub
comments are not the record.

### EvaluationRun
Automated gate execution and result capture for tests, proof requirements,
policy gates, architecture checks, and targeted live verification evidence. If
an evaluation fails while the WorkCycle is in review, Patrol treats it like
requested rework and queues the implementer again with the failing evidence.

### ProofPacket
Human-readable and machine-readable proof. The record is the Temper row plus
an optional Temper File (HTML or JSON via `$value`). Vercel and GitHub are
not the source. The human view should include a visual one-page summary,
state-transition diagram, changed-files map, test matrix, reviewer verdict,
residual risks, PR links, entity links, and trace/log links.

### RiskRule
Explicit rule that sets a minimum risk lane from concrete evidence. Agents may
raise risk but cannot silently lower the rule-derived floor.

### RepoGraphSnapshot
Recurring codebase/dependency graph snapshot for routing, quality cleanup,
security sweeps, and agent orientation. The local Codex worker performs an
agent-led investigation of the repo/dependency graph, giant modules, duplicate
logic, specs, Cedar policies, WASM modules, dependencies, tests, proofs,
security drift, and readability. Patrol validates the structured findings,
opens QualityFinding/SecurityFinding entities, records the visual summary, and
dispatches `AssessmentComplete` from the agent evidence unless a configured real
assessment Session is available.

### QualityFinding
Readable code health finding: giant modules, duplicated logic, TODO/HACK
band-aids, missing proofs, hidden orchestration, polling loops, and related
cleanup debt.

### SecurityFinding
Security or authorization finding: Cedar drift, secret handling, dangerous tool
surface, dependency risk, or risky deploy/billing/provider change.

### DailyBrief
Daily visual and textual summary of completed work, open risks, new findings,
proof packets, and escalations. It is an agent-driven DailyBrief Session plus
local Codex WorkerRun: `daily_brief_lifecycle` gathers source facts, creates the
Session record, queues the local Codex WorkerRun, and attaches both. Codex writes
the visual daily summary through `DailyBrief.Render`, then self-reports through
the normal reviewer/evaluator/proof gates so the brief can include judgment,
diagrams, and readable prioritization.

### PatrolSchedule
Recurring Patrol job that schedules repo sweeps and daily briefs from Temper
state transitions. It uses `schedule_at` to fire `Trigger`, then
`patrol_schedule_lifecycle` creates RepoGraphSnapshot and DailyBrief entities.
Patrol seeds `patrol-default-daily-maintenance` as an active daily schedule so a
fresh install has the recurring maintenance loop visible in Temper immediately.

## PatrolSchedule And CronJob

PatrolSchedule intentionally does not reuse the paw-agent CronJob entity.
Both entities use Temper's schedule_at timer effect, but they are different
business state machines.

CronJob is for scheduled agent Session creation: each trigger computes the next
run and declaratively spawns a `Session`.
PatrolSchedule is for scheduled Patrol maintenance: each trigger creates
Patrol-native `RepoGraphSnapshot` and `DailyBrief` entities so repo health and
briefs stay in the Patrol audit graph.

If the cadence parsing or schedule conventions drift, factor shared helpers or
platform conventions. Do not route Patrol maintenance through CronJob unless
CronJob stops meaning "spawn a scheduled agent Session."

## Quality Cleanup Status

Detection is not cleanup. Patrol's repo sweep opens `QualityFinding` and
`SecurityFinding` entities for cleanup debt, and accepted findings become
Patrol `WorkCycle`s. The scanner makes giant WASM modules visible and
reviewable; it does not mark them fixed.

As of this implementation, giant WASM modules remain work to be done. Known
examples include Monty REPL, provider_caller, context_preparer, and
route_message. Each should become a scoped cleanup `WorkCycle` with reviewer,
evaluation, and proof gates before the cleaned area is ratcheted.

## Intake

Submit everything to Patrol first:

```text
You / OpenClaw / Discord / Datadog / GitHub / schedule
        |
        v
WorkRequest / legacy PatrolRequest / Signal / PatrolRun
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

Use the three intake shapes this way:

| Shape | Use it for | What Patrol creates |
| --- | --- | --- |
| WorkRequest | A human or manager-agent says "do this work" | FactoryCase, optional paw-pm Issue, WorkCycle, and a risk-gated WorkerRun |
| Signal | Observed evidence or error from Datadog, Discord, GitHub, a webhook, or another agent | Normalized/triaged Signal, then a FactoryCase and WorkCycle if actionable |
| PatrolRun | Active investigation by an agent, such as Datadog or GitHub patrol | WorkerRun for the patrol agent, evidence, Signals, findings, cases, work, and ProofPackets |

Do not submit new work directly to paw-pm. Paw-pm is durable project memory;
Patrol owns intake, triage, risk, worker assignment, review, evaluation, and
proof. Patrol creates or links paw-pm Issues only after it decides the input is
real work.

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

## Agent Submission API

Use these OData action paths when an agent, OpenClaw, Discord bridge, script, or
human operator submits work directly to Temper. The examples omit auth headers;
callers still need a valid bearer token and principal headers that Cedar allows.

### SDLC Intent (ARN-441)

Create an Intent, commit intent.md, name its git path, triage, accept.
Accept creates the Effort. Then commit spec.md / plan.md / decisions.md,
name those git paths, and walk Specify → Plan → StartBuild → … → Merge →
Deploy (record a DsfDeploy or TemperDeploy id).

```http
POST /tdata/Intents
{}

POST /tdata/Intents('<id>')/TemperPaw.Patrol.Submit
{
  "source": "harness",
  "request_text": "Ship the locked Stage 3 walk.",
  "requester_id": "cursor"
}

POST /tdata/Intents('<id>')/TemperPaw.Patrol.AttachIntentFile
{
  "intent_ref": "docs/efforts/ARN-441/intent.md"
}

POST /tdata/Intents('<id>')/TemperPaw.Patrol.Triage
{
  "triage_summary": "worth doing",
  "task_summary": "Stage 3 walk",
  "task_detail": "",
  "risk_lane": "L0",
  "repo": "nerdsane/temperpaw",
  "branch": "cursor/arn-441-stage3",
  "intent_ref": "docs/efforts/ARN-441/intent.md"
}

POST /tdata/Intents('<id>')/TemperPaw.Patrol.Accept
{
  "factory_case_id": ""
}
```

Expected result: Intent is Accepted. An Effort exists in Intended, seeded
with the intent.md git path. WorkRequest stays the live worker/dashboard path
until the phase-3 flip.

### Human or manager-agent task

Create a WorkRequest, then submit it:

```http
POST /tdata/WorkRequests
{}

POST /tdata/WorkRequests('<id>')/TemperPaw.Patrol.Submit
{
  "source": "openclaw",
  "request_text": "Fix the broken dashboard detail view and prove it live.",
  "requester_id": "openclaw"
}
```

Expected result: WorkRequest moves through Patrol routing, then links to a
FactoryCase. If the request is accepted, Patrol creates or links a paw-pm Issue,
creates a WorkCycle, applies RiskRule floors, and either queues a WorkerRun or
pauses in `AwaitingHumanStartApproval` for L3 work.

### Observed evidence or error

Create a Signal, then ingest the raw evidence:

```http
POST /tdata/Signals
{}

POST /tdata/Signals('<id>')/TemperPaw.Patrol.Ingest
{
  "source": "datadog",
  "payload": "{...raw alert, trace, log, webhook, or agent evidence...}",
  "source_url": "https://app.datadoghq.com/...",
  "severity": "error"
}
```

Expected result: Signal is normalized and triaged. Noise is archived visibly.
Actionable evidence links to a FactoryCase, WorkCycle, WorkerRun, and ProofPacket
through `signal_router`.

### Active patrol run

Create a PatrolRun, configure the patrol kind and worker capability, then start
it:

```http
POST /tdata/PatrolRuns
{}

POST /tdata/PatrolRuns('<id>')/TemperPaw.Patrol.Configure
{
  "patrol_kind": "datadog_observability",
  "summary": "Investigate current TemperPaw/TemperPaw runtime health.",
  "requested_by": "risk-patrol",
  "required_capabilities": "datadog_query"
}

POST /tdata/PatrolRuns('<id>')/TemperPaw.Patrol.Start
{}
```

For GitHub issue and PR patrols, use:

```json
{
  "patrol_kind": "github_repository",
  "summary": "Triage open issues, PRs, checks, reviews, and repository anomalies.",
  "requested_by": "risk-patrol",
  "required_capabilities": "github_query"
}
```

Expected result: `patrol_run_lifecycle` queues a WorkerRun for a worker that
advertises the required capability. The patrol agent does read-only
investigation through its tools, returns structured evidence, and Patrol records
the evidence through `PatrolRun.RecordEvidence`. Datadog patrol fans out to
Signals, ObservabilityFindings, FactoryCases, risk-gated WorkCycles, and visual
ProofPackets. GitHub patrol fans out to Signals, FactoryCases, WorkCycles, and
ProofPackets for actionable repository issues or PR anomalies.

## Review And Rework Loop

Every implementation WorkerRun is followed by an independent ReviewRun,
EvaluationRun, and ProofPacket. You should not be the first reviewer of normal
agent work.

```text
WorkerRun.ReportDone
        |
        v
ReviewRun + EvaluationRun + ProofPacket draft
        |
        +--> ReviewRun.Approve        -> evaluation/proof can complete
        +--> ReviewRun.RequestChanges -> WorkCycle.RequestChanges
        +--> ReviewRun.Escalate       -> WorkCycle.Fail + FactoryCase escalation
        +--> ReviewRun.Fail           -> WorkCycle.Fail + FactoryCase escalation
```

`ReviewRun.RequestChanges` is the ordinary unhappy path. Patrol dispatches
`WorkCycle.RequestChanges`, creates a new implementer WorkerRun, reuses the
same branch/worktree when the previous WorkerRun had one, and prompts the
implementer with the reviewer feedback. This can repeat until review and
evaluation pass.

`ReviewRun.Escalate` means the reviewer cannot safely decide. Typical reasons:
the change is security-sensitive, policy-sensitive, production-impacting,
deployment/secrets/billing/data-migration related, user-facing in a risky way,
or the available proof is not enough for an agent to approve.

`ReviewRun.Fail` means the reviewer machinery failed rather than judged the
implementation. Examples: the reviewer timed out, crashed, could not write back
to Temper, or the reviewer output is invalid because it did not include a
recognized verdict marker.

Webhook intake is provided by `paw-ingest`, with routes seeded by Patrol:

```text
POST /triggers/webhook/patrol-request
  -> WebhookEvent -> WorkRequest.Submit

POST /triggers/webhook/patrol-signal
POST /triggers/webhook/patrol-datadog
POST /triggers/webhook/patrol-github
POST /triggers/webhook/patrol-discord
  -> WebhookEvent -> Signal.Ingest
```

Use `patrol-request` for human or manager-agent asks. Use the signal routes for
observed failures, alerts, traces, GitHub events, and Discord incidents.
PatrolRequest remains a legacy entity set, but new human or manager-agent work
flows through `WebhookEvent -> WorkRequest.Submit`.

## WASM Modules

Patrol's business logic lives in WASM integrations on entity actions. The Rust
server hosts Temper and triggers, but these modules own the workflow decisions:

- `patrol_request_router`: turns an accepted `WorkRequest` or legacy
  `PatrolRequest` into a
  `FactoryCase`, optional paw-pm Issue linkage, `WorkCycle`, and queued
  `WorkerRun`.
- `patrol_run_lifecycle`: starts Risk Patrol investigations such as Datadog
  Observability Patrol, picks a registered worker with the required
  capabilities, and escalates visibly if no capable worker is available.
- `signal_router`: routes Datadog, Discord, GitHub, and other machine signals
  into Patrol cases, work cycles, and worker assignments when the signal is
  real work.
- `repo_sweep_lifecycle`: starts repo graph scans, assigns the local worker,
  and fans scan results into quality and security findings.
- `worker_run_lifecycle`: reacts to worker success or failure, records proof
  evidence, and starts the independent review/evaluation gates.
- `review_gate_lifecycle`: applies reviewer verdicts and evaluation results to
  the `WorkCycle` and final `ProofPacket`.
- `finding_lifecycle`: turns accepted quality/security findings into cleanup
  `WorkCycle`s and links the source finding to the resulting work.
- `work_cycle_lifecycle`: handles high-risk approval transitions, completion,
  failure, and source-finding resolution.
- `patrol_schedule_lifecycle`: keeps recurring Patrol schedules inside Temper
  by creating repo sweeps and daily briefs from schedule transitions.
- `daily_brief_lifecycle`: queues the local Codex DailyBrief WorkerRun from
  finished proof packets, findings, and open risks.
- `chain_github_ready`: GET the repo (tenant `github_token` must be able to
  see it), then GET contents for `docs/efforts/<id>/*.md`. Fired by
  AttachIntentFile / AttachSpec / AttachPlan / AttachDecisions. A 404 on the
  repo is "token cannot see this repo", not a missing file.
- `chain_file_ready`: GET a Temper File and require Ready/Locked. Fired by
  AttachReviewFile / AttachProofFile.
- `temper_deploy_lifecycle`: one side effect per TemperDeploy trigger
  (IMAGE_TAG swap, /paw/version poll, restore previous tag).
- `release_run_lifecycle`: also runs DsfDeploy merge / watch / revert.

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

If a schedule fails because a required WASM module, policy, or secret was not
loaded yet, repair the missing dependency and dispatch `PatrolSchedule.Recover`.
Recovery recomputes `next_run_at` through the same `patrol_schedule_lifecycle`
activation path and keeps the repair visible in the entity history.

## Mac Mini Worker

The Mac mini runs `paw-codex-worker` under launchd as the `openclaw` user. The
worker connects outbound to Railway TemperPaw's `/tdata/$events`, watches for
`WorkerRun.Queued`, claims work through Cedar, starts local Codex, then reports
completion back to Temper. Codex subscription auth and Datadog MCP auth also
belong to `/Users/openclaw`, so worker worktrees, env files, launchd plist, and
doctor checks all stay under that user.
Patrol sets `WorkerRun.allowed_worker_id` from `local_codex_worker_id` so only
the registered local worker principal can claim the queued run. Patrol sets
`WorkerRun.worktree_path` from `local_codex_worktree_root` so queued runs point
at the worker host's worktree root rather than the machine that submitted the
request or signal.
Patrol also sets `WorkerRun.required_capabilities`; Datadog Patrol requires
`datadog_query`, so a generic local Codex worker cannot silently claim
observability work without the Datadog read capability.

This is resource-bound ownership. The principal ID identifies the caller; the
resource assignment field identifies which exact Temper entity the caller is
allowed to operate on. A worker principal with `id = "worker-a"` is not
authorized merely because it is a worker. It is authorized for a WorkerRun only
when that WorkerRun carries `allowed_worker_id = "worker-a"` for claim or
`worker_id = "worker-a"` for start/report actions. Review and evaluation use
the same pattern with `reviewer_id` and `evaluator_id`, so a valid reviewer can
review only the ReviewRun assigned to that reviewer.

For L3 requests, no `WorkerRun.Queued` event exists until a human or supervisor
approves `WorkCycle.ApproveHumanStart`. After proof gates pass, Patrol waits for
`WorkCycle.ApproveHumanCompletion` before marking the WorkCycle and FactoryCase
complete.

## OpenClaw

OpenClaw may act as a manager surface: summarize events, route approvals, and
submit WorkRequests, Signals, or PatrolRuns. It should not write code or mutate
repo files in v1.
