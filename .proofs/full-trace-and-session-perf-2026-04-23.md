# Full Trace And Session Perf Verification

Date: 2026-04-23
Worktree: `/Users/seshendranalla/Development/openpaw-worktrees/full-trace-and-session-perf`
Branch: `codex/full-trace-and-session-perf`

## Scope

- Reuse exact-match completed `WebQuery` entities before creating new search/fetch work, reducing duplicate research turns during long sessions.
- Batch safe read-only `monty_repl` tool calls so repeated `temper.web_search(...)`, `temper.web_fetch(...)`, and selected read endpoints no longer serialize one-by-one inside a tool batch.

## Commands

1. `cargo test web_query_cache_lookup_path_escapes_single_quotes -- --nocapture`
   Result: passed
2. `cargo test batchable_tool_plan_parses_web_fetch_literal --config 'patch."https://github.com/nerdsane/temper.git".temper-wasm-sdk.path="/Users/seshendranalla/Development/temper-worktrees/full-trace-and-session-perf/crates/temper-wasm-sdk"' -- --nocapture`
   Result: passed
3. `cargo test batchable_run_len_stops_before_non_batchable_snippet --config 'patch."https://github.com/nerdsane/temper.git".temper-wasm-sdk.path="/Users/seshendranalla/Development/temper-worktrees/full-trace-and-session-perf/crates/temper-wasm-sdk"' -- --nocapture`
   Result: passed
4. `cargo test --lib --config 'patch."https://github.com/nerdsane/temper.git".temper-wasm-sdk.path="/Users/seshendranalla/Development/temper-worktrees/full-trace-and-session-perf/crates/temper-wasm-sdk"' -- --nocapture`
   Result: passed (`38` tests)
5. `cargo build --target wasm32-wasip1 --release --config 'patch."https://github.com/nerdsane/temper.git".temper-wasm-sdk.path="/Users/seshendranalla/Development/temper-worktrees/full-trace-and-session-perf/crates/temper-wasm-sdk"'`
   Result: passed

## Notes

- The crate emitted one pre-existing `unused doc comment` warning in `src/lib.rs`.
- Local verification uses a Cargo patch to point `monty_repl` at the Temper worktree's `temper-wasm-sdk`, because the crate dependency still targets `https://github.com/nerdsane/temper.git` `main`.
- The expanded test suite covers the batch-planning parser, checkpoint-safe run grouping, cache lookup path, OData escaping, and cached-result interpretation helpers.

## Addendum: 2026-04-24 Session Query-Plane Verification

### Additional Scope

- Mark Session hot fields `last_heartbeat_at`, `progress_token`, and `last_progress_at` as `query_indexed = false`.
- Verify a live local Session still completes while those fields remain absent from `entity_field_index`.

### Additional Commands

6. `cargo build -p temperpaw --config /tmp/openpaw-local-temper-patch.toml`
   Result: passed
7. `HOME=/tmp/openpaw-full-trace-home PORT=4476 TURSO_URL=file:/tmp/openpaw-full-trace.db TEMPER_API_KEY=live-e2e-key OPENAI_API_KEY=dummy-local-key RUST_LOG=info,temper_server::state::dispatch=debug,llm_caller=debug,monty_repl=debug,temperpaw_server::startup=info TEMPERPAW_WASM_STARTUP_POLICY=warn ./target/debug/temperpaw-server`
   Result: server booted successfully on `:4476`
8. Live Session E2E:
   - Create `Session` with `Id=full-trace-e2e-v2`
   - Configure with provider `mock`, model `mock-model`, max_turns `5`
   - Submit a mock tool transcript that dispatches `temper.done("live e2e complete via Monty tool")`
   Result: Session `ss-019dbea5-925a-7872-aec0-0f0e4fc90a7d` completed with `sequence_nr=15`
9. Query-plane DB verification:
   - `sqlite3 /tmp/openpaw-full-trace.db "SELECT field_name, field_value FROM entity_field_index WHERE tenant='default' AND entity_type='Session' AND entity_id='ss-019dbea5-925a-7872-aec0-0f0e4fc90a7d' AND field_name IN ('last_heartbeat_at','last_progress_at','progress_token','result') ORDER BY field_name; SELECT 'catalog', sequence_nr, projection_hash FROM entity_catalog WHERE tenant='default' AND entity_type='Session' AND entity_id='ss-019dbea5-925a-7872-aec0-0f0e4fc90a7d';"`
   Result:
   - only `result|live e2e complete via Monty tool` remained in `entity_field_index`
   - `entity_catalog` row advanced to `sequence_nr=15`

### Additional Notes

- This proves the Session still advanced normally while the three hot operational fields stayed out of the durable query plane.
- The live server for these checks was the OpenPaw worktree, with `os-apps/katagami-curation` symlinked to the base Katagami checkout.
