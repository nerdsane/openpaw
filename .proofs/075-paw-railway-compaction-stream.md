# Proof Report: 075 — Paw Railway compaction stream contract

## Date
2026-05-11 / 2026-05-12 UTC

## Branch / Commit
- Branch: `codex/paw-railway-compaction`
- Base: `origin/main` at `cca3581a`

## What Was Done
Fixed the `context_compactor` request body for the ChatGPT Codex Responses backend.

Production symptom:

```text
Compaction LLM call failed (HTTP 400): {"detail":"Stream must be set to true"}
```

Root cause: `context_compactor` already used the Codex SSE header contract (`accept: text/event-stream`) and SSE parser, but the `openai_codex` request body omitted `stream: true`. The provider caller already sends `stream: true`; the compactor had drifted from that contract.

Change:
- `openai` compaction requests remain non-streaming JSON with `store: false`.
- `openai_codex` compaction requests now include `stream: true` and `store: false`.
- Added regression test `codex_compaction_body_requests_streaming_response`.

ADR judgement: this is a narrow provider wire-format bug fix, not a material architecture change. No ADR was added.

## Verification Flow
1. Red: added `codex_compaction_body_requests_streaming_response` before implementation.
2. Green: added `stream: true` only to `openai_codex` compaction request bodies.
3. Built the deployable `context_compactor.wasm`.
4. Ran full `temperpaw` tests.
5. Built all app-required WASM modules needed for a cold local boot.
6. Booted an isolated local TemperPaw server.
7. Created a mock-provider `Session` through OData with an intentionally oversized reserve to force compaction.
8. Queried the `Session` OData state transitions and cancelled the synthetic session after proving `CompactionComplete`.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Red regression test | Fails before implementation | Failed with `left: Null`, `right: Bool(true)` | PASS |
| Focused green test | Regression passes | `1 passed` | PASS |
| Compactor suite | Existing and new tests pass | `13 passed` | PASS |
| WASM release build | Deployable compactor builds | `context_compactor.wasm` built for `wasm32-unknown-unknown` | PASS |
| Rust workspace build | Full workspace compiles | `cargo build --workspace` passed | PASS |
| `temperpaw` tests | Server package tests pass | `141 passed` across package/integration tests | PASS |
| Required OS-app WASM bundle | Local cold boot has required modules | Required startup modules built, including `context_compactor` | PASS |
| Local boot | Server reaches readiness | `/healthz=200`, `/readyz=200` on port `63419` | PASS |
| OData compaction flow | Session traverses compaction actions | Observed `NeedsCompaction -> CompactionAuthReady -> CompactionComplete` | PASS |

## Evidence
Key commands:

```sh
cargo test --manifest-path os-apps/paw-agent/wasm/context_compactor/Cargo.toml codex_compaction_body_requests_streaming_response -- --nocapture
cargo test --manifest-path os-apps/paw-agent/wasm/context_compactor/Cargo.toml
cargo build --manifest-path os-apps/paw-agent/wasm/context_compactor/Cargo.toml --target wasm32-unknown-unknown --release
cargo build --workspace
cargo test -p temperpaw
```

Local OData evidence:

```text
Session: compaction-stream-regression-1778547104
Status after proof: Cancelled
Compaction actions observed:
  NeedsCompaction
  CompactionAuthReady
  CompactionComplete
```

The forced local session used `provider=mock` so it did not call an external LLM. The exact Codex HTTP contract that caused the Railway 400 is covered by the red/green request-body regression test.

## What Worked
- The regression test reproduces the missing `stream` field directly.
- The local server loaded the rebuilt `context_compactor` WASM and completed compaction state transitions through the Temper action pipeline.

## What Didn't Work
- A first local server boot failed readiness because the fresh worktree lacked several app-required WASM artifacts. Building the OS-app WASM bundle resolved readiness.
- The synthetic forced-compaction session repeatedly compacted because the reserve was intentionally larger than the model window. It was cancelled after proving `CompactionComplete`.

## Limitations
- No live ChatGPT Codex request was made locally because no Codex OAuth token is present in the local environment.
- Railway deployment evidence is recorded in the rollout notes/final response after the merged commit is deployed.

## Artifacts
- `os-apps/paw-agent/wasm/context_compactor/src/lib.rs`
- Local evidence files under `/tmp/temperpaw-local-*.json`

## Architecture Diagram
```text
Session.PreparingContext
        |
        v
NeedsCompaction
        |
        v
CompactionAuthReady
        |
        v
context_compactor
        |
        +-- openai       -> JSON body, store=false
        |
        +-- openai_codex -> SSE body, stream=true, store=false
        |
        v
CompactionComplete
```
