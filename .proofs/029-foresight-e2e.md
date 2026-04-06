# Proof 029: Foresight E2E — Deep Sci-Fi

**Date:** 2026-04-04T03:21:52+00:00
**Target:** https://github.com/arni-labs/deep-sci-fi.git

## Results

- ProductModel: 019d5681-0bae-7d21-85b7-ac09e56a0885 (Active, 19673 chars knowledge graph)
- Projection: 019d5681-fb3b-7591-8264-c1d8860f0f01
- Observations: 10 with content
- Directions: 4 with reasoning

## Observations

### [high] Observability infrastructure is in active flux with multiple competing approaches to Datadog integra

Observability infrastructure is in active flux with multiple competing approaches to Datadog integration. In the last 5 days (commits 8c4fde1 back through 63b58d6), the team cycled through 4 different trace export strategies: direct OTLP/HTTP → ddtrace-run → OTLP/HTTP again → duck-typed TracerProvider. This pattern indicates either unclear requirements, incomplete testing at integration boundaries, or unresolved compatibility issues between Logfire (global TracerProvider wrapper) and Datadog OTLP exporter attachment. PR #96 is still open, suggesting the current approach may not be final.

**Signals:** ["commit:8c4fde1", "commit:8ef1d84", "commit:87a7dd5", "commit:80dd26b", "commit:63b58d6", "commit:cf4db14", "commit:c725a02", "pr:96", "pr:95", "pr:94", "pr:93", "pr:92", "pr:91", "pr:90"]

