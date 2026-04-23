# WASM Timeout And Observability Fix

Date: 2026-04-23

## Worktrees

- Temper: `/Users/seshendranalla/Development/temper-worktrees/fix-wasm-timeout-observability`
- Temper branch: `codex/fix-wasm-timeout-observability`
- OpenPaw: `/Users/seshendranalla/Development/openpaw/.worktrees/fix-temper-wasm-observability`
- OpenPaw branch: `codex/fix-temper-wasm-observability`

## Root Cause

The failing Datadog trace showed `wasm:provider_caller` failing with
`execution timeout -- module exceeded time budget of 600s` even though the
provider call had already returned successfully and the visible runtime was far
below 600 seconds.

The platform bug was in Temper's WASM engine: every invocation created its own
timer that called `Engine::increment_epoch()` on timeout while each store used
`set_epoch_deadline(1)`. Wasmtime epochs are global to the shared engine, so one
timed-out invocation could interrupt unrelated active stores. This matched the
nearby timeout cascade in the trace.

## Fix

- Replaced per-invocation epoch increment timers with one shared engine epoch
  ticker.
- Converted each invocation timeout into a per-store relative epoch deadline.
- Added a regression test where an infinite-loop module times out while a
  different active module is in a slow host call; the slow invocation must not
  be interrupted.
- Added Datadog/OpenTelemetry-standard error fields (`error.type`,
  `error.message`, `exception.message`) to WASM wide events, dispatch spans, and
  `wasm.invoke` spans.
- Added explicit `wasm_guest.log` OTel span events for guest logs and structured
  logs, including severity, message, entity/session context, and structured
  fields JSON.
- Added a named tracing span event path for guest logs so the exported OTLP
  payload contains the stable event name `wasm_guest.log` in addition to the
  normal `wasm_guest` log record.

## Verification

Commands run from the Temper worktree:

```text
cargo test -p temper-wasm timed_out_invocation_does_not_interrupt_unrelated_active_invocation -- --nocapture
result: passed

cargo test -p temper-observe test_wasm_invocation_with_error -- --nocapture
result: passed

cargo test -p temper-wasm guest_log_span_attrs_include_message_and_invocation_context -- --nocapture
result: passed

cargo test -p temper-wasm guest_log_span_event_is_named_for_trace_export -- --nocapture
result: passed

cargo fmt
result: passed

cargo test -p temper-wasm
result: 70 unit tests passed, 5 e2e_invoke tests passed

cargo test -p temper-observe
result: 48 tests passed

cargo test -p temper-server strips_private_llm_observability_params_before_callback_dispatch -- --nocapture
result: passed
```

OpenPaw smoke from the OpenPaw worktree:

```text
cargo test -p temperpaw
result: 36 temperpaw-server unit tests passed, 4 session_turn_architecture tests passed
note: this smoke used the existing local Cargo Temper patch to /Users/seshendranalla/Development/temper, so it verifies OpenPaw package health but not the new Temper worktree changes.
```

## Live Local E2E

Ran a full local server E2E from the OpenPaw worktree using a temporary
`.cargo/config.toml` patch to the fixed Temper worktree.

```text
TEMP_DIR=/var/folders/6m/lm283ng13931_42z4z8n1x7c0000gn/T/openpaw-live-e2e-blpnrg7_
SERVER_PORT=58491
OTLP_PORT=58492
SLOW_PORT=58493
HEALTHZ=200
CREATE_POLICY=201
UPLOAD_E2E_SLOW=200
UPLOAD_E2E_SPIN=200
LOAD_INLINE=200, verification all_passed=true for E2eWasmProbe
CREATE_SLOW=201
CREATE_SPIN=201
CONCURRENT_ACTIONS_ELAPSED=1.822
SLOW_ACTION=200, status=SlowDone
SPIN_ACTION=200, status=Failed, error="execution timeout -- module exceeded time budget of 1s"
OBSERVE_WASM_INVOCATIONS=200, e2e_spin success=false with timeout error
OTLP_BLOBS=27
OTLP_CHECKS={
  guest_log_event: true,
  slow_log_message: true,
  spin_log_message: true,
  error_message_attr: true,
  timeout_text: true,
  spin_module: true,
  slow_module: true
}
LIVE_E2E=PASS
SERVER_LOG=/var/folders/6m/lm283ng13931_42z4z8n1x7c0000gn/T/openpaw-live-e2e-blpnrg7_/server.log
TRACE_STRINGS=/var/folders/6m/lm283ng13931_42z4z8n1x7c0000gn/T/openpaw-live-e2e-blpnrg7_/trace_strings.txt
```

This verified the failure fix end-to-end: while `e2e_spin` timed out at one
second, the concurrent `e2e_slow` invocation stayed alive through its slow host
HTTP call and completed `Created -> RunningSlow -> SlowDone`. The timeout path
also self-reported through entity state, persisted in `/observe/wasm/invocations`,
and exported trace payloads with guest logs and error attributes.

## OpenPaw Integration Note

OpenPaw currently pins Temper from `https://github.com/nerdsane/temper.git`
branch `main` at `7858b428074acfa4e75d4c7e9b90e9a4e66e3a82`. No OpenPaw
runtime workaround was added because the clean fix is in Temper. After the
Temper branch merges, OpenPaw should update its Temper git dependency lock to
the merged commit.
