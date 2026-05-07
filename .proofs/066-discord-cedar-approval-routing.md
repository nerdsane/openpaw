# 066 Discord Cedar Approval Routing

Date: 2026-05-07

## Change

- `request_approval` now resolves Discord delivery by current `session_entity_id` first, then parent session, then persistent agent binding.
- This matches `agent_reply` routing so Cedar denials from child sessions can surface in the same Discord thread instead of falling back to out-of-band approval.

## Red

```text
cargo test
error[E0061]: this function takes 6 arguments but 5 arguments were supplied
```

The initial signature change exposed the missing current-session lookup path.

## Green

```text
cargo fmt --check
cargo test
running 3 tests
test tests::channel_session_lookup_deduplicates_resumed_parent_session ... ok
test tests::channel_session_lookup_escapes_odata_values ... ok
test tests::channel_session_lookup_prefers_current_session_then_parent_then_agent_binding ... ok
```

## Build

```text
cargo build --target wasm32-unknown-unknown --release
request_approval.wasm sha256 d1492c00e9c42f431b91353bd5afe08f2040c40c6241e4b933d47b5be02a2a8e
```

## Production Hot-Load

```json
{"module_name":"request_approval","sha256_hash":"d1492c00e9c42f431b91353bd5afe08f2040c40c6241e4b933d47b5be02a2a8e","size_bytes":275564}
```

## Live Recovery

- Failed stale approval-gated session `ss-019e04a8-8d5b-7761-97c3-53b91f4490f1`.
- Expired stranded Discord `ChannelSession` `en-019e0497-87f7-7493-afe4-9414cc28fca7`.
- Verified replacement Discord session `ss-019e04c7-4be7-7f70-8d0f-66356bc17d5d` reached `Completed`.
