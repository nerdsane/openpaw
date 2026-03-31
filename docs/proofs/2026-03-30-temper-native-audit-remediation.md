# Temper-Native Audit Remediation Proof

Date: 2026-03-30
Branch: `feat/openpaw-self-heal-loop-codex`

## Scope

This proof covers the two remaining architectural issues from the Temper-native audit:

1. `route_message` blocked a WASM execution slot by long-polling `wait_for_agent()` for up to 5 minutes.
2. Runtime WASM modules authenticated as `admin`, which bypassed Cedar evaluation and made the policies decorative.

## Architectural Changes

### 1. Channel routing is now fire-and-forget

- `os-apps/paw-channels/wasm/route_message/src/lib.rs` no longer calls `wait_for_agent()`.
- The channel route path now:
  - creates/configures/provisions the `Agent`
  - creates or updates the `ChannelSession`
  - returns immediately with a routed result payload
- Human-visible replies now arrive asynchronously from the agent lifecycle instead of from a blocked channel WASM call.

### 2. Reply delivery is now driven by entity transitions

- Added `os-apps/paw-agent/wasm/agent_reply/`.
- Added `deliver_reply` integration wiring in `os-apps/paw-agent/specs/agent.ioa.toml`.
- Added `deliver_reply` effects on terminal agent actions so that completed/failed/cancelled agents can look up their `ChannelSession` and dispatch `Channel.SendReply`.

### 3. Runtime WASM no longer uses `admin`

- Shared WASM headers in `os-apps/paw-agent/wasm/wasm-helpers/src/lib.rs` now send:
  - `x-temper-principal-kind: agent`
  - `x-temper-principal-id: <triggering entity id>`
- Runtime WASM modules in `paw-agent`, `paw-channels`, `paw-ingest`, and `paw-heal` were updated to use scoped runtime headers instead of `admin`.
- Bootstrap remains elevated only where expected during daemon startup and installation.

### 4. Cedar now governs runtime behavior

- Cedar policies were updated to permit required runtime behavior without restoring `admin`.
- The key agent-policy fix was to authorize self-actions with `principal == resource` and parent callbacks with `context.parent_agent_id == principal.id`, which matches the way the runtime supplies bound-action authorization context.

## Static Verification

### Command

```sh
rg -n "wait_for_agent" os-apps/paw-channels -g '*.rs'
```

### Result

- No matches.

### Command

```sh
rg -n "principal-kind.*admin|x-temper-principal-kind.*admin|\"admin\"\\.to_string\\(\\)" os-apps -g '*.rs'
```

### Result

- No runtime WASM matches under `os-apps/`.

## Runtime Verification

Fresh daemon used for proof:

```sh
PORT=4472 \
TURSO_URL=file:/tmp/openpaw-arch-proof-20260330g.db \
RUST_LOG=info \
./target/debug/openpaw
```

### Proof 1: asynchronous channel continuation

Command:

```sh
python3 scripts/prove_channel_continuation.py --base-url http://127.0.0.1:4472
```

Observed result:

- `SESSION_CONTINUED=true`
- First reply: `REMEMBERED moon-biscuit-42`
- Second reply: `RECALL moon-biscuit-42`
- `second_parent_agent_id` matched the first agent
- The same `ChannelSession` and session file were reused across the continuation

Evidence from the successful run:

- Channel: `019d4112-6d51-7c71-8926-2ebd6292e168`
- First agent: `019d4112-6e86-7c33-a004-006c411e71a3`
- Second agent: `019d4112-73f1-79e0-b769-1303b2f5f04b`
- Channel session: `019d4112-6e98-7282-852d-8efea4c150c3`
- Session file: `019d4112-6ee6-7540-8728-269850d2b737`

Conclusion:

- `Channel.ReceiveMessage` no longer waits for the model run inline.
- Replies are delivered asynchronously after agent lifecycle progress.

### Proof 2: webhook to SRE spawn with Cedar-governed runtime principals

Command:

```sh
python3 scripts/prove_webhook_to_sre.py --base-url http://127.0.0.1:4472
```

Observed result:

- `WebhookEvent.status = Processed`
- `AlertCycle.status = Triaging`
- `AlertCycle.fields.sre_agent_id` was populated
- SRE `Agent.status = Provisioning`
- Routed action: `OpenPaw.Heal.Open`
- Routed entity type: `AlertCycle`

Evidence from the successful run:

- ProjectHarness: `019d4112-6d52-7410-83ed-173b190b61cd`
- WebhookRoute: `019d4112-6d66-7660-8f8b-673aaefb7faa`
- WebhookEvent: `019d4112-6d70-71c1-92bb-f5bb8bab8225`
- AlertCycle: `019d4112-6dae-7513-a9b2-212ba6afc925`
- SRE agent: `019d4112-6ddc-70d0-986a-3a5cfcf71663`

Daemon log evidence from the same clean run:

- Agent `Heartbeat` succeeded under the new runtime principal path:
  - `action=Heartbeat success=true ... authz_denied=None`

Conclusion:

- The webhook flow still works without runtime `admin` headers.
- Cedar is being evaluated for runtime agent actions instead of being bypassed.

## Notes

- The webhook proof intentionally validates the architectural checkpoint requested by the audit: `WebhookEvent -> AlertCycle.Open -> SRE agent spawned and progressing`.
- It does not require the full remediation loop to finish, since that depends on downstream sandbox and model execution and is not the architectural concern being tested here.
- For synchronous `Channel.ReceiveMessage` callers, the correct behavior is now an immediate routed response followed by asynchronous delivery through the channel/session path.

## Final Outcome

The blocking `wait_for_agent()` pattern is removed from channel routing, reply delivery is now driven by entity state transitions, runtime WASM calls no longer run as `admin`, and the affected end-to-end flows still work with Cedar-governed principals.
