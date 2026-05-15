# Railway Datadog Product Coverage Live Proof

Date: 2026-05-15

Status: live proof complete. APM, Error Tracking, logs correlation, LLMObs, and
on-demand profiling are supported and verified on Railway. Continuous profiling
and USM are not misconfigured in TemperPaw; they are blocked by Railway host
capability boundaries.

## Objective

Implement the Railway-locked Datadog observability plan from a branch based on
`main`, keep production on Railway, keep application instrumentation
OpenTelemetry-native, make Datadog-specific supplements only where Railway can
support them, prove each Datadog product live, open PRs into `main`, and merge
them.

## Architecture And Merge Evidence

Temper itself was in scope. The WASM guest log correlation issue belonged at the
Temper host boundary, so it was fixed there instead of adding a TemperPaw-side
workaround.

- Temper PR: https://github.com/nerdsane/temper/pull/228
  - Merged: 2026-05-15T03:31:47Z
  - Main commit: `413ff6810b961317e93e275c5b4277d22501b318`
  - Change: removed duplicate uncorrelated WASM guest logs; guest log content is
    now represented by correlated span events.
- TemperPaw PR: https://github.com/nerdsane/temperpaw/pull/265
  - Merged: 2026-05-15T03:56:20Z
  - Main commit: `a90c3b0b69af14cad57525d391d00c9f4fece9df`
  - Change: pinned TemperPaw to the Temper host-boundary fix and corrected
    Railway Datadog tag syntax.
- TemperPaw PR: https://github.com/nerdsane/temperpaw/pull/267
  - Merged: 2026-05-15T04:59:04Z
  - Main commit: `f4e70e3fcddfd40408f5059f748a85e0582b93bc`
  - Change: added the live Error Tracking synthetic backend issue endpoint.
  - PR CI: `checks` succeeded in run `25900698435`.
- Current `main` validation for `f4e70e3fcddfd40408f5059f748a85e0582b93bc`:
  - CI run `25901155805`: success.
  - Docker run `25901155797`: success; image
    `ghcr.io/nerdsane/temperpaw:sha-f4e70e3`.

Earlier Railway Datadog implementation PRs are also merged into `main`,
including runtime Agent support, batched Railway variable upserts, and the
initial Railway blocker record.

## Live Railway Deployment

Project: `openpaw-seshendranalla`

Environment: production

Services after final deployment:

| Service | Deployment | Status |
| --- | --- | --- |
| `openpaw` | `0c3c2a20-5942-46b9-82f8-c36cd83ac263` | SUCCESS |
| `datadog-runtime-agent` | `1838ae10-f61d-4a5b-83e8-b45647a289e2` | SUCCESS |
| `otel-collector` | `2e87f6ac-dc9e-440d-bca6-7774362f096b` | SUCCESS |
| `datadog-postgres-agent` | `5d58739f-22c9-43c9-9c49-1c485efee1bb` | SUCCESS |
| `Postgres` | `fd6f0af6-0db7-4f94-add7-78b31244a5e4` | SUCCESS |

`GET /paw/version` after final canary cleanup:

```json
{
  "version": "sha-f4e70e3",
  "sha": "f4e70e3fcddfd40408f5059f748a85e0582b93bc"
}
```

Runtime Agent metric proof for `host:temperpaw-runtime-agent`, from
2026-05-15T04:20:00Z to 2026-05-15T05:45:00Z:

- `datadog.agent.running`: 150 points, sum 160.
- `datadog.trace_agent.heartbeat`: 149 points, sum 159.

## Live Agent Session

Proof id: `dd-proof-a90c3b0-20260515T042843Z`

Thread: `codex-datadog-proof-a90c3b0`

Session: `ss-019e29e4-ddee-7651-9fad-47fc09b6f924`

The session was dispatched through `Paw.Channel.ReceiveMessage` at
2026-05-15T04:28:43Z. It completed successfully after tool use and returned a
read-only diagnostic summary. The session result reported:

- Current agent: `aj-019d8cde-5bf6-7472-8ad1-2b2798c822b1`
- Active agents: 10
- Channel/thread context inspected: `Channels=1`, `ChannelSessions=1`,
  `AgentRoutes=1`
- Recent sessions listed: 10

Known transport caveat: the reply webhook was a local proof URL
(`127.0.0.1`) and produced a reply delivery failure. The session and telemetry
proof still completed; the failure is a proof transport setup issue, not an
agent execution or observability blocker.

## Product Coverage

