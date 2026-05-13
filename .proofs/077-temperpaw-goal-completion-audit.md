# TemperPaw Identity And Observability Completion Audit

Date: 2026-05-13

Status: complete for the current working TemperPaw production system, with the
remaining external/resource and Datadog helper limitations explicitly
documented.

## Objective Restated As Deliverables

1. Runtime and active source identity use TemperPaw. Remaining external resource
   names from the previous product identity are explicitly allowlisted with
   migration exit criteria.
2. Production telemetry is complete enough for humans and agents: APM traces,
   logs, metrics, profiling, Postgres DBM/APM correlation, LLMObs, dashboards,
   monitors, log pipeline, log metrics, source-controlled facets, source
   Sensitive Data Scanner rules, and operator/agent runbooks.
3. WASM is not opaque. Temper host-boundary spans and correlated guest
   logs/progress expose the module, action, session, trace, span, workflow step,
   and host function involved.
4. A real production agent session proves the chronological trace shape:
   session workflow, WASM integrations, host function spans, database work, LLM
   work, logs, and bottleneck timing under one usable tree.
5. Red-green tests, builds, runtime deployment, live Datadog verification, proof
   artifacts, ADRs, and a human-facing guide exist.

## Current Runtime Evidence

| Surface | Evidence | Status |
| --- | --- | --- |
| TemperPaw runtime | Version `86bd073dc89efc6e559cbdf9787ce9e0b92228fe`, runtime label `sha-86bd073`, `/paw/version` returned that SHA. | Covered |
| Temper dependency | Live production proof used Temper `64824d640a915272e21a307029030439a41fdde5`; PR-ready source now pins the merged mainline equivalent `d4797f0bc9e22cf8cc075e18e5a00926a391faf1`, which includes LLMObs root stability, published-artifact telemetry enrichment, WASM observability, DBM propagation, profiling upload envelopes, and long-lived session roots. | Covered |
| Railway deploy | Deployment `598c9ca9-f026-40c0-9b95-f086d82fe846` served HTTP 200 on `/readyz`. | Covered |
| Image provenance | GitHub Actions run `25811095072` built `ghcr.io/nerdsane/temperpaw:sha-86bd073`; GHCR digest `sha256:9859786cdbdbc72c76417e94531497b16c04df4af0b4a115a0def7a58d604e3c`; Railway build log pulled that exact tag/digest. | Covered |
| Deploy drift prevention | `Dockerfile.deploy` now uses `ARG IMAGE_TAG` and `FROM ghcr.io/nerdsane/temperpaw:${IMAGE_TAG}`; `railway.toml` uses `builder = "DOCKERFILE"` and `dockerfilePath = "Dockerfile.deploy"`. | Covered |
| Active identity scan | `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture` passed 7 tests, including active surface scanning and the new Railway image-tag contract. | Covered |
| External allowlist | `docs/temperpaw-legacy-identity-allowlist.md` documents the Railway project, Railway service, generated Railway domain, and R2 bucket as external resources with exit criteria. | Covered as allowlist |

## Production Agent Session Proof

Final session:

- Session id: `ss-019e2239-fe6f-7810-b717-d842442bfce1`
- Action: `TemperPaw.Configure`
- Provider/model: `openai_codex`, `gpt-5.5`
- Prompt: `Reply exactly: TemperPaw 86bd073 final Datadog observability verified.`
- Result: `TemperPaw 86bd073 final Datadog observability verified.`
- State flow:
  `Created -> Provisioning -> PreparingContext -> EnsuringProviderAuth -> CallingProvider -> ApplyingProviderResponse -> Steering -> Completed`

APM evidence:

- Trace id: `3582885463604920100`
- Root resource: `Session.workflow`
- Root span id: `12367668208079438116`
- Root duration: `21.523s`
- Raw span count observed: `524`
- Service/version: `temperpaw`,
  `86bd073dc89efc6e559cbdf9787ce9e0b92228fe`
- Important resources observed: `Session.Configure`,
  `Session.WorkspaceReady.integrations`, `wasm:workspace_provisioner`,
  `Session.ContextReady.integrations`, `Session.ProviderAuthReady`,
  `wasm.host.read_field`, `Session.CheckSteering.integrations`,
  `wasm:agent_reply`, `emit_ots_trajectory`, and Postgres spans.
- Datadog trace link:
  `https://app.datadoghq.com/apm/trace/3582885463604920100?graphType=flamegraph&shouldShowLegend=true&spanID=12367668208079438116&timeHint=1778690686000.0000&trace=358288546360492010012367668208079438116&traceQuery=`

Logs evidence:

- Query `service:temperpaw "ss-019e2239-fe6f-7810-b717-d842442bfce1"`
  returned correlated logs for the same version.
- Logs included steering/finalization and OTS trajectory emission:
  `trj-ss-019e2239-fe6f-7810-b717-d842442bfce1`.
