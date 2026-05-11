# 073: Governance Approval Scope Routing

Date: 2026-05-11

## Scope

Investigated and fixed the approval path for governed agent actions when a PO
session delegates work to managed SWE sessions.

The change covers:

- richer Discord/Slack approval prompts
- complete Cedar approval scope payloads
- scoped approval choices
- CLI/TUI approval command parity
- managed-session parent provenance so delegated approvals route back to the
  originating channel-bound session

## Red

Added regression coverage in
`crates/temperpaw/tests/session_lifecycle_and_config.rs`:

- `governance_approval_prompts_include_decision_details_and_scope_choices`
- `managed_agent_inner_sessions_preserve_parent_for_approval_routing`

Initial run failed as expected because:

- approval notifications did not include detailed pending-decision context or
  scoped approval choices
- transport approval payloads did not build the complete Cedar scope matrix
- `ManagedSession` did not preserve `parent_session_id`

## Green

Implemented the fix across:

- `os-apps/paw-agent/wasm/request_approval/src/lib.rs`
- `crates/paw-transport/src/lib.rs`
- `crates/paw-transport/src/discord/transport.rs`
- `crates/paw-transport/src/slack/transport.rs`
- `crates/temperpaw-cli/src/tui.rs`
- `crates/temperpaw-cli/src/events.rs`
- `os-apps/paw-managed-agents/specs/managed_session.ioa.toml`
- `os-apps/paw-managed-agents/wasm/session_orchestrator/src/lib.rs`
- `os-apps/paw-agent/wasm/monty_repl/src/dispatch.rs`

## Verification

Passed:

```bash
cargo fmt --all
cargo test -p temperpaw --test session_lifecycle_and_config
cargo test -p paw-transport
cargo test -p temperpaw-cli
(cd os-apps/paw-agent/wasm/request_approval && cargo test)
(cd os-apps/paw-managed-agents/wasm/session_orchestrator && cargo test)
(cd os-apps/paw-agent/wasm/monty_repl && cargo test)
cargo build --workspace
git diff --check
```

Observed results:

- `session_lifecycle_and_config`: 6 passed
- `paw-transport`: 27 passed
- `temperpaw-cli`: 30 passed
- `request_approval`: 3 passed
- `session_orchestrator`: 7 passed
- `monty_repl`: 49 passed
- workspace build completed successfully
- `git diff --check` completed successfully

## Local Live Smoke

Started an isolated local server with temporary HOME, local Turso file storage,
telemetry disabled, and chat credentials blanked:

```bash
env -i \
  PATH="$PATH" \
  HOME="/tmp/temperpaw-approval-smoke.QR906n" \
  PORT=3897 \
  OTEL_ENABLED=false \
  TEMPER_EVENT_STORE=turso \
  TEMPER_PLATFORM_STORE=turso \
  TEMPER_QUERY_PROJECTION_STORE=turso \
  TURSO_URL="file:/tmp/temperpaw-approval-smoke.QR906n/paw.db" \
  TEMPERPAW_WASM_STARTUP_POLICY=build \
  TEMPERPAW_ORPHANED_SESSION_RECOVERY=false \
  TEMPERPAW_QUERY_PROJECTION_BACKFILL_ON_STARTUP=false \
  ./target/debug/temperpaw-server
```

Initial boot surfaced missing WASM artifacts for `paw-ingest`, `paw-patrol`,
and `paw-skills`. Built those app WASMs and retried. The server then reached:

```text
/readyz -> {"status":"ready","healthz":"/healthz","discord":{"status":"disconnected","configured":false,"connected":false}}
```

Because Discord credentials are not present in the local checkout, the live
transport was represented by a local webhook sink on `127.0.0.1:3902`. The
smoke then exercised the Temper state path end-to-end:

1. Created a connected `Channel` with `webhook_url=http://127.0.0.1:3902/approval`.
2. Created an active `ChannelSession` bound to a parent session id:
   `session_entity_id=ss-parent-pause-1778516095`.
3. Created a real pending governance decision through `POST /api/authorize`:
   `PD-019e17d1-f39a-7453-b160-fed6bf04e0a3`.
4. Created a child `Session` with:
   `agent_id=aj-child`, `parent_session_id=ss-parent-pause-1778516095`.
5. Drove the child through:
   `Created -> Provisioning -> PreparingContext -> CallingProvider -> Executing`.
6. Dispatched `TemperPaw.PauseForApproval` with the pending decision id.
7. Observed `request_approval` register the GovernanceDecision callback and post
   the rich approval message to the webhook.

Observed child session:

```text
session=ss-019e17d1-f60f-70a1-94fb-6fdee68d7e9e
parent=ss-parent-pause-1778516095
status=WaitingForApproval
events=Created,Configure,ProvisionWorkspace,WorkspaceReady,ContextReady,ProviderAuthReady,Heartbeat,ProviderResponseReady,ProcessToolCalls,PauseForApproval
```

Observed GovernanceDecision:

```text
entity_id=GD-019e17d1-f39b-7af3-9763-c926495958e2
status=Pending
fields.callback_entity_id=ss-019e17d1-f60f-70a1-94fb-6fdee68d7e9e
fields.callback_entity_set=Sessions
fields.callback_on_approve=ResumeAfterApproval
fields.callback_on_deny=Fail
events=Created,CreateGovernanceDecision,RegisterCallback
```

Observed webhook body included:

```text
Agent: `aj-child`
Action: `temper.write`
Resource: `File/approval-pause-1778516095`
Decision: `PD-019e17d1-f39a-7453-b160-fed6bf04e0a3`
buttons: approve_always, approve_session, approve_once, deny
thread_id=thread-pause-1778516095
```

Simulated the approval button path by POSTing the same scoped payload used by
Discord/Slack handlers:

```json
{
  "scope": {
    "principal": "this_agent",
    "action": "this_action",
    "resource": "any_of_type",
    "duration": "always"
  },
  "decided_by": "smoke:button"
}
```

Observed approval/resume:

```text
approve_response.status=approved
generated_policy=permit principal Agent::"aj-child", action Action::"temper.write", resource is File
GovernanceDecision.status=Approved
GovernanceDecision.events=Created,CreateGovernanceDecision,RegisterCallback,Approve
Session.events include ResumeAfterApproval
Session.status=Completed
```

This local smoke verifies the delegated child-session approval route through
parent `ChannelSession` lookup, rich notification delivery, callback
registration, scoped approval body acceptance, and resumed child session. The
remaining untested part in this checkout is the external Discord gateway itself;
that requires real Discord credentials and connectivity.

## Architecture Records

Recorded:

- `os-apps/paw-agent/adrs/007-scoped-governance-approval-notifications.md`
- `os-apps/paw-managed-agents/adrs/002-parent-session-provenance-for-approval-routing.md`
