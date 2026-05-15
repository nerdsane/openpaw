# Railway Datadog Product Coverage Proof Attempt

Date: 2026-05-15

Status: blocked before full end-to-end proof.

## Objective

Implement the Railway-locked Datadog observability plan from a clean branch off
main, prove it live end-to-end, create PRs into main, and merge them.

## Implementation Evidence

- PR #257, `[codex] Add Railway Datadog runtime agent coverage`, merged into
  `main` at `955be1beeccde2954366482855b55a9ac7f39b85`.
- PR #259, `Batch Railway Datadog variable upserts`, merged into `main` at
  `f80f185c85cc70590bd5bfa58fc5d329d136b9db`.
- `origin/main` used for this proof branch is `db23cc0d`, which includes #257
  and #259.
- The Docker workflow for #257 completed successfully and published `edge` for
  `955be1beeccde2954366482855b55a9ac7f39b85`.
- The Docker workflow for #259 completed successfully and published the fixed
  image digest:
  `ghcr.io/nerdsane/temperpaw@sha256:18c4db119e556bbb05929f0ff66ba3d83f50032e71f520daa135a8f2fcbd42bc`.

## Local Verification

Before #257 merged:

- `cargo fmt --all`
- `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
- `cargo build --workspace --locked`
- `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture`
- `cargo test -p temperpaw-cli datadog_runtime -- --nocapture`
- `git diff --check`
- `cargo clippy --locked -p temperpaw -p paw-codex-worker --all-targets -- -D warnings`

Before #259 merged:

- Red test:
  `cargo test -p temperpaw --test datadog_observability_contract railway_runtime_agent_variable_upserts_are_batched_before_redeploy -- --nocapture`
  failed because `railway_upsert_variable` did not set `skipDeploys`.
- Green test:
  `cargo test -p temperpaw --test datadog_observability_contract railway_runtime_agent_variable_upserts_are_batched_before_redeploy -- --nocapture`
  passed after adding `"skipDeploys": true`.
- Full contract:
  `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
  passed with 29 tests.
- Existing Railway redeploy guard:
  `cargo test -p temperpaw --test temperpaw_identity_contract railway_redeploy_uses_current_deployment_api -- --nocapture`
  passed.
- `git diff --check` passed.
- PR #259 CI passed in GitHub Actions.

## Live Railway Attempt

### Existing Production

Production URL checked:

- `https://openpaw-production.up.railway.app`

Observed state before any successful production deploy:

- `/paw/version` returned
  `86bd073dc89efc6e559cbdf9787ce9e0b92228fe`.
- `/paw/infra/edge` reported the merged #257 edge SHA:
  `955be1beeccde2954366482855b55a9ac7f39b85`.
- `/paw/infra/railway/redeploy` with `{"image_tag":"edge"}` failed:
  `{"error":"Not Authorized"}` with HTTP 502.
- Local Railway CLI could not access the existing production project
  `openpaw-seshendranalla`; `railway status` for that project reported
  unauthorized.

Result: existing production could not be updated with the current available
Railway credentials.

### Accessible Railway Canary

The local Railway account could access project `codex-pawfs-deploy-probe` with
service `temperpaw`.

Canary URL:

- `https://temperpaw-production.up.railway.app`

Canary deploy evidence:

- The canary was deployed to the #257 image and `/paw/version` returned
  `955be1beeccde2954366482855b55a9ac7f39b85`.
- `/readyz` returned ready.
- `/paw/infra/railway/status` returned `configured: true`, `can_update: true`,
  and `datadog_runtime_agent_service_id: null`.

Runtime Agent setup evidence:

- `POST /paw/infra/railway/datadog-runtime-agent/ensure` created a live Railway
  service named `datadog-runtime-agent`.
- Created service id:
  `9af19426-8c38-4252-ab7c-036a1d8fed54`.
- Railway showed both services:
  `temperpaw` and `datadog-runtime-agent`.
- Runtime Agent variables were present, including:
  `DD_APM_ENABLED=true`, `DD_APM_NON_LOCAL_TRAFFIC=true`,
  `DD_LOGS_ENABLED=true`,
  `DD_OTLP_CONFIG_RECEIVER_PROTOCOLS_HTTP_ENDPOINT=0.0.0.0:4318`,
  `DD_OTLP_CONFIG_RECEIVER_PROTOCOLS_GRPC_ENDPOINT=0.0.0.0:4317`, and
  `DD_PROCESS_AGENT_ENABLED=true`.
