# Directed Evolution Local Feedback Closure Proof

Date: 2026-06-03

This proof records the strongest proof cycle available without production
Temper credentials. It is not a substitute for the required live Agent Answers
Datadog proof, but it does exercise the current Genesis-authored Directed
Evolution app, shared `paw-orchestration` worker entities, and the local
TemperPaw worker loop against a fresh local Temper runtime.

## Runtime

- Genesis worktree:
  `/Users/seshendranalla/Development/genesis-worktrees/directed-evolution-live-backend`
- TemperPaw worktree:
  `/Users/seshendranalla/Development/temperpaw-worktrees/directed-evolution-hotload-variants`
- Local proof bundle:
  `/tmp/directed-evolution-local-proof-foreground.udcnRR`
- Local Temper URL: `http://127.0.0.1:3201`
- App catalog: temp symlink catalog containing:
  - `directed-evolution`
  - `paw-orchestration`
  - `agent-answers`
  - `agent-answers-evaluation`

Startup installed the relevant app surfaces:

```text
App 'agent-answers' installed for 'default': Answer, Question
App 'agent-answers-evaluation' installed for 'default': TrialMetricDefinition, TrialSuite, ValidatorRun
App 'directed-evolution' installed for 'default': AdaptationGoal, AutonomyPolicy, Direction, EliminationRule, Episode, EpisodeStartRequest, EvaluationStage, EvidenceArtifact, Generation, LineageEdge, Measurement, MetricDefinition, Mutation, Organism, OrganismVersion, Pressure, Promotion, ScoringRule, SelectionPressure, SelectionProtocol, Signal, SimulatedUserPlan, StageResult, Trial, Variant, ViabilityConstraint, WorkItemReceipt
App 'paw-orchestration' installed for 'default': BudgetLedger, HeartbeatRun, Organization, WorkItem, WorkerAgent, WorkerProvider, WorkerRun
```

Metadata checks confirmed these live entity/action surfaces:

```text
EntitySet Name="Organisms" True
EntitySet Name="EpisodeStartRequests" True
EntitySet Name="WorkItems" True
Action Name="SubmitEpisodeStartRequest" True
```

## Episode Start

The proof seeded a baseline Agent Answers organism and direction through real
OData create/action calls, then submitted `EpisodeStartRequest`:

```text
Organisms org-agent-answers create -> 201
OrganismVersions ov-agent-answers-local-proof create -> 201
OrganismVersions ov-agent-answers-local-proof MarkOrganismVersionParent -> 200
Organisms org-agent-answers ActivateOrganism -> 200
Directions dir-agent-answers-local-proof create -> 201
Directions dir-agent-answers-local-proof ProposeDirection -> 200
EpisodeStartRequests episode-start-local-proof create -> 201
EpisodeStartRequests episode-start-local-proof SubmitEpisodeStartRequest -> 200
```

The inline `episode_start_requestor` path materialized:

```text
EpisodeStartRequests: count=1 statuses={'Started': 1}
Directions: count=1 statuses={'Selected': 1}
Episodes: count=1 statuses={'Running': 1}
Generations: count=1 statuses={'Generating': 1}
WorkItems: count=3 statuses={'Queued': 3}
```

The started episode was
`en-019e8af2-b5b2-7200-b291-1916d528457c`, with generation
`en-019e8af2-b97d-7b22-8232-40d7c50a5dec`.

## Worker Loop

The local worker was started with execution disabled:

```sh
TEMPER_URL=http://127.0.0.1:3201 \
TEMPER_TENANT=default \
WORKER_ID=local-proof-worker \
PAW_CODEX_ENABLE_EXECUTION=0 \
PAW_CODEX_POLL_ON_START=1 \
./target/debug/paw-codex-worker run
```

It registered, connected to the local event stream, claimed Directed Evolution
work items, created shared `WorkerRun` rows, and routed receipts back into
Directed Evolution. Final captured state:

```text
WorkItems: count=13 statuses={'Succeeded': 9, 'Cancelled': 4}
WorkerRuns: count=9 statuses={'Succeeded': 9}
EvidenceArtifacts: count=9 statuses={'Linked': 9}
Variants: count=3 statuses={'Eliminated': 3}
Trials: count=3 statuses={'Succeeded': 3}
StageResults: count=9 statuses={'Eliminated': 3, 'Pending': 3, 'Running': 3}
Episodes: count=1 statuses={'Failed': 1}
Generations: count=1 statuses={'Failed': 1}
```

The episode failed closed because dry-run worker output is not a viable real
variant/evaluation proof:

```text
All variants were eliminated before selection.
```

That failure is expected for this local proof. The proof still demonstrates the
fresh feedback-closure mechanics through episode materialization, shared work
item creation, worker claiming, worker-run provenance, evidence artifact
linking, trial routing, and fail-closed elimination.

## Remaining Live Proof Blocker

The required production Agent Answers proof remains blocked. Production
preflight bundle
`/tmp/paw-patrol-production-preflight-20260603T001306Z-903` reported missing:

- `TEMPER_URL`
- `WORKER_TOKEN`
- `PATROL_OPERATOR_TOKEN`
- signed webhook secrets
- launchd plist/load state
- production local Codex worker-id confirmation
- checkout ancestry against current main

Datadog MCP access was available, and recent TemperPaw telemetry could be read,
but no fresh Directed Evolution / Agent Answers Datadog proof trace was found.
The live proof can only be closed after those production credentials and worker
activation gates are supplied.