- Correlated log trace id: `3582885463604920100`.

LLMObs evidence:

- LLMObs trace id: `123527112440865216744564245077429649188`
- Span tree:

```text
agent temperpaw.agent.session
  workflow Session.ProviderAuthReady
    llm wasm:provider_caller
```

- Total spans: 3
- Tree depth: 3
- Error count: 0
- LLM span id: `5439141549405964232`
- Duration: `2.333s`
- Provider/model: `openai`, `gpt-5.5`
- Tokens: 1114 input, 19 output, 1133 total

Known LLMObs helper limitation: `get_llmobs_agent_loop` returned an empty
timeline for this direct Session trace. `get_llmobs_trace`, APM, logs, and
Temper entity history are the authoritative chronology.

## WASM Observability Evidence

Temper host-side instrumentation covers the meaningful WASM boundaries used by
TemperPaw:

- `wasm.host.http_call`
- `wasm.host.http_call_binary`
- `wasm.host.http_stream`
- `wasm.host.connect_call`
- `wasm.host.get_secret`
- `wasm.host.evaluate_spec`
- `wasm.host.cache_contains`
- `wasm.host.cache_to_stream`
- `wasm.host.cache_from_stream`
- `wasm.host.read_field`
- `wasm.host.hash_stream`

The final session trace contained WASM integration spans and host-function spans
under `Session.workflow`, not orphaned telemetry. Guest logs and progress events
carry Datadog-readable `trace_id`, `span_id`, `session_id`, `entity_id`,
`action_name`, `wasm_module`, and `workflow_step` fields.

ADR-0086 records the current design: these are host-boundary spans and
correlated guest logs/progress, not a misleading claim that Datadog sees inside
guest code automatically. Explicit guest-created spans remain a future
host-API extension.

## Published Artifact Proof

Final publication:

- Route: `POST /api/files/publish-artifact`
- Deployment owner: `598c9ca9-f026-40c0-9b95-f086d82fe846`
- Source file: `bootstrap-soul-file-paw`
- Source content hash:
  `sha256:a7b843737b4e8d4eaab95a060898b7abbaad53b4b618dcbe2c18b14e5a7eeaa9`
- Response: HTTP 200 in `631.753ms`
- Artifact id: `part-33863cb7a1bc3906a4819ac56ddcfcc5`
- Public URL:
  `https://temperpaw-assets.katagami.ai/codex-live-proof/CodexProof/598c9ca9-f026-40c0-9b95-f086d82fe846/codex-live-publish-86bd073-rich-telemetry-v2-a7b843737b4e8d4eaab95a060898b7abbaad53b4b618dcbe2c18b14e5a7eeaa9.md`
- Public read: HTTP 200, content length `18568`
- Downloaded object hash matched the source hash.

APM trace:

- Trace id: `d1f2b8c57fcf4858fd5bea0aeb5bbdf6`
- Shape:
  `http.server.request POST /api/files/publish-artifact ->
  POST /api/files/publish-artifact -> state.publish_file_artifact`
- Child/sibling spans:
  `state.read_file_stream_indexed`, `state.put_public_blob`,
  `postgres.upsert_published_artifact`, and SQL children for
  `published_artifacts`.
- Datadog trace link:
  `https://app.datadoghq.com/apm/trace/d1f2b8c57fcf4858fd5bea0aeb5bbdf6?graphType=flamegraph&shouldShowLegend=true&spanID=6934239732066560871&timeHint=1778690633928.0625&trace=d1f2b8c57fcf4858fd5bea0aeb5bbdf66934239732066560871&traceQuery=`

Logs confirmed:

- `public blob PUT succeeded`
- `published artifact metadata persisted`
- `publish artifact request completed`

This closes the previous weakness where a successful publish route did not
teach operators enough about blob write, metadata persistence, and public URL
readability.

## Postgres DBM Proof

Final DBM sample:

- Timestamp: `2026-05-13T16:44:53.801Z`
- Database instance: `temperpaw-postgres`
- Database: `railway`
- Table: `entity_field_index`
- Query signature: `94651ed8bdbcaeb0`
- Statement class:
  `INSERT INTO entity_field_index ... ON CONFLICT ... DO UPDATE`
- APM trace id: `3582885463604920100`
- APM span id: `357018841880160397`
- Trace mode: `full`
- Calling service: `temperpaw`
- Calling resource: `Session.workflow`
- Calling version: `86bd073dc89efc6e559cbdf9787ce9e0b92228fe`
- SQLCommenter traceparent:
  `00-5cee74dbd4bd9d9631b8f758a114ff24-04f462bec063688d-01`

This proves DBM activity, SQLCommenter propagation, service/version tagging,
and APM calling-service correlation for the final production session.

## Profiling Proof

Final on-demand CPU profile:

