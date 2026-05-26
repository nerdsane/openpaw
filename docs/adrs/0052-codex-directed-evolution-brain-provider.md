# ADR 0052: Codex as the Directed-Evolution V1 Brain Provider

## Status

Accepted.

## Decision

V1 runs the directed-evolution brain through `paw-codex-worker`. The
`directed-evolution-run` mode consumes an `EVOLUTION_CAMPAIGN_PLAN_PATH`
manifest so arbitrary Temper-native subjects can supply their own traffic,
trial suite, metrics, capability decisions, generations and release controls.
The plan also declares its evaluator namespace and entity-set names, so the
runner does not depend on the Agent Answers evaluator namespace.
The `directed-evolution-demo` mode remains an Agent Answers convenience entry
point and can call Codex for a selection-design rationale when
`PAW_EVOLUTION_USE_CODEX=1`.
Deterministic smoke mode exercises the protocol without an external model call.
The `directed-evolution-mutate` mode asks Codex to edit a candidate workspace,
then rejects any change outside the Temper-native subject app directories
before Genesis publishes or installs that candidate. Its frozen evaluator
compatibility contract is campaign input, rather than a built-in dependency on
the Agent Answers interaction model.
Live Codex mode requires the seed and both selected candidate versions to be
immutable Genesis commit refs (`owner/app@hash`); it does not accept illustrative
candidate labels as releases.

The worker records Codex as a provider and communicates only through native
campaign actions. It does not embed a fixed fitness vector or mutate the
active evaluator while candidate trials are running. A future TemperPaw-native
brain can issue the same actions and replace Codex without changing campaign
state or Evolution Studio.

## Evidence And Release Control

The proof mode freezes evaluator-owned `TrialSuite` and `MetricDefinition`
records. A live run requires `EVOLUTION_VALIDATOR_EVIDENCE_PATH`, produced by
executing the frozen scenario against the exact pinned candidate refs; a
mismatched or absent record prevents release. The worker records a native
`ValidatorRun` for each validated selected candidate and attaches
simulated, real-traffic, and Datadog evidence locators, performs two automatic
local releases, then pauses and rolls back. New local
Datadog ingestion requires an execution-time `DD_API_KEY`; absent that key the
Datadog locator remains explicitly pending instead of claiming ingestion.

The paired Genesis lineage smoke publishes and installs two real Temper-native
subject versions before these refs are handed to this runner. This separation
keeps candidate bytes and installability in Genesis while campaign decisions and
human direction remain native directed-evolution records.