**If ignored:** If this churn is not addressed, the system will ship with fragile observability that breaks silently when Logfire versions update or Datadog API endpoints change. Team will lose 3-5 hours per incident debugging span export failures that have no visible impact on app behavior (traces simply don't arrive). This is especially dangerous in a system managing 'deep sci-fi' agents where audit trails and interaction logs are critical.

### [critical] All 18 deep-sci-fi backend monitors show 'No Data' state despite being configured (monitor IDs 27132

All 18 deep-sci-fi backend monitors show 'No Data' state despite being configured (monitor IDs 271327268-290). This indicates either: (1) the service is not running in production yet, (2) instrumentation is wired but not emitting metrics, or (3) metric names/tags don't match query expectations. The only 'OK' monitors are generic system checks (NTP sync, active entities). This is a blind spot — the system has measurement infrastructure but no visibility into whether it's working.

**Signals:** ["monitor:271327268", "monitor:271327270", "monitor:271327271", "monitor:271327272", "monitor:271327274", "monitor:271327275", "monitor:271327277", "monitor:271327280", "monitor:271327281", "monitor:271327282", "monitor:271327284", "monitor:271327289", "monitor:271327290", "commit:919257b"]

**If ignored:** If monitors remain 'No Data' at deployment time, the system will go into production completely blind. P99 latency could spike to 10s, error rates could climb to 50%, and database connection pools could exhaust — and the team won't know until users complain. This directly undermines the stated goal of observability.

### [high] Recent middleware additions (ResponseTimeMiddleware in 040c1d5, RequestIDMiddleware in e9aa03c) show

Recent middleware additions (ResponseTimeMiddleware in 040c1d5, RequestIDMiddleware in e9aa03c) show the team is building observability infrastructure systematically at the FastAPI layer. However, these are application-level signals, not integration tests. There's no evidence (commit message, test name, or PR description) that these middleware actually integrate with the Datadog exporter to produce queryable spans. The ResponseTime and RequestID data exists in logs, but may not be flowing to Datadog metrics.

**Signals:** ["commit:040c1d5", "commit:e9aa03c", "commit:919257b"]

**If ignored:** If middleware is logging to stdout only and not connecting to the Datadog span pipeline, the monitors will continue to show 'No Data'. The system will appear to have observability (code is there, tests pass) but metrics queries will find nothing. This creates a false sense of safety.

### [medium] The codebase shows substantial recent activity in the observability domain but virtually no commits 

The codebase shows substantial recent activity in the observability domain but virtually no commits to core business logic (world/dweller/story endpoints) in the last 7 days. The 20-commit window is dominated by observability refactoring. This suggests either: (1) the team is deliberately hardening instrumentation before scaling, or (2) feature development is blocked waiting for observability to stabilize. Either way, the system is not moving forward on core functionality while burning velocity on infrastructure debugging.

**Signals:** ["commit:8c4fde1", "commit:040c1d5", "commit:e9aa03c", "commit:919257b"]

**If ignored:** If observability churn continues without resolution, feature delivery will slow further. The team will be trapped in an observability-first loop where every deploy requires instrumentation validation before any feature work can proceed.

### [high] The ProductModel shows 25 open and closed PRs, with a clustering around observability fixes (PRs #90

The ProductModel shows 25 open and closed PRs, with a clustering around observability fixes (PRs #90-96) but no merged PRs for feature work since #79 (media/illustration style, 6 days ago). The gap between 'committed observability strategy' (PRs 82, 83) and 'working observability' (open PR 96) is still unresolved. This is not a missing feature — it's a blocking issue in the critical path.

**Signals:** ["pr:96", "pr:95", "pr:94", "pr:93", "pr:92", "pr:91", "pr:90", "pr:82", "pr:83"]

**If ignored:** If PR #96 is not merged and validated, the next deployment will ship with unknown observability state. The team will be unable to diagnose production issues because they won't know if the problem is in the app, the exporter, or the Datadog integration. Incident response will be severely hampered.

### [high] Observability instrumentation thrashing: 6 commits and 5 merged PRs in 72 hours (Apr 1-3) chasing Da

Observability instrumentation thrashing: 6 commits and 5 merged PRs in 72 hours (Apr 1-3) chasing Datadog OTLP endpoint configuration. The team cycled through three different approaches: (1) api.datadoghq.com/api/intake/otlp/v1/traces (404), (2) ddtrace-run with trace.agent.datadoghq.com (PUT/POST mismatch), (3) otlp.datadoghq.com/v1/traces (202 success). Each approach required test rewrites. PR #96 is still open, suggesting the solution is incomplete. The rapid iteration pattern indicates the team is learning the endpoint contract in production time rather than via documentation or pre-flight validation.

**Signals:** ["commit:7701667", "commit:63b58d6", "commit:00eec82", "commit:c725a02", "commit:8b566ab", "commit:8ef1d84", "commit:80dd26b", "pr:96", "pr:95", "pr:94", "pr:93", "pr:92", "pr:91", "pr:90"]

**If ignored:** If this pattern continues, the team will spend more cycles on observability infrastructure than on feature work. Each failed endpoint choice creates technical debt (test rewrites, TracerProvider wrapping logic). The open PR #96 suggests the current solution may still not be correct, risking production blind spots when traces fail silently due to endpoint misconfiguration.

### [critical] Monitoring blind spot: 25 of 28 Datadog monitors are in 'No Data' state (all deep-sci-fi backend mon

Monitoring blind spot: 25 of 28 Datadog monitors are in 'No Data' state (all deep-sci-fi backend monitors + OpenPaw monitors). Only 3 monitors report OK state (NTP sync, Active Entities, AlertCycle health). The backend service has comprehensive monitor coverage (error rates, latency percentiles, DB query latency, connection errors, CPU/memory, /worlds endpoint, pgvector queries, dweller interactions, request volume anomalies) — but none are receiving data. This indicates the service is not yet emitting metrics to Datadog, or the instrumentation is not working despite the recent fixes.

**Signals:** ["monitor:271327268", "monitor:271327270", "monitor:271327271", "monitor:271327272", "monitor:271327274", "monitor:271327275", "monitor:271327277", "monitor:271327280", "monitor:271327281", "monitor:271327282", "monitor:271327284", "monitor:271327289", "monitor:271327290", "monitor:270433464", "monitor:270433469", "monitor:270433472", "monitor:270433477", "monitor:270470278", "monitor:270470281", "monitor:270470295"]

**If ignored:** Without metric emission verified, the backend is flying blind. Error spikes, latency degradation, database stalls, and resource exhaustion will not trigger alerts. The team will discover incidents post-facto or through user reports. This is especially dangerous for the /worlds endpoint, dweller interactions, and pgvector queries — all core to the deep-sci-fi product — which have dedicated monitors that are currently dark.

### [high] Instrumentation complexity explosion: The observability stack now includes Logfire (TracerProvider),

Instrumentation complexity explosion: The observability stack now includes Logfire (TracerProvider), Datadog OTLP dual-shipping (gRPC + HTTP fallback), X-Request-ID middleware, response time logging, and auto-instrumentation of FastAPI/SQLAlchemy/httpx. The recent commits show 'duck typing' checks for TracerProvider (commit:8ef1d84), custom TracerProvider creation when Logfire wraps the global provider (commit:80dd26b), and conditional exporter attachment based on DD_API_KEY presence. This complexity is fragile: if Logfire changes its ProxyTracerProvider interface, or if Datadog changes endpoint contracts again, the hand-rolled compatibility layer breaks. Tests exist but are tightly coupled to implementation details.

**Signals:** ["commit:8ef1d84", "commit:80dd26b", "commit:919257b", "commit:60c48cb", "pr:95", "pr:94"]

**If ignored:** Each new observability requirement (new exporter, provider version, middleware) will require revisiting the instrumentation layer. The hand-rolled duck typing and conditional provider creation are technical debt. If the team needs to add another observability backend (e.g., Honeycomb, New Relic) or upgrade Logfire, the code will become unmaintainable. Lack of a clear contract between Logfire and Datadog exporters means subtle bugs (like the endpoint misconfigurations) will recur.

### [medium] Recent commits show healthy middleware + health check work (PR #89, #88, #84), but these are decoupl

Recent commits show healthy middleware + health check work (PR #89, #88, #84), but these are decoupled from the observability thrashing. ResponseTimeMiddleware and RequestIDMiddleware are pure ASGI, well-tested, and ship 2 days before the endpoint chaos began (Apr 1 vs Apr 3). The /health endpoint with db/alembic/uptime probing (PR #84) is solid. However, these foundational pieces are now shadowed by the Datadog integration crisis. The middleware log the data, but observability backend is dark — so the logs exist but are not routed to Datadog for alerting.

**Signals:** ["commit:e9aa03c", "commit:040c1d5", "pr:89", "pr:88", "pr:84"]

**If ignored:** The middleware work is not wasted, but it's not fully leveraged. If Datadog observability never stabilizes, the rich request-level telemetry (request ID, response time, endpoint) remains application-local only. The health check endpoint can be polled manually, but there's no continuous monitoring dashboard. Operators will lack visibility into request patterns, latency trends, and error modes.

### [medium] Test coverage for observability is growing but remains implementation-specific. PR #93 rewrites test

Test coverage for observability is growing but remains implementation-specific. PR #93 rewrites tests to verify OTLP exporter attachment, endpoint URL, and idempotency. PR #91 tests ddtrace module mocking. Tests pass, but they validate *that* the code does what it says, not *whether* the endpoint actually receives traces. There's no integration test or e2e validation that a real trace roundtrips from the backend through Datadog and lands in the monitor system. This is why the endpoint misconfigurations (404, PUT vs POST) were only discovered by manual retry cycles, not by tests.

**Signals:** ["commit:8b566ab", "commit:17722fa", "pr:93", "pr:91"]

**If ignored:** Without end-to-end observability validation, the team will continue discovering endpoint and configuration errors at deploy time. The test suite will give false confidence that observability is working, when in fact traces are being silently dropped or rejected.

## Directions

### Halt observability churn; validate the current approach end-to-end before shipping

The rapid iteration on trace export strategies (4 approaches in 5 days) suggests the team is solving the wrong problem. Instead of merging PR #96 and moving on, pause and run an integration test: deploy the current code to a staging environment that mirrors production (with real Datadog API key), emit a test trace, and verify it arrives in Datadog with correct tags and spans. Do not merge another observability PR until this validation passes. This is not a feature request — it's a quality gate. The team has built middleware and monitors, but hasn't proven they talk to each other.

**If not taken:** If this validation is skipped, the system will deploy with observability that is either silent (traces lost) or broken (exporter crashes at runtime). This will be discovered in production, not in staging. Given the team's recent history of OTLP endpoint failures and TracerProvider compatibility issues, this is a high-probability scenario.

### Make monitor 'No Data' state a deployment blocker; do not ship with blind monitoring

All 18 deep-sci-fi monitors are in 'No Data' state. This is unacceptable for a production service. Before any deployment, the team must ensure at least the top-4 critical monitors (error rate, P95 latency, DB errors, service uptime) show either 'OK' or 'Alert' — never 'No Data'. 'No Data' means the system has no insight into whether it's working. This is a critical gap. Create a checklist: (1) confirm metric names match what FastAPI/SQLAlchemy is emitting, (2) verify Datadog API key is set and exporter is reaching otlp.datadoghq.com, (3) run a synthetic request through the /worlds endpoint and check that the trace appears in Datadog UI within 10 seconds. This should be a pre-deployment ritual, not an afterthought.

**If not taken:** If monitoring remains blind at deployment, the team will have no way to detect or diagnose the observability churn they've just spent a week on. Every incident will require manual log inspection. The monitors are configured but useless, creating false confidence that the system is observable when it isn't.

### Freeze observability changes and validate metric ingestion end-to-end before merging PR #96

The ProductModel shows 5 merged PRs in 72 hours, each rewriting observability code and tests, with 1 PR still open (#96). This is a sign of incomplete problem-solving. The 25 monitors in 'No Data' state confirm that metric/trace emission is not working. The team is iterating on configuration (endpoint URLs, provider wrapping logic) without validating that the signal actually reaches Datadog. PR #96 is titled 'OTLP base URL only (exporter auto-appends /v1/traces)' — suggesting another endpoint-related tweak. Before merging, the team should (1) verify with a simple test span that otlp.datadoghq.com/v1/traces accepts POST with the current exporter configuration, (2) wait for at least one monitor to report data (not 'No Data'), (3) confirm that a trace launched from the backend appears in the Datadog UI within 5 minutes. The observability complexity (Logfire + OTLP dual-shipping + duck typing) should remain frozen while validation is in flight. If validation fails, roll back to a simpler approach (single-shipper OTLP or ddtrace, not both).

**If not taken:** If this direction is ignored, the team will continue merging observability PRs without verification. The monitors will remain in 'No Data' state, operators will have no visibility, and the next incident will be discovered post-facto. The hand-rolled compatibility layer will accumulate more special cases (duck typing, conditional provider creation, fallback logic), making the code harder to debug and harder to migrate away from.

### Decouple observability backend choice from application code; move Datadog configuration to environment-only contract

The ProductModel shows scattered observability logic: configure_datadog_otlp() in observability.py with conditional imports, hand-rolled TracerProvider creation, duck typing checks for ProxyTracerProvider, and gRPC/HTTP fallback logic all living in the application startup path. Every change to the Datadog endpoint or exporter type requires code changes, test rewrites, and validation cycles. Instead, move the observability backend configuration entirely to environment variables and a thin adapter layer. The contract should be: (1) if DD_API_KEY is set, attach a Datadog exporter to the global TracerProvider (whatever it is); (2) the exporter endpoint and auth come from env, not code; (3) the adapter does not assume a specific TracerProvider type — it uses only the standard OpenTelemetry interfaces (add_span_processor, etc.); (4) if attachment fails (wrong provider type, missing dependency), log and continue, do not crash. This moves the brittleness (duck typing, version-specific compatibility) from application code to deployment-time validation. The team can then iterate on Datadog configuration (endpoint URLs, exporter types) without touching Python code, reducing the cycle time from 72 hours of thrashing to a single env var change.

**If not taken:** Without this decoupling, the team will continue to discover observability endpoint and exporter contract issues in the application startup sequence. Each fix requires a code change, test rewrite, and re-deploy. As the observability stack grows (more exporters, more backends, more providers), the application startup code will become a compatibility layer for the entire observability ecosystem. This is unmaintainable and will eventually push the team toward removing observability entirely rather than fixing it.