- Endpoint: authenticated `/_admin/profile/cpu?seconds=5&frequency=100`
- HTTP status: 200
- Content type: `application/vnd.google.protobuf`
- Downloaded profile size: `83` bytes
- Hash:
  `sha256:a702af1125e50891c7ab96e35073489fd38a25e6eb6191641711a55747db8e49`
- Runtime logs:
  - `2026-05-13T16:51:45.768Z` capture started
  - `2026-05-13T16:51:50.859Z` capture complete, `seconds=5`,
    `frequency=100`, `bytes=83`
  - `2026-05-13T16:51:51.062Z` profile uploaded to Datadog Agent intake
- Metric:
  `sum:datadog.profiling.rust.profiles_uploaded{service:temperpaw,env:prod,version:86bd073dc89efc6e559cbdf9787ce9e0b92228fe}.as_count()`
  returned one point at `2026-05-13T16:52:00Z`.
- Matching upload-error series returned no data in the final check.

## Datadog Assets

| Asset | Evidence | Status |
| --- | --- | --- |
| Dashboard | Dashboard `mn4-k3k-i66`, `TemperPaw - Platform Overview`, exists with session, LLM, DBM, profiling, logs, trace, transport, webhook, approval, sandbox, and TemperFS surfaces. | Covered |
| Monitors | `search_datadog_monitors(query:"tag:team:temperpaw")` returned the TemperPaw monitor set. `search_datadog_monitors(query:"tag:team:temperpaw status:alert")` returned no active alerting monitors during final verification. | Covered |
| Log pipeline | Pipeline `TemperPaw / Temper Logs (ADR-0054)`, id `Wyq_6z_fTviM9uVH9MUIrQ`, applied. | Covered |
| Log metrics | `temperpaw.logs.errors`, `temperpaw.logs.warns`, and `temperpaw.logs.wasm.default_timeout_fallback` are source-controlled and verified. | Covered |
| Facets | `dd-pipelines/facets.json` defines session, trace, LLM, WASM, transport, webhook, approval, sandbox, and TemperFS diagnostic facets. | Source covered; UI proof limited |
| Sensitive Data Scanner | `dd-pipelines/sensitive-data-scanner.json` defines Datadog, LLM, source-control, chat, email, and cloud-token redaction patterns. | Source covered; UI proof limited |

## Commands And Gates

Temper work already completed and pushed on branch
`codex/temperpaw-llmobs-service-identity-main`:

```text
cargo test -p temper-server published_artifact_success_log_carries_publication_observability_fields -- --nocapture
cargo test -p temper-server --test published_artifacts -- --nocapture
cargo fmt --check
git diff --check
bash scripts/readability-ratchet.sh check
cargo clippy -p temper-server --all-targets --features observe -- -D warnings
full pre-push gates
```

Temper commits:

```text
64824d640a915272e21a307029030439a41fdde5 fix(observe): enrich published artifact telemetry
d4797f0bc9e22cf8cc075e18e5a00926a391faf1 Add TemperPaw runtime observability foundations
```

TemperPaw gates refreshed after the final guide update:

```text
cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture
# 7 passed

cargo test -p temperpaw --test datadog_observability_contract -- --nocapture
# 23 passed
```

Final local hygiene gates after this proof update:

```text
cargo fmt --check
# passed

git diff --check
# passed

rg "64824d640a915272e21a307029030439a41fdde5|488c521|18955ea|e295420|d9869809|sha-afeca|9609583" Dockerfile Cargo.lock crates/temperpaw/Cargo.toml os-apps -S --no-ignore -g '!**/target/**'
# no active runtime/config hits
```

## Known Limitations And Follow-Ups

These are not hidden blockers to the current system observability, but they are
the next cleanup items:

- External Railway and object-storage resource names remain on the explicit
  allowlist until a planned resource cutover.
- Datadog's `get_llmobs_agent_loop` helper returned an empty direct-session
  timeline. The LLMObs tree, APM trace, logs, and Temper entity history are
  still complete enough to debug the session chronologically.
- Log facet registration and Sensitive Data Scanner application require
  Datadog UI/account context in this org; the source definitions and automated
  contract coverage exist.
- The plain `openai` provider path failed because a runtime secret template was
  unresolved. The final proof used the working `openai_codex` provider. Treat
  the plain provider path as a configuration follow-up.

## Audit Decision

The goal's success criteria are satisfied for the currently working TemperPaw
production system:

- active runtime identity is TemperPaw, with explicit external allowlists;
- meaningful WASM execution is visible through host-boundary spans and
  correlated guest logs/progress;
- a real agent session has coherent APM, logs, DBM, LLMObs, and profiling
  evidence;
- artifact publishing has route, state, blob, Postgres, log, and public-read
  proof;
- humans and agents have a guide and query vocabulary for operating the system.

The final local hygiene checks passed. Marking the active goal complete is
appropriate once this proof is committed and pushed with the code/docs changes.
