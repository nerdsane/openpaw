# DSF software factory contract

## Outcome

A user asks an agent to change, investigate or explore DSF. The agent can inspect its live operational model, edit and test in a computer, raise a decision when required, request an operation on the correct resource, and verify the result. The user can join that same agent directly in the private Foundry workspace. After a run ends or a process restarts, the model and work records remain available and scheduled observation continues.

The scope includes delivery, operations and isolated exploration. DSF product features are changed only where needed to make those flows observable and safely operable. Building the future scientific prediction engine or migrating the product domain is outside this effort.

## Sources of state

Temper owns operational resources, observations, operations, experiments and the existing Intent/Effort/Ask/review/proof records. Foundry owns its computer-run lifecycle, command mailbox and transcript. A stable effort/run association connects them. Provider identifiers and observed revisions identify external resources. Source files and Datadog links support interpretations; neither a human label nor a successful HTTP transport response substitutes for evidence.

The new application belongs in `os-apps/dsf-factory`, published to Genesis with a pinned installed revision. Existing SDLC changes belong in `paw-patrol`. Foundry integration stays in the private `arni-labs/foundry` fork. Necessary DSF application instrumentation and startup corrections stay in `arni-labs/deep-sci-fi`. Kernel changes require an actual missing primitive and a separately recorded architectural decision.

## Operational records

Use app-specific entity names to avoid existing tenant collisions. The concrete model includes:

| Record | Meaning and required linkage |
|---|---|
| DsfResource | Application/environment containers and actual deployable or external resources. Kind, parent, provider identity, source repository, intended configuration, observed configuration, latest revision and observation references. |
| DsfFlow | A real application flow, its source revision, entry points, dependent resource IDs and outcome definitions. |
| DsfParticipant | An identified DSF API participant or an aggregate anonymous-reader cohort, with identity provenance and activity window. No API keys, content or invented individual browser identities. |
| DsfObservation | Immutable evidence summary with subject, source, query/window, observed time, coverage, outcome and evidence reference. Measured, absent, inaccessible and stale data are distinct. |
| DsfOperation | One requested operation owned by a resource and linked to an Effort. Immutable target and operation key, intended revision/configuration, provider execution ID, actual result and verification evidence. |
| DsfModelSync | Bounded observation/reconciliation execution, source cursor/revision, last successful observation and next due time. Failure preserves prior facts and makes staleness visible. |
| DsfExperiment | An isolated variant with its Effort, branch, computer, actual data/media bindings, results and cleanup record. Selection returns to delivery; it never directly replaces production. |

The first resource graph represents the observed DSF deployment: Vercel production and staging frontends; Railway production API and Datadog collector; both distinct staging APIs; Supabase production and staging databases; R2 media storage; the discovered foresight database and sleeping Bun service with unconfirmed ownership. Flow records cover agent registration/participation, story submission and media completion, story reading, feed browsing, world exploration, action processing, notification delivery and video generation.

Application and environment containers are resource kinds, not separate lifecycle types. Flow and participant records describe operation, not copies of DSF business objects. Dependencies use actual IDs and explicit provenance. Observers do not turn unconfirmed relationships into facts.

## Observation and reconciliation

Provider adapters collect deployed revisions, aliases, resource configuration and bindings. Source inspection updates flow and dependency descriptions. Datadog queries collect service/monitor status, request outcomes and bounded trace/metric summaries. DSF exposes authenticated operational aggregates where provider/telemetry data cannot establish queue, job or participant state.

Each adapter has a narrow typed result and redacts secrets at its boundary. It retains observation windows and coverage. Sampled spans are labeled sampled; metric counts, jobs, users and span counts are never conflated. An empty query result does not imply health. Missing permissions are reported as inaccessible rather than silently ignored.

Provider and monitor events request reconciliation where available. A bounded recurring reconciliation recovers missed events and marks stale observations. The state machine owns scheduling, retries and failure transitions. Agents interpret changed code and ambiguous operational findings using the same recorded work/Ask flow. Adapter code performs one collection or provider operation, not a hidden SDLC loop.

Repeated observation of the same source event is idempotent. Older data cannot overwrite a newer observation or intended configuration. Reconciliation never changes intended configuration. Drift creates or updates a single outstanding Ask for the same resource and drift fingerprint.

## Resource operations and delivery

An Effort retains specification, plan, decisions, review, proof and completion. Its deployment configuration names a resource and operation, instead of assuming every project deploys a TemperPaw image. Existing release behavior must keep working while callers move to the resource operation contract.

A resource operation proceeds through request, validation, execution, observation and verification. Each external concern is one integration. Declared state transitions sequence work. A WASM module never dispatches another transition itself.