| Datadog product | Status | Live evidence |
| --- | --- | --- |
| APM | supported | Datadog APM contains `service:temperpaw env:prod` spans from the runtime Agent. Aggregate query from 2026-05-15T04:20:00Z to 2026-05-15T05:45:00Z returned top resources including `GET /odata/{path}` with 6,728 spans, `dispatch.dispatch_tenant_action_core` with 141 spans, DB spans, dispatch phases, and OData/tdata spans. Trace `3301f15edd33b58bb632588fda352cc6` links to the proof session and includes `temper.action: Configure`, entity id `ss-019e29e4-ddee-7651-9fad-47fc09b6f924`, service `temperpaw`, env `prod`, version `a90c3b0b69af14cad57525d391d00c9f4fece9df`. |
| Logs correlation | supported | Datadog logs for the proof session returned four in-span logs with decimal `trace_id` and `span_id`: route creation/routing at 04:28:46Z, `FinalizeResult` at 04:29:52Z, and OTS trajectory emission at 04:29:57Z. Query `service:temperpaw env:prod wasm_guest` returned zero logs from 04:20Z to 05:45Z, proving the duplicate uncorrelated WASM guest log stream is gone. |
| LLM Observability | supported | LLMObs trace `67800715658509700452019680817418939590` has 21 spans, total duration 54,410.917 ms, tree depth 3, and span kinds `agent=1`, `workflow=3`, `llm=3`, `tool=14`. Root is `temperpaw.agent.session`; children include workflow spans, `wasm:provider_caller` LLM spans, and tool spans such as `temper.get_agent_id`, `temper.get_session_id`, `temper.list`, `temper.list_sessions`, and `json.dumps`. |
| Error Tracking | supported | Synthetic proof id `dd-error-f4e70e3-20260515T052429Z` emitted at 2026-05-15T05:24:29Z. Datadog APM trace `7699b3bc3747bdf80558f1e79ddd3662` contains errored span `datadog.error_tracking.synthetic` with `error.type`, `error.kind`, `error.message`, `error.stack`, and mirrored `exception.*` fields. Datadog assigned Error Tracking issue `5a9eda66-501e-11f1-8bbb-da7ad0900002`, URL https://app.datadoghq.com/error-tracking/issue/5a9eda66-501e-11f1-8bbb-da7ad0900002, state `FOR_REVIEW`, service `temperpaw`, language `RUST`, platform `BACKEND`. |
| On-demand profiling | supported | `GET /_admin/profile/cpu?seconds=5&frequency=100` returned HTTP 200, content type `application/vnd.google.protobuf`, filename `cpu-profile-5s.pb`, size 83 bytes, hash `ecdb3d378c2c97b0b9347d916d05b66a9228067c9f23c92629adde2268100592`. Datadog logs show capture started, capture completed, and `profile uploaded to Datadog Agent intake`. Metric `datadog.profiling.rust.profiles_uploaded{service:temperpaw,env:prod}` returned value 2 for `profile_type:cpu,version:f4e70e3fcddfd40408f5059f748a85e0582b93bc`; upload-error metric returned no data. |
| Continuous profiling | blocked-on-Railway-perf-permissions | Temporary canary set `TEMPER_DDPROF_ENABLED=true` and `DD_PROFILING_ENABLED=true`, redeployed `openpaw` as `6d06957a-669c-402f-a495-9b21b6b6133c`, and `GET /paw/infra/railway/datadog-capability-check` returned `continuous_profiler_status: blocked-on-Railway-perf-permissions`, `ddprof_present: true`, `perf_event_paranoid: "3"`, and `CAP_PERFMON: false`. The canary was then disabled and redeployed as `0c3c2a20-5942-46b9-82f8-c36cd83ac263`. |
| USM / Universal Service Monitoring | blocked-on-Railway-system-probe | `GET /paw/infra/railway/datadog-capability-check` returned `usm_status: blocked-on-Railway-system-probe`. Required system-probe capabilities and mounts are absent: `CAP_SYS_ADMIN=false`, `CAP_SYS_RESOURCE=false`, `CAP_SYS_PTRACE=false`, `CAP_NET_ADMIN=false`, `CAP_NET_RAW=false`, `CAP_IPC_LOCK=false`, `host_proc=false`, `host_cgroup=false`, `lib_modules=false`. Searches for USM/network metrics and system-probe logs returned no live USM data, consistent with the Railway capability block. |

## Error Tracking Details

Endpoint:

```text
POST /paw/infra/datadog/error-tracking-synthetic
```

Request body:

```json
{"proof_id":"dd-error-f4e70e3-20260515T052429Z"}
```

Response:

```json
{
  "emitted": true,
  "proof_id": "dd-error-f4e70e3-20260515T052429Z",
  "service": "temperpaw",
  "env": "prod",
  "version": "f4e70e3fcddfd40408f5059f748a85e0582b93bc",
  "error_type": "DatadogSyntheticBackendError",
  "error_message": "Synthetic Datadog Error Tracking backend issue for proof dd-error-f4e70e3-20260515T052429Z",
  "required_fields": [
    "error.type",
    "error.kind",
    "error.message",
    "error.stack",
    "exception.type",
    "exception.message",
    "exception.stacktrace"
  ]
}
```

