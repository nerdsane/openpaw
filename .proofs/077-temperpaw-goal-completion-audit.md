# TemperPaw Identity and Observability Completion Audit

Date: 2026-05-13

Status: Not complete yet. The core implementation is live and verified, and the
remaining items are now narrowed to explicit external allowlists and Datadog UI
or helper-surface gaps.

## Objective Restated As Deliverables

1. Active TemperPaw and Temper runtime identity uses TemperPaw, not the previous
   product name, with any remaining external resource names explicitly
   allowlisted.
2. TemperPaw production telemetry is complete enough for humans and agents:
   APM traces, metrics, logs, profiling, Postgres DBM/APM correlation, LLMObs,
   dashboards, monitors, pipelines, facets, sensitive-data scanning source
   rules, and operator/agent runbooks.
3. WASM is not opaque. Host-boundary spans and guest logs/progress events expose
   module/action/session/trace context for meaningful host functions.
4. A real production agent session proves the chronological trace shape:
   agent/session workflow, WASM integration, host function spans, external/db/LLM
   work, logs, and bottleneck timing.
5. Red-green tests, build/format gates, Datadog verification, proof artifacts,
   ADRs, and a human-facing guide exist.

## Prompt-To-Artifact Checklist

| Requirement | Evidence | Status |
| --- | --- | --- |
| Remove or allowlist active identity residue | `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture` passed 6 tests; active scan covers `.github`, `crates`, `dashboard`, Datadog assets, `docs`, `os-apps`, `scripts`, Docker, Railway, README, env example. | Covered |
| Explicit external allowlist | `docs/temperpaw-legacy-identity-allowlist.md` documents Railway project, Railway service, generated Railway domain, and R2 bucket with reasons and exit criteria; enforced by `legacy_external_resource_allowlist_documents_live_runtime_residue`. | Covered as allowlist |
| Datadog service identity | Live APM/log/DBM/LLMObs proof uses `service:temperpaw`, `team:temperpaw`, `ml_app:temperpaw`; active prior-identity service searches returned zero live service buckets except migration-query evidence. | Covered |
| Temper host WASM instrumentation | Temper commit `18955ea724fc531deddd534e1319060ac59d8a59` adds spans for `wasm.host.http_call`, `http_call_binary`, `http_stream`, `connect_call`, `get_secret`, `evaluate_spec`, cache/stream/read/hash functions, and guest progress/log correlation. | Covered |
| WASM SDK/runtime pin | `datadog_observability_contract` verifies TemperPaw server crates and all WASM SDK manifests/lockfiles pin the same Temper observability revision. | Covered |
| Guest logs/progress trace correlation | Live session logs for `ss-019e213f-aac6-7981-91b9-1a9df81a9dc4` returned 36 correlated logs with session id, version, span ids, provider phase timings, and WASM guest progress. | Covered |
| No misleading inside-WASM APM claim | Guide section `WASM Host Boundary Visibility` says Datadog does not see inside guest code automatically and documents host-boundary spans plus logs/progress. ADR-0086 records the rationale. | Covered |
| Production agent-session trace | Final session `ss-019e213f-aac6-7981-91b9-1a9df81a9dc4`; APM trace `6b66255ce8c679c034ca302230625216`; LLMObs/APM decimal trace `142757767638743301785701158388630704662`; result `TemperPaw e295420 observability verified.` | Covered |
| Chronological useful trace shape | APM root `Session.workflow`, duration 13.4s, 494 hidden child spans; resource aggregation includes Session stages, `wasm.host.read_field`, dispatch/integration spans, `wasm:agent_reply`, OTS trajectory emission, and Postgres spans. | Covered |
| LLMObs hierarchy | LLMObs tree `temperpaw.agent_session -> Session.ProviderAuthReady -> wasm:provider_caller`, provider `openai`, model `gpt-5.5`, 213 input tokens, 14 output tokens, status OK. | Covered |
| LLMObs agent-loop helper | `get_llmobs_agent_loop` for the same trace still returns `iterations: []` and `timeline: null`. | Not covered |
| Postgres DBM/APM | Current e295 DBM sample at `2026-05-13T12:26:58Z`, query signature `12941344394c8422`, `trace.caller.version:e295420...`, SQLCommenter traceparent `00-f16e96540c3d5762091448123a151a07-fb77aaa48d39dffb-01`, calling resource `GET /tdata/Sessions`. | Covered |
| DBM monitor correctness | Red test failed on `< 1`; fixed monitor query to `< 0.1`, reconciled live Datadog monitor `282522099`, verified status OK. | Covered |
| Profiling | On-demand e295 profile returned 10,564-byte `cpu-profile-5s.pb`; logs show capture start/complete/upload; metric shows one `profiles_uploaded` point for e295 cpu and no upload-error series. | Covered |
| Logs/pipelines/log metrics | Pipeline `TemperPaw / Temper Logs (ADR-0054)` id `Wyq_6z_fTviM9uVH9MUIrQ`; log metrics include `temperpaw.logs.errors`, `temperpaw.logs.warns`, `temperpaw.logs.wasm.default_timeout_fallback`. | Covered |
| Dashboards | Dashboard `mn4-k3k-i66` reconciled; `datadog_observability_contract` checks session, LLM, DBM, profiling, log/trace, transport, webhook, approval, sandbox, and TemperFS surfaces. | Covered |
| Monitors | Monitor reconciliation updated the full set; `tag:team:temperpaw status:alert` returned no active alerting monitors after the DBM threshold fix. | Covered |
| Facets | `dd-pipelines/facets.json` defines required facets and tests verify coverage. Datadog facet API returned unavailable/404 during reconciliation. | Source covered; UI proof missing |
| Sensitive Data Scanner | `dd-pipelines/sensitive-data-scanner.json` defines Datadog/OpenAI/GitHub/Slack redaction patterns and tests verify source coverage. Scanner group application requires Datadog UI context. | Source covered; UI proof missing |
| Human-facing guide | `docs/temperpaw-datadog-observability-guide.md` updated with current e295 deployment, trace IDs, DBM/profiling evidence, and query vocabulary. | Covered |
| Proof artifacts | `.proofs/076-temperpaw-e295420-live-observability-proof.md` and this audit record live evidence, queries, ids, tests, and gaps. | Covered |
| Material ADRs | Temper ADR-0086 documents WASM host-boundary observability. Existing TemperPaw ADRs cover Datadog/LLMObs/profiling/DBM deployment surfaces. | Covered |