Before an external write, deterministic validation requires the exact resource, allowed operation, linked Effort and evidence for the intended revision. Retries reuse the operation key. On uncertain provider responses, observe the provider execution before issuing another write. An unrelated successful provider deployment cannot satisfy this operation.

Verification requires provider state and application behavior. For the backend this includes exact revision, health body, schema and the changed flow; for the frontend it includes active alias/revision, compiled API target and browser behavior. Datadog verification checks the affected service/flow and distinguishes unavailable telemetry from a passing result. A rollback is a recorded operation that also needs observed verification.

The current backend startup migration-reset defect must be corrected and exercised against a disposable migrated database before routine backend deployment. The newer staging API currently points to production data/media and cannot serve as an isolated test target. Recovery endpoints need authentication and atomic claiming before they become permitted routine repair operations.

## Ongoing operations

Fresh observations can produce an investigation Effort. The agent establishes cause and a proposed repair, invokes an already permitted resource operation or raises an Ask for a new capability, then records the result and affected flow verification. Historical failed media is not automatically regenerated. Configuration drift remains a decision until accepted.

Provider API failures, stale credentials, missing telemetry, abandoned runs and unsuccessful repairs remain visible. A restart resumes from durable records. It must not replay an already completed deploy or silently mark interrupted work successful.

## Isolated exploration

An experiment records exact branch/revision, computer identity, database identity, media namespace and permitted external calls. Validation refuses production database/media bindings. A local disposable Postgres with pgvector and a private experiment namespace are sufficient; no staging name is treated as isolation evidence.

An agent can create and compare at least two variants, run the required application checks, record results, choose a candidate through the existing decision mechanism and clean up unused resources. Promoting a candidate creates ordinary delivery work and respects its review/proof gates.

## Foundry and agent access

Preserve Foundry's existing authenticated membership and durable mailbox. Admit workspace members with the agreed full control. Configure our own public origins, repository installation and provider credentials. Remove upstream deployment targets before any fork deployment. Keep images and source private.

An explicit machine-authenticated integration lets an outside agent start, steer and inspect runs in the configured factory organization. Do not copy a user's short-lived browser JWT or expose broad credentials to the browser. The resident harness receives a scoped Temper connection after Foundry writes its configuration. The default agent auth is the connected ChatGPT subscription; no API-key fallback.

The workspace shows the live resource graph, observations, operations, linked work, run transcripts and outstanding Temper Asks. Answering invokes the existing Temper action. Synchronous MCP elicitation must reach the human and return the actual response; it cannot be unconditionally accepted, declined or answered with an empty object. Resume uses the existing mailbox and rechecks the authoritative Temper state.

## Contract invariants

The readable contract, formal state model and executable fault-injection tests express the same invariants:

1. Observing state cannot change intended configuration or authorize an operation.
2. Operation resource, kind, effort and idempotency key cannot change after acceptance.
3. A retry or restart cannot cause a duplicate external write after its provider execution is known.
4. Verified requires the intended revision/resource, successful affected-flow proof and explicit telemetry coverage.
5. An unresolved blocking Ask prevents its dependent action; resolving an unrelated Ask cannot release it.
6. Experiment bindings exclude production data and media; promotion uses delivery.
7. Stale, missing or inaccessible evidence cannot become healthy by default.
8. Run completion is distinct from verified Effort completion; interrupted work resumes or fails explicitly.
9. Agent credentials, provider keys and private upstream source never enter public artifacts or browser responses.
10. Metered agent fallback is disabled, and additional resource/product verification spend stays within the authorized cap.

## Required proof

- Verify specs and policies, then exercise the same invariants under duplicate events, out-of-order observations, concurrent operations, lost responses and process restarts.
- Boot the production shape locally or on the governed computer using real configuration and disposable data before any production deploy.
- Refresh the real DSF graph from providers, code, Datadog and operational aggregates; prove a later observation updates the same identities without adopting drift.
- Start a subscription-authenticated agent inside a real computer; edit and test a scratch change; join and steer that same run through Foundry.
- Raise an actual decision, answer it through the UI, and prove the dependent agent continues exactly once. Exercise MCP elicitation separately from ordinary planning Asks.
- Complete a real resource-owned deployment with exact revision and affected-flow/Datadog evidence attached to the Effort.
- Complete an observation-driven repair and verify the outcome, then create two isolated variants and prove cleanup.
- Restart services and recover interrupted work without duplicate deployment.
- Run the fixed three-harness review panel, Greptile and required CI checks; fix all findings; merge, publish to Genesis, deploy and verify the installed revisions and browser flow.

No core part is deferred by calling the initial end-to-end proof a completed phase.
