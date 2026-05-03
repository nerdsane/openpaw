# Proof Report: 064 - CLI TUI

## Date

2026-05-02

## Scope

- Wired the existing terminal UI modules into the `temperpaw` CLI as `temperpaw tui`.
- Added the CLI dependencies needed by the TUI/event watcher.
- Kept inbound TUI messages on the existing Temper-native channel path by dispatching `Paw.Channel.ReceiveMessage`.
- Added inline reply delivery for `cli`/`tui` channels in `send_reply`, so terminal clients can observe `ReplyDelivered` events without a Slack/Discord webhook.
- Preserved the webhook requirement for non-terminal transports.

## Verification

| Check | Result |
| --- | --- |
| Red test: `cargo test -p temperpaw-cli parses_tui_command -- --nocapture` before wiring | Failed because `tui` was not a valid subcommand |
| Red test: `cargo test --manifest-path os-apps/paw-channels/wasm/send_reply/Cargo.toml cli_channel_without_webhook_delivers_inline -- --nocapture` before helper | Failed because inline delivery helper did not exist |
| `cargo test -p temperpaw-cli -- --nocapture` | 30 passed |
| `cargo test --manifest-path os-apps/paw-channels/wasm/send_reply/Cargo.toml -- --nocapture` | 2 passed |
| `cargo test -p paw-transport -- --nocapture` | 22 passed |
| `cargo check -p temperpaw-cli -p paw-transport` | Passed |
| `cargo check -p temperpaw` | Passed |
| `cargo fmt --check` | Passed |
| `bash os-apps/paw-channels/wasm/build.sh` | Rebuilt channel WASM modules, including updated `send_reply.wasm` |
| `cargo run -p temperpaw-cli -- tui --help` | Rendered TUI command options |
| Local server `PORT=3467 OTEL_ENABLED=false RUST_LOG=info cargo run -p temperpaw --` | Booted; `/readyz` returned `HTTP/1.1 200 OK` |
| Termwright TUI `/status` | TUI rendered connected state and showed `profile=codex-tui` |
| Termwright normal message `tui smoke reply` | TUI displayed `paw: tui smoke reply` |
| OData channel audit | Channel events ended with `ReceiveMessage`, `SendReply`, `ReplyDelivered` for thread `smoke` |
| OData ChannelSession audit | `cli:codex-tui`/`smoke` ChannelSession was `Active` and pointed at Session `ss-019deaf4-7649-70e3-9c06-91dff218dc7e` |
| OData Session audit | Session completed with `provider=mock`, `model=mock`, `result=tui smoke reply` |
| Termwright `/execute tui execute smoke` | TUI displayed `paw: tui execute smoke` |
| OData `/execute` audit | Channel event had `ReceiveMessage(command=execute)` followed by `SendReply` and `ReplyDelivered`; backing Session completed |
| Quiet fallback rerun | TUI displayed `paw: quiet fallback smoke`, did not show `event stream error`, and Session `ss-019deaf7-9961-7cd1-8ce4-291772bcbc3a` completed with `provider=mock` |
| Red test: `cargo test --manifest-path os-apps/paw-agent/wasm/context_preparer/Cargo.toml prepared_context_storage -- --nocapture` before helper | Failed because `choose_prepared_context_storage` did not exist |
| `cargo test --manifest-path os-apps/paw-agent/wasm/context_preparer/Cargo.toml -- --nocapture` | 9 passed |
| Uploaded `context_preparer.wasm` to running server | `sha256=1f5ffba399979699c0f9f88db929cd94628ba421d37c4cd12045944aa4a3a30c` |
| Large prepared-context live smoke | Session `ss-019deb0f-4e5e-74e2-9056-64b9aedcae8a` completed; `prepared_context_file_id=fl-019deb0f-4ef6-7341-b878-152f01b8ba49`, inline JSON length `0` |
| Red test: `cargo test --manifest-path os-apps/paw-channels/wasm/route_message/Cargo.toml continuation_drops_inline_prepared_context -- --nocapture` before helper | Failed because `continuation_prepared_context_storage` did not exist |
| `cargo test --manifest-path os-apps/paw-channels/wasm/route_message/Cargo.toml -- --nocapture` | 12 passed |
| Uploaded `route_message.wasm` to running server | `sha256=f69fca35141e3fef24a589803b76bdc5b84aa17b37e697231337a76759361794` |
| Continuation live smoke on hidden `codex-smoke` thread | Session `ss-019deb10-d4bb-7810-975f-4253d0d58f1f` completed; continuation `Configure` carried inline JSON length `0`; reply delivered as `ok\n\nok2` |

## Notes

- Event-stream failures fall back to polling without adding noisy system messages to the chat scrollback.
- Live TUI verification used a channel-specific mock `AgentRoute` for `cli:codex-tui` to avoid depending on external LLM credentials during the smoke test.
- The mock `AgentRoute` was disabled after verification.
- The local test server and termwright daemon were stopped after verification.
- The later provider-caller crash was caused by repeated inline prepared-context artifacts in Session state/history; the fix externalizes large artifacts and prevents continuation sessions from copying inline artifacts forward.