Datadog issue lookup:

```text
issue_id: 5a9eda66-501e-11f1-8bbb-da7ad0900002
state: FOR_REVIEW
service: temperpaw
error_type: DatadogSyntheticBackendError
first_seen: 2026-05-15T05:24:29.561Z
first_seen_version: f4e70e3fcddfd40408f5059f748a85e0582b93bc
platform: BACKEND
languages: RUST
```

Note: `search_datadog_error_tracking_issues` did not return the issue by broad
search query during the proof window, but the errored span included
`issue.id: 5a9eda66-501e-11f1-8bbb-da7ad0900002`, and
`get_datadog_error_tracking_issue` retrieved the live backend issue by id.

## Continuous Profiling Canary

The governed canary endpoint is implemented and tested, but production's
server-side Railway token could not mutate Railway variables:

```text
POST /paw/infra/railway/datadog-continuous-profiler-canary
HTTP 502
{"error":"Railway Runtime Agent variableUpsert failed: Not Authorized"}
```

To complete the product proof, the same canary flags were applied with the
operator Railway CLI:

```text
railway variable set -s openpaw TEMPER_DDPROF_ENABLED=true DD_PROFILING_ENABLED=true
```

Canary deployment:

```text
2026-05-15T05:32:05Z status=SUCCESS deployment=6d06957a-669c-402f-a495-9b21b6b6133c
```

Capability response while enabled:

```json
{
  "usm_status": "blocked-on-Railway-system-probe",
  "continuous_profiler_status": "blocked-on-Railway-perf-permissions",
  "continuous_profiler": {
    "TEMPER_DDPROF_ENABLED": "true",
    "ddprof_present": true,
    "perf_event_paranoid": "3",
    "CAP_PERFMON": false
  }
}
```

Cleanup:

```text
railway variable set -s openpaw TEMPER_DDPROF_ENABLED=false DD_PROFILING_ENABLED=false
2026-05-15T05:32:59Z status=SUCCESS deployment=0c3c2a20-5942-46b9-82f8-c36cd83ac263
```

Final capability response after cleanup:

```json
{
  "usm_status": "blocked-on-Railway-system-probe",
  "continuous_profiler_status": "best-effort-canary-not-enabled",
  "continuous_profiler": {
    "TEMPER_DDPROF_ENABLED": "false",
    "ddprof_present": true,
    "perf_event_paranoid": "3",
    "CAP_PERFMON": false
  }
}
```

## Verification Commands

Representative local and live checks run for this proof:

```text
cargo test -p temperpaw --test datadog_observability_contract setup_api_can_emit_datadog_error_tracking_synthetic_issue -- --nocapture
cargo test -p temperpaw --test datadog_observability_contract -- --nocapture
cargo check -p temperpaw
cargo fmt --all
git diff --check
railway service status --all --json
GET /paw/version
GET /paw/infra/railway/datadog-capability-check
POST /paw/infra/datadog/error-tracking-synthetic
GET /_admin/profile/cpu?seconds=5&frequency=100
```

Datadog live queries covered:

```text
service:temperpaw env:prod
service:temperpaw env:prod status:error
service:temperpaw env:prod DatadogSyntheticBackendError
service:temperpaw env:prod ("ss-019e29e4-ddee-7651-9fad-47fc09b6f924" OR "dd-proof-a90c3b0-20260515T042843Z")
service:temperpaw env:prod wasm_guest
sum:datadog.profiling.rust.profiles_uploaded{service:temperpaw,env:prod} by {version,profile_type}.as_count()
sum:datadog.profiling.rust.upload_errors{service:temperpaw,env:prod} by {version,stage}.as_count()
sum:datadog.agent.running{host:temperpaw-runtime-agent}.as_count()
sum:datadog.trace_agent.heartbeat{host:temperpaw-runtime-agent}.as_count()
```

## Conclusion

The Railway-locked plan is implemented and live-proven with honest product
boundaries:

- Application telemetry remains OpenTelemetry-native.
- Datadog-enhanced Railway mode uses a dedicated `datadog-runtime-agent`.
- TemperPaw keeps the `portable-otel` fallback path.
- Temper host-boundary changes were made in Temper where the architecture
  required them.
- APM, Error Tracking, logs correlation, LLMObs, and on-demand profiling are
  live in Datadog for `service:temperpaw env:prod`.
- Continuous profiling is not supported on Railway without perf permission
  changes.
- USM is not supported on Railway without system-probe host/kernel access.