- Datadog metric evidence confirmed the Agent reported:
  `datadog.agent.running{host:temperpaw-runtime-agent}` and
  `datadog.trace_agent.heartbeat{host:temperpaw-runtime-agent}` emitted one
  point per minute beginning at `2026-05-14T23:02:00Z`.

Live blocker discovered:

- The first ensure call failed while setting Railway variables:
  `Railway Runtime Agent variableUpsert failed: Service deployment rate limit exceeded`.
- Root cause: the shared Runtime Agent/app variable helper upserted many
  variables without `skipDeploys`, causing Railway to start too many deployments.
- Fix shipped in PR #259: shared `railway_upsert_variable` now sets
  `"skipDeploys": true` and the setup flow relies on the explicit final service
  redeploys.

Second live blocker:

- After #259 merged and the fixed Docker image was published, redeploying the
  accessible canary failed:
  `Your trial has expired. Please select a plan to continue using Railway.`
- After that, `railway service status --all` showed `deploymentId: null` and
  `status: null` for both `temperpaw` and `datadog-runtime-agent`.
- `railway restart --service temperpaw --yes --json` failed:
  `No deployment found for service`.
- The canary public URL returned HTTP 404 for `/paw/version`, `/readyz`,
  `/paw/infra/railway/status`, and
  `/paw/infra/railway/datadog-capability-check`.

Result: the accessible Railway canary was disabled before the fixed setup
endpoint could be redeployed and before a full product proof could run.

## Datadog Product Status

| Product | Status | Evidence |
| --- | --- | --- |
| APM | Not fully proven | Runtime Agent trace-agent heartbeat reached Datadog, but the app was still exporting OTLP to `localhost:4318` before the env restart/deploy was blocked. |
| Universal Service Monitoring | Not proven | Capability endpoint could not be queried after the canary was disabled. ADR classifies USM as blocked if Railway cannot provide system-probe privileges. |
| Error Tracking | Not proven | Synthetic backend issue was not generated because the canary was disabled before the proof session. |
| Logs correlation | Not proven | Datadog contained old production logs for `service:temperpaw`, but the canary app did not restart with the Runtime Agent endpoint before Railway disabled it. |
| LLMObs | Not proven | No real agent session could be run after Runtime Agent wiring because the canary was disabled. |
| On-demand profiling | Not proven in this canary | The profile endpoint could not be called after the canary became 404. |
| Continuous profiling | Not proven | Continuous `ddprof` canary could not be run after the Railway deployment blocker. |

## Completion Audit

Completed:

- ADR and docs for Railway Datadog product coverage.
- Dedicated `datadog-runtime-agent` service support in deploy/config/setup code.
- Runtime Agent env contract for APM, OTLP HTTP/gRPC ingest, logs, unified tags,
  and process agent.
- TemperPaw env contract for `datadog-enhanced-railway` and `portable-otel`.
- Direct LLMObs export contract when bypassing collector.
- Session/tool/error/log correlation contract tests.
- Profiling contract docs/tests distinguishing on-demand and continuous lanes.
- Railway USM/continuous profiler capability endpoint implementation.
- Follow-up fix for live Railway rate limiting on variable upserts.
- PRs #257 and #259 merged into main.

Not completed:

- Live end-to-end Datadog product proof.
- Live proof that the fixed `datadog-runtime-agent/ensure` endpoint completes
  after #259.
- Real agent session with multiple tool calls visible in APM and LLMObs.
- Synthetic backend error visible in Error Tracking.
- Trace/log correlation proof for the test session.
- On-demand CPU profile upload proof from the canary.
- Continuous profiler canary proof or Railway perf-permission blocker proof.
- USM metrics proof or Railway system-probe blocker proof.

## Required Next Action

One of these must happen before the objective can be completed:

1. Restore Railway access to the existing production project
   `openpaw-seshendranalla`, then deploy current `main` and run the full proof.
2. Select a Railway plan or otherwise re-enable deployments for the accessible
   `codex-pawfs-deploy-probe` canary, then redeploy current `main` and rerun
   the full proof.

After Railway is unblocked, rerun:

1. Deploy current `main` image.
2. `POST /paw/infra/railway/datadog-runtime-agent/ensure`.
3. `GET /paw/infra/railway/datadog-capability-check`.
4. Run a real agent session with multiple tool calls.
5. Invoke `/_admin/profile/cpu`.
6. Run the temporary continuous profiler canary only if the capability check
   does not already prove Railway perf permissions are blocked.
7. Query Datadog for APM, LLMObs, logs correlation, Error Tracking, profile
   upload, and USM status.

The active goal is not complete until that live proof is green or each remaining
Datadog/Railway blocker is proven with live evidence.
