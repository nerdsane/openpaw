# Proof Report: 037 — Invocation Context Read Fix

## Date
2026-04-11

## Branch / Commit
- `openpaw-codex`: `main` @ `7a46921d`
- `temper`: `feat/governance-decision-callbacks` @ `c85919a`

## What Was Done
- Fixed `temper-wasm-sdk::Context::from_host()` to support invocation contexts larger than the static host buffer by using a two-pass read.
- Added a real end-to-end `temper-wasm` test that exercises the SDK-backed guest path with an oversized invocation context.
- Patched `openpaw-codex` to resolve `temper-wasm-sdk` from the local Temper checkout even for standalone `os-apps/*/wasm/*` Cargo projects.
- Rebuilt the affected OpenPaw WASM modules and re-ran a live oversized-session repro against a local server.

## Verification Flow
1. Wrote an e2e regression test in `temper-wasm` for an oversized invocation context read through `temper-wasm-sdk::Context::from_host()`.
2. Confirmed the new test failed before the SDK fix.
3. Implemented the two-pass SDK context read and rebuilt the fixture WASM.
4. Re-ran the `temper-wasm` test suite and the `temper-wasm-sdk` crate tests.
5. Reproduced the issue live in OpenPaw with a deliberately oversized session system prompt while only the Temper fix was present; the session still failed with `failed to read invocation context`.
6. Added `.cargo/config.toml` patching so standalone OpenPaw WASM crates resolve `temper-wasm-sdk` from `../temper`.
7. Rebuilt the relevant OpenPaw WASM modules, restarted OpenPaw, and re-ran the same oversized-session probe.
8. Verified the session progressed through `WorkspaceReady` and `RecordResult` and ended in `Completed` without the invocation-context failure.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| `cargo test -p temper-wasm invoke_sdk_module_with_large_context_succeeds -- --nocapture` before fix | New e2e test fails on oversized SDK context read | Failed before SDK patch | PASS |
| `cargo test -p temper-wasm invoke_sdk_module_with_large_context_succeeds -- --nocapture` after fix | New e2e test passes | Passed after SDK patch and fixture rebuild | PASS |
| `cargo test -p temper-wasm --test e2e_invoke -- --nocapture` | All invoke e2e tests pass | 5 tests passed | PASS |
| `cargo test -p temper-wasm-sdk` | SDK crate tests pass | Passed | PASS |
| Live OpenPaw oversized-session probe before local SDK patch wiring | Session still fails if standalone WASM crates do not consume the patched SDK | Failed with `failed to read invocation context` | PASS |
| `cargo tree -i temper-wasm-sdk` in `os-apps/paw-agent/wasm/workspace_provisioner` after `.cargo/config.toml` patch | Standalone WASM crate resolves local SDK | Resolved to `/Users/seshendranalla/Development/temper/crates/temper-wasm-sdk` | PASS |
| Live OpenPaw oversized-session probe after rebuild | Session survives oversized invocation context | Session `ss-019d7ec2-564c-7b12-85ea-ab1055da282a` reached `Completed` | PASS |

## What Worked
- The real fault was not a transient TemperFS lock failure; it was the guest SDK rejecting invocation contexts larger than the fixed host buffer.
- Host-side WASM ABI support was already correct; the missing piece was the shared SDK using the returned required size.
- A local Cargo patch in `openpaw-codex/.cargo/config.toml` was necessary because standalone WASM crates under `os-apps/*/wasm/*` do not inherit the root workspace patch table.
- A deliberately oversized system prompt was an effective deterministic repro for the failure mode.

## What Didn't Work
- Fixing only the Temper repo was not enough for the live OpenPaw repro because the standalone OpenPaw WASM crates were still pulling `temper-wasm-sdk` from Git.
- Session token counters in the `Session` entity still look suspicious because they increment by `1` per turn instead of accumulating token values. That is separate from this crash and did not block the fix.

## Limitations
- The live verification used the mock LLM provider with an intentionally oversized prompt to force the invocation context over the host buffer size. This proves the crash class and the fix, but it does not simulate every real-world turn sequence.
- The proof relies on a local sibling Temper checkout at `../temper`; that is already how this repo patches other Temper crates.

## What Still Doesn't Work
- `Session.input_tokens`, `Session.output_tokens`, and `Session.context_tokens` remain inaccurate observability counters and should be fixed separately.

## Artifacts
- Temper SDK fix:
  - `/Users/seshendranalla/Development/temper/crates/temper-wasm-sdk/src/context.rs`
- Temper e2e regression:
  - `/Users/seshendranalla/Development/temper/crates/temper-wasm/tests/e2e_invoke.rs`
  - `/Users/seshendranalla/Development/temper/crates/temper-wasm/tests/fixtures/sdk-context-reader-src/`
  - `/Users/seshendranalla/Development/temper/crates/temper-wasm/tests/fixtures/sdk_context_reader.wasm`
- OpenPaw local SDK patching:
  - `/Users/seshendranalla/Development/openpaw-codex/Cargo.toml`
  - `/Users/seshendranalla/Development/openpaw-codex/.cargo/config.toml`
- Live verification log:
  - `/tmp/openpaw_invocation_context_fix_server.log`
- Loaded OpenPaw module hashes in the successful run:
  - `workspace_provisioner`: `1bfee3aed799ca40f66d78a928abb4671a136b66bf8277a98d22793f2e43abda`
  - `llm_caller`: `69c1e44024dea9adb36ca20b6f0a2c4b936ead8b203e5af354ebd9e4bee0c11d`
- Successful live session:
  - `ss-019d7ec2-564c-7b12-85ea-ab1055da282a`

## Architecture Diagram
```text
OpenPaw Session action
        |
        v
standalone WASM module in os-apps/paw-agent/wasm/*
        |
        v
temper-wasm-sdk::Context::from_host()
        |
        v
host_get_context(ptr, len)
        |
        +--> if len <= static buffer: parse directly
        |
        +--> if len > static buffer: allocate exact-size Vec, read again, parse
```
