# TemperPaw e295420 Live Observability Proof

Date: 2026-05-13

## Scope

This proof records the current live TemperPaw observability state after the
required WASM artifact repair and the Temper WASM host-boundary observability
work.

- TemperPaw branch: `codex/temperpaw-observability-live-image`
- TemperPaw commit: `e29542078559fb90fa4c46aa30d42fdf8630df7a`
- Temper branch: `codex/temperpaw-llmobs-service-identity-main`
- Temper commit: `18955ea724fc531deddd534e1319060ac59d8a59`
- Railway deployment: `d9869809-4bcd-4693-88f8-2d50923f3f25`
- Railway image digest:
  `sha256:6400baef44cb65885c7498e6a5ae4ed2f391a2a1bcaf5e2a528220cb369f9390`
- Railway build output digest:
  `sha256:99aea99a2fab065d892e997783da3dddb9f4686ab7bb5c1a4c65feb41d384252`

## Why This Proof Exists

Datadog caught a real production packaging issue during the final verification
pass. The `[Temper] Required WASM Load Failures` monitor alerted after target
directory pruning removed the only runtime-discoverable `blob_adapter` and
`workspace_fs` artifacts from the app bundle.

The fix was to make the paw-fs WASM build scripts publish the compiled artifacts
both to the app-level WASM directory and to the module-local paths the Temper
OS-app loader requires:

- `os-apps/paw-fs/wasm/blob_adapter/blob_adapter.wasm`
- `os-apps/paw-fs/wasm/workspace_fs/workspace_fs.wasm`

The repaired deployment now starts cleanly, registers both paw-fs modules, and
the Datadog monitor is back to OK.

## Red-Green Evidence

Red test:

```text
cargo test -p temperpaw --test temperpaw_identity_contract app_required_wasm_build_scripts_publish_module_local_artifacts -- --nocapture
```

The test failed before implementation because `blob_adapter/build.sh` did not
publish a module-local artifact.

Green tests after the repair:

```text
cargo test -p temperpaw --test temperpaw_identity_contract app_required_wasm_build_scripts_publish_module_local_artifacts -- --nocapture
cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture
cargo test -p temperpaw --test datadog_observability_contract -- --nocapture
cargo fmt --check
git diff --check
```

Results:

- `temperpaw_identity_contract`: 5 passed
- `datadog_observability_contract`: 23 passed
- formatting and whitespace gates passed

## Runtime Image Evidence

The deployed artifact was assembled from the repaired runtime rootfs and tagged
with the repaired commit identity:

- `BUILD_VERSION=sha-e295420`
- `BUILD_SHA=e29542078559fb90fa4c46aa30d42fdf8630df7a`
- `DD_VERSION=e29542078559fb90fa4c46aa30d42fdf8630df7a`

Local image checks before deploy showed:

- `/app/os-apps/paw-fs/wasm/blob_adapter/blob_adapter.wasm` exists
- `/app/os-apps/paw-fs/wasm/workspace_fs/workspace_fs.wasm` exists
- no nested `target` directories remained under `/app/os-apps`

Railway deployed the repaired artifact successfully. The live readiness endpoint
returned HTTP 200:

```text
https://openpaw-production.up.railway.app/readyz
```

The readiness response reported `status=ready` and Discord connected.

## Datadog Asset Reconciliation

The Datadog assets were reconciled from production credentials using the repo
scripts.

- Log pipeline: `TemperPaw / Temper Logs (ADR-0054)`
- Log pipeline id: `Wyq_6z_fTviM9uVH9MUIrQ`
- Dashboard id: `mn4-k3k-i66`
- Monitor set: updated with no orphan monitors
- Required WASM monitor id: `275384705`
- Required WASM monitor status after repair: OK

The Datadog facet API was unavailable for this account/tier during reconciliation,
so facets and SDS scanner rules remain UI-application proof items.

## Final Live Session

Final proof session:

- Session: `ss-019e213f-aac6-7981-91b9-1a9df81a9dc4`
- Result: `TemperPaw e295420 observability verified.`
- Status flow:
  `Created -> PreparingContext -> CallingProvider -> ApplyingProviderResponse -> Completed`
- APM trace id:
  `6b66255ce8c679c034ca302230625216`
- LLMObs/APM decimal trace id:
  `142757767638743301785701158388630704662`
- Datadog APM root: `Session.workflow`
- APM root duration: about 13.4s
- APM root version:
  `e29542078559fb90fa4c46aa30d42fdf8630df7a`
