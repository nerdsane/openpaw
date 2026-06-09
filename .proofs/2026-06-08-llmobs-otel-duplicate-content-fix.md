# LLMObs OTel Duplicate Content Fix Proof

Date: 2026-06-08
Branch: codex/disable-otel-llmobs-duplicates

## Issue

Datadog LLMObs still showed rows that looked like `No content` after the
direct LLMObs content exporter was fixed. Export API investigation showed two
LLMObs sources for the same production traffic:

- Temper direct LLMObs API rows with input and output content.
- Datadog automatic OTel-to-LLMObs conversion rows created from APM GenAI spans.

The automatic conversion path was the duplicate source that could render empty
input/output content in the LLMObs UI.

## Fix

- Keep APM OTLP export to the Railway Datadog Runtime Agent.
- Keep direct LLMObs API export enabled with `DD_LLMOBS_API_ENABLED=true`.
- Add `dd_llmobs_enabled=false` to `OTEL_RESOURCE_ATTRIBUTES` so Datadog keeps
  GenAI OTel spans in APM and does not auto-convert them into second LLMObs
  rows.
- Preserve service identity in `OTEL_RESOURCE_ATTRIBUTES`:
  `service.name=temperpaw`, `service.version=<build version>`, and
  `deployment.environment=prod`.
- Apply the same boundary in the deploy CLI, governed Railway setup API, and
  governed Railway redeploy endpoint.
- Apply the same boundary in the manual GitHub `railway-redeploy` workflow.
- ADR: `docs/adrs/0064-llmobs-direct-api-export-boundary.md`.

## Red-Green Evidence

- Red: expanded
  `setup_api_can_ensure_railway_datadog_runtime_agent_without_exposing_tokens`
  to require `OTEL_RESOURCE_ATTRIBUTES`; it failed before implementation.
- Green: `cargo test -p temperpaw --test datadog_observability_contract`
  passed 32 tests.
- Green: `cargo test -p temperpaw-cli deploy::tests` passed 21 tests.
- Green:
  `cargo test -p temperpaw --test temperpaw_identity_contract railway_redeploy_uses_current_deployment_api`
  passed.
- Red: expanded
  `manual_railway_redeploy_workflow_is_secret_backed_and_version_proven` to
  require the OTEL resource boundary; it failed before the workflow upsert was
  added.
- Green:
  `cargo test -p temperpaw --test temperpaw_identity_contract manual_railway_redeploy_workflow_is_secret_backed_and_version_proven`
  passed.
- Green:
  `cargo test -p temperpaw datadog_enhanced_app_vars_disable_otel_llmobs_auto_conversion`
  passed.
- Green: `cargo build -p temperpaw --release --bin temperpaw-server` passed.

## Production Hot Fix

Production Railway service: `openpaw`.

Set and confirmed:

```text
BUILD_SHA=3aebc0a09a2c578708354cd840fda3847c895931
BUILD_VERSION=sha-3aebc0a
DD_VERSION=sha-3aebc0a
OTEL_RESOURCE_ATTRIBUTES=service.name=temperpaw,service.version=sha-3aebc0a,deployment.environment=prod,dd_llmobs_enabled=false
```

Restarted Railway deployment:

```text
84ab260d-ba7d-4f35-bc73-9b1cd1b15816
```

`/healthz` responded after restart. `/paw/version` returned 401 from the public
URL, so runtime version was verified from Railway non-secret env instead.

## Datadog Verification

Queried Datadog LLMObs export API for the latest 200 production
`span_kind=llm`, `ml_app=temperpaw` rows in the post-restart window using
top-level `input` and `output` fields:

```json
{
  "total": 200,
  "empty_both": 0,
  "empty_output": 0,
  "undefined_parent_empty_both": 0,
  "by_name": [
    {
      "name": "wasm:provider_caller",
      "count": 200,
      "empty_both": 0,
      "empty_output": 0
    }
  ]
}
```

The newest sampled rows each had `input_count=3`, `output_count=3`, and
non-null metadata. Older LLMObs rows are not rewritten by Datadog; verification
must use a post-deploy time window.
