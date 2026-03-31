# Proof Report: 023 — Monty REPL for Agent Tool Execution

## Date
2026-03-31

## Branch / Commit
- OpenPaw: `feat/monty-repl` (PR #6) — `c5774318`
- Temper: `feat/host-get-time-millis` (PR #94) — `fe2cc0c`

## What Was Done
Replaced the monolithic 19-tool architecture with a single `execute` tool backed by Pydantic's Monty Python sandbox compiled to WASM. Added Cedar-governed self-provisioning via CapabilityRequest entity.

## Verification Flow

### V1: WASM Compilation
Verify all new and existing WASM modules compile without errors.

### V2: Monty-in-WASM
Verify Monty Python interpreter compiles to `wasm32-wasip1` without any fork or patches.

### V3: WASI Detection
Verify the monty_repl binary imports `wasi_snapshot_preview1` and the engine will detect it.

### V4: Host Function Availability
Verify both WASI syscalls and custom `env.*` host functions are present in the binary.

### V5: Single Execute Tool
Verify llm_caller produces exactly one tool definition (`execute`) with `code` parameter.

### V6: SDK Reference in System Prompt
Verify `build_sdk_reference()` generates the Temper SDK documentation block.

### V7: Existing Module Regression
Verify all pre-existing WASM modules still compile on wasm32-unknown-unknown.

### V8: Temper Engine Tests
Verify Temper WASM engine tests pass with WASI additions.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| V1: monty_repl build | Compiles to wasm32-wasip1 | `Finished release` (5.0MB binary) | PASS |
| V1: capability_installer build | Compiles to wasm32-unknown-unknown | `Finished release` (212KB binary) | PASS |
| V1: llm_caller check | Compiles (warnings only) | `Finished dev` (12 pre-existing warnings) | PASS |
| V2: Monty compilation | No fork needed, compiles directly | Monty crate compiles as dependency for wasip1 | PASS |
| V3: WASI imports | Binary contains wasi_snapshot_preview1 | `strings` confirms: clock_time_get, fd_write, proc_exit, environ_get, environ_sizes_get | PASS |
| V4: Host functions | Binary contains custom host_* imports | Confirmed: host_get_context, host_http_call, host_log, host_set_result | PASS |
| V5: Single tool | build_tool_definitions returns 1 tool | Returns `[{name: "execute", input_schema: {code: string}}]` | PASS |
| V6: SDK reference | build_sdk_reference generates temper_sdk block | Function exists, conditionally includes entity/sandbox/platform/evolution sections | PASS |
| V7: tool_runner regression | Compiles unchanged | `Finished dev` | PASS |
| V7: sandbox_provisioner regression | Compiles unchanged | `Finished dev` | PASS |
| V7: agent_reply regression | Compiles unchanged | `Finished dev` | PASS |
| V7: steering_checker regression | Compiles unchanged | `Finished dev` | PASS |
| V7: context_compactor regression | Compiles unchanged | `Finished dev` | PASS |
| V7: request_approval regression | Compiles unchanged | `Finished dev` | PASS |
| V8: Temper engine tests | 4/4 pass | 4 passed, 0 failed | PASS |

## What Worked
- Monty compiles to wasm32-wasip1 without any patches — WASI provides std::time::Instant, getrandom, and LazyLock natively
- The monty-js crate's existence proved the path was viable before we started
- Adding WASI to the Temper engine was additive — existing non-WASI modules are completely unaffected
- The dispatch layer mirrors temper-sandbox/dispatch.rs patterns cleanly
- MontyRun event loop (Complete, FunctionCall, ResolveFutures, NameLookup, OsCall) maps directly to synchronous WASM execution

## What Didn't Work
- Initial attempt to compile Monty for wasm32-unknown-unknown failed: getrandom, Instant, and LazyLock all need OS support. Adding WASI was the correct solution.
- Two duplicate tool definitions (save_memory, spawn_agent) existed in the old build_tool_definitions — confirmed as pre-existing technical debt

## Limitations
- **E2E runtime testing not yet performed**: Requires Temper PR #94 merged and daemon running with WASI engine. Compilation verification proves structural correctness; runtime dispatch needs integration testing.
- **REPL state persistence (dump/load)**: The MontyRepl dump/load serialization is not yet wired into entity state. The monty_repl module uses stateless MontyRun per invocation. Full persistence requires MontyRepl integration (future PR).
- **Session tree integration**: The monty_repl module returns tool results but doesn't yet persist to TemperFS session tree. Needs porting from tool_runner's session logic.
- **Hooks**: Before/after tool hooks are not yet implemented in monty_repl. The dispatch layer executes directly without hook evaluation.

## What Still Doesn't Work
1. Runtime E2E: Temper WASI support must be merged (PR #94) before daemon can load monty_repl
2. REPL persistence: Variables don't persist across LLM turns yet (MontyRepl vs MontyRun)
3. Session tree: Tool results not written to TemperFS
4. Hooks: Before/after interceptors not ported
5. File sync: Sandbox file manifest sync not ported from tool_runner

## Artifacts
- ADR: `docs/adrs/0008-monty-repl-agent-tools.md`
- Spec: `os-apps/paw-agent/specs/capability_request.ioa.toml`
- WASM: `os-apps/paw-agent/wasm/monty_repl/` (4 files)
- WASM: `os-apps/paw-agent/wasm/capability_installer/` (2 files)
- Modified: `os-apps/paw-agent/wasm/llm_caller/src/lib.rs`
- Modified: `os-apps/paw-agent/specs/agent.ioa.toml`
- Temper: `crates/temper-wasm/src/engine/{mod.rs, host_functions.rs}`
- Temper: `crates/temper-wasm-sdk/src/{host.rs, context.rs}`

## Architecture Diagram
```text
┌─────────────────────────────────────────────────────────────────────┐
│ Temper Server                                                        │
│                                                                      │
│  LLM ──▶ execute({code: "..."}) ──▶ WASM [Monty Python REPL]       │
│                                       │  (wasm32-wasip1, 5MB)       │
│                                       │                              │
│        Python interpreter runs ──────▶│  temper.create()  ──local──▶ Temper API
│        in restricted sandbox          │  temper.action()  ──local──▶ /tdata/...
│                                       │  temper.submit_specs() ──▶  /api/specs/
│                                       │  temper.get_trajectories()──▶ /api/evolution/
│                                       │                              │
│                                       │  sandbox.bash() ──HTTP──▶ Tensorlake
│                                       │  sandbox.read() ──HTTP──▶ Tensorlake
│                                       │                              │
│  WASI provides: clock, random         │                              │
│  Custom host: http_call, log,         │                              │
│    get_context, set_result            │                              │
│                                       │                              │
│  CapabilityRequest entity ──Cedar──▶ capability_installer WASM      │
│  (self-provisioning)           ──▶ POST /api/apps/install           │
│                                ──▶ POST /api/specs/load-inline      │
│                                ──▶ POST /api/wasm/modules/<name>    │
└─────────────────────────────────────────────────────────────────────┘
```

## Runtime E2E Addendum (2026-03-31 17:37 UTC)

### Daemon Startup Verification

The OpenPaw daemon started successfully with all modules:

| Module | Status | Details |
|--------|--------|---------|
| monty_repl | REGISTERED | hash=61d8bd7c, path=wasm32-wasip1/release, 5428KB |
| capability_installer | REGISTERED | hash=8ffbb135, 216KB |
| CapabilityRequest entity | ADDED | New entity type added to schema |
| Agent entity | UPDATED | repl_state field + monty_repl integration |
| All 15 other WASM modules | REGISTERED | Unchanged, zero regressions |

### Key Confirmations

1. **WASI module detection works**: `find_wasm_binary()` found monty_repl at `wasm32-wasip1/release/`
2. **WASI compilation in engine**: `wasmtime-wasi` compiled and linked (`Compiling wasmtime-wasi v29.0.1`)
3. **CapabilityRequest schema installed**: Shows as `added=["CapabilityRequest"]` in install log
4. **Agent entity updated**: `updated=["Agent", "ToolHook"]` confirms repl_state field and integration change
5. **Zero startup errors**: Daemon booted through all 9 phases successfully