- APM child coverage: 494 hidden child spans

LLMObs tree:

```text
temperpaw.agent_session
  Session.ProviderAuthReady
    wasm:provider_caller
```

LLM span evidence:

- Provider: `openai`
- Model: `gpt-5.5`
- Input tokens: 213
- Output tokens: 14
- Total tokens: 227
- Duration: about 2.2s
- Status: OK

The Datadog `get_llmobs_agent_loop` helper returned an empty iteration timeline
for this direct Session trace even though the LLMObs trace tree is present and
correct. That is tracked as a known gap in the guide.

## Chronological Trace Shape

The final live APM trace gives a useful expanded workflow view instead of tiny
or unordered fragments. Resource aggregation for the session included:

- `Session.workflow`
- staged Session actions from provisioning through finalization
- `Session.ProviderAuthReady`
- `Session.ProviderResponseReady`
- `Session.FinalizeResult`
- `wasm.host.read_field`
- `dispatch_single_integration`
- `wasm:agent_reply`
- `emit_ots_trajectory`
- Postgres spans for `entity_field_index`, `entity_catalog`, and
  `wasm_invocation_logs`

The logs query:

```text
service:temperpaw @session_id:ss-019e213f-aac6-7981-91b9-1a9df81a9dc4
```

returned 36 correlated logs with the same session id, version, trace/span ids,
provider phase timings, WASM guest progress, and terminal OTS trajectory
emission.

## WASM Host Boundary Evidence

Temper commit `18955ea724fc531deddd534e1319060ac59d8a59` added host boundary
spans and SDK context fields so WASM work is visible at the host/guest boundary.
Verified host-boundary resources included:

- `wasm.host.get_secret`
- `wasm.host.cache_contains`
- `wasm.host.cache_from_stream`
- `wasm.host.hash_stream`
- `wasm.host.http_call_binary`
- `wasm.host.read_field`
- `wasm.invoke`
- `wasm_guest.progress`

Representative host-boundary trace:

- Trace: `ec3510f2ca0e2f854fcc3aa6580dfcde`
- Root HTTP operation: `PUT /odata/{path}`
- WASM invocation: `bootstrap-soul-file-probe`
- Entity type: `File`
- Trigger action: `StreamUpload`
- Observed host children: secret lookup, cache lookup, stream hashing, binary
  HTTP upload, and subsequent `File.StreamUpdated` reaction dispatch

This trace also exposed a remaining external storage identity gap: the R2 bucket
still contains the legacy `openpaw` name. It is explicitly tracked as an
allowlist item until the external resource is safely migrated.

## Postgres DBM Evidence

Datadog DBM is live for production Postgres:

- Database instance: `temperpaw-postgres`
- Service: `temperpaw`
- Source: `postgres`
- Team tag: `temperpaw`
- Calling service: `temperpaw`

The latest DBM sample captured before the e295 repair deployment showed:

- SQL statement family: `INSERT INTO snapshots`
- SQLCommenter trace propagation
- `trace.caller.service:temperpaw`
- `trace.caller.env:prod`
- `trace.mode:full`
- `trace.sampled:true`

The final e295 session has Postgres APM spans. DBM sampling did not select the
short post-repair burst by the time this proof was written, so the DBM sample is
recorded from the immediately preceding same-instrumentation production version.

## Profiling Evidence

On-demand profiling was verified on `e295420...` under read traffic.

- Request: `/_admin/profile/cpu?seconds=5&frequency=100`
- Concurrent read load: 84 `GET /tdata/Sessions?$top=5` requests
- Response: HTTP 200
- Returned profile size: 10,564 bytes
- Returned filename: `cpu-profile-5s.pb`

Datadog logs for the e295 version showed:

- `ADR-0055: starting CPU profile capture`
- `ADR-0055: CPU profile capture complete`
- `profile uploaded to Datadog Agent intake`

## Remaining Known Gaps

These are not hidden. They remain explicit in the human guide and should be
closed or accepted as permanent allowlist items before declaring every naming
item fully complete:

- Railway project/service/public Railway domain still carry the external legacy
  name.
- R2 bucket name still contains the external legacy storage identity.
- `temperpaw.katagami.ai` DNS was not resolving during verification.
- Datadog `get_llmobs_agent_loop` returns an empty helper timeline for the
  direct Session trace even though the LLMObs tree is present.
- ManagedSession semantic span-name export is not yet visible in the final live
  trace.
- Datadog facet and SDS scanner rule application require UI proof because the
  facet API was unavailable during script reconciliation.