## Commands And Gates Inspected

Already-run gates recorded in proof 076:

```text
cargo test -p temperpaw --test datadog_observability_contract -- --nocapture
cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture
cargo fmt --check
git diff --check
```

Additional red-green work in this audit:

```text
cargo test -p temperpaw --test datadog_observability_contract monitors_cover_session_trace_llmobs_and_postgres_dbm_health -- --nocapture
```

This failed on the previous DBM activity threshold, then passed after the
monitor source of truth was changed to `< 0.1`.

```text
cargo test -p temperpaw --test temperpaw_identity_contract legacy_external_resource_allowlist_documents_live_runtime_residue -- --nocapture
```

This failed before the explicit external-resource allowlist existed, then passed
after `docs/temperpaw-legacy-identity-allowlist.md` was added.

## Remaining Missing Or Weak Items

- Datadog `get_llmobs_agent_loop` still returns an empty helper timeline for
  the direct Session trace even though the LLMObs span tree is correct.
- Datadog APM does not yet expose the hinted `temperpaw.agent.session` semantic
  span name from ManagedSession bridge headers in the current final trace. Direct
  Session roots are live as `Session.workflow`.
- Log facet registration and Sensitive Data Scanner application still need
  Datadog UI proof or a working API path for this account.
- `temperpaw.katagami.ai` DNS was not resolving during live verification.
- External Railway/R2 names are explicitly allowlisted, not migrated. That is
  acceptable only under the "remove or allowlist" criterion; a zero-legacy-name
  target still needs a planned external-resource cutover.

## Audit Decision

Do not mark the active goal complete yet. Core observability is live and useful,
the e295 DBM/profiling/monitor gaps found during audit were closed, and external
legacy names are now explicitly allowlisted. The remaining Datadog helper/UI
proof and custom-domain items still need either closure or an explicit user
decision that the documented limitations are acceptable for final completion.
