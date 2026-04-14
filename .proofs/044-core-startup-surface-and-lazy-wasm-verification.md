# 044 Core Startup Surface And Lazy WASM Verification

## Goal

Reduce the warm-start baseline footprint by:

- shrinking OpenPaw's default startup install surface to true core apps only
- removing eager startup compilation for bundled and persisted WASM modules
- keeping actor residency bounded under load and confirming passivation after idle
- proving the resulting behavior end to end with local measurements and Datadog

## Code Changes

### OpenPaw

- `crates/openpaw/src/startup.rs`
  - startup OS apps now come from `temper_platform::os_apps::list_startup_os_apps()`
  - missing WASM build checks only scan startup-core apps
  - added `startup_os_apps_only_include_core_apps`

- `os-apps/paw-agent/app.toml`
- `os-apps/paw-channels/app.toml`
- `os-apps/paw-fs/app.toml`
  - added `startup_install = "core"`
  - added or updated `[[wasm_modules]]` contracts with `startup_loading = "lazy"`

### Temper

- `crates/temper-platform/src/os_apps/mod.rs`
  - added manifest support for `startup_install`
  - added WASM module contract metadata:
    - `startup_loading`
    - `criticality`
    - `target`
    - `provenance`
    - `import_class`
  - default install only pulls apps with `startup_install = "core"`
  - bundled WASM modules are persisted and registered at install time
  - eager compile only happens for modules explicitly marked `startup_loading = "eager"`

- `crates/temper-server/src/state/persistence/mod.rs`
  - persisted WASM registry restoration no longer eagerly compiles every module at startup
  - added `ensure_wasm_module_cached(...)` for first-use lazy compilation

- `crates/temper-server/src/state/dispatch/wasm.rs`
  - direct invoke and integration dispatch now ensure the target module is compiled on demand before execution

## Targeted Red-Green Tests

### OpenPaw

- `cargo test -p openpaw startup_os_apps_only_include_core_apps -- --nocapture`
- `cargo test -p openpaw startup::tests:: -- --nocapture`

### Temper

- `cargo test -p temper-platform --lib test_manifest_parses_startup_install_and_wasm_loading_policy -- --nocapture`
- `cargo test -p temper-platform --lib test_load_app_bundle_carries_wasm_module_contracts -- --nocapture`
- `cargo test -p temper-platform --lib -- --nocapture`
- `cargo test -p temper-server --test wasm_dispatch persisted_wasm_modules_are_lazy_compiled_on_first_invoke -- --nocapture`
- `cargo test -p temper-server --test wasm_dispatch -- --nocapture`

## Broad Verification

### OpenPaw Workspace

- `cargo test --workspace -- --nocapture`
- Result: passed

### Temper Workspace

- `cargo test --workspace -- --nocapture`
- Result during this report:
  - large suite chunks already green, including:
    - `temper-platform` library suite
    - `temper-server` library suite
    - `wasm_dispatch`
    - DST lifecycle and multi-tenant suites
  - the long-running workspace sweep was still draining through remaining DST cases while this proof was written

## End-To-End Setup

### Clean Environment

- killed all stray local `openpaw` processes before the run
- built a single release binary:
  - `cargo build -p openpaw --release`

### Investigation Server

- `HOME=/tmp/openpaw-core-startup.QaIQ4H`
- `PORT=61121`
- `OTEL_ENABLED=true`
- `DD_ENV=isolated-core-startup`
- `OPENPAW_WASM_STARTUP_POLICY=load-only`
- `TEMPER_RUNTIME_METRICS_INTERVAL_SECS=2`
- `TEMPER_ACTOR_IDLE_TIMEOUT=20`
- `TEMPER_PASSIVATION_CHECK_INTERVAL=5`

Server process:

- `target/release/openpaw`
- listening on `61121`
- health check returned `200`

## Startup Surface Evidence

Installed apps in the isolated store:

- `paw-agent`
- `paw-channels`
- `paw-fs`

This confirms startup install was reduced to the core surface rather than the full default app catalog.

Persisted WASM modules in the isolated store:

- `26`

This count is still non-trivial because `paw-agent` bundles multiple tools and agents, but the important behavior change is that those modules are no longer all eagerly compiled at startup.

Largest persisted modules in the isolated store:

- `monty_repl`: `6711219` bytes
- `llm_caller`: `615736` bytes
- `route_message`: `376656` bytes
- `context_compactor`: `337760` bytes
- `steering_checker`: `325749` bytes
- `plan_review_feedback_handler`: `321825` bytes
- `request_approval`: `267740` bytes
- `request_plan_review`: `265033` bytes
- `session_recoverer`: `254951` bytes
- `workspace_fs`: `254899` bytes

Interpretation:

- the remaining startup surface is now dominated by the persisted app/tool corpus rather than hot actors
- `monty_repl` is by far the largest individual module
- the next reduction pass should focus on startup app curation and module criticality, not on actor residency

## Baseline Memory

Before running workload:

- RSS: `242128 KB`
- Physical footprint: `81.1 MB`

This is materially lower than the prior baseline recorded in [043-baseline-memory-root-cause-investigation.md](/Users/seshendranalla/Development/openpaw-codex/.proofs/043-baseline-memory-root-cause-investigation.md:1):

- previous release baseline RSS: about `634.08 MB`
- previous release physical footprint: about `261.1 MB`

## Workload 1: Fast Burst

File: `/tmp/openpaw-core-startup.QaIQ4H/investigation_run.json`

- target files: `1000`
- created files: `1000`
- workers: `24`
- workload duration: `3.23s`
- idle wait: `45s`
- errors: none

Outcome:

- local indexed corpus rose from about `83` to about `1083`
- actors drained back to `1`
- this run was intentionally bursty and too short for Datadog's `30s` bucket rollups to capture the full actor peak cleanly

## Workload 2: Sustained Load

File: `/tmp/openpaw-core-startup.QaIQ4H/sustained_run.json`

- target files: `600`
- created files: `600`
- workers: `8`
- per-item sleep: `0.5s`
- workload duration: `38.7s`
- idle wait: `45s`
- local peak active actors: `380`
- errors: none

### Local Runtime Samples

Key observations from `/observe/health` sampling every `2s`:

- actors rose with the hot working set:
  - `1`
  - `33`
  - `65`
  - `97`
  - `129`
  - `161`
  - `193`
  - `225`
  - `257`
  - `282`
  - `313`
  - `345`
  - `380` peak

- indexed entities rose with the created file corpus:
  - `1083`
  - `1115`
  - `1147`
  - `1179`
  - `1211`
  - `1243`
  - `1275`
  - `1307`
  - `1339`
  - `1364`
  - `1395`
  - `1427`
  - `1459`
  - `1491`
  - `1523`
  - `1555`
  - `1587`
  - `1619`
  - `1647`
  - `1675`
  - `1683`

- after workload completion and idle passivation:
  - actors dropped from `305`
  - to `233`
  - to `153`
  - to `73`
  - and then back to `1`

### Datadog Metrics

Window:

- `2026-04-14T12:58:20Z` to `2026-04-14T13:00:00Z`

Queries:

- `max:temper_active_actors{service:openpaw,env:isolated-core-startup}`
- `max:temper_indexed_entities{service:openpaw,env:isolated-core-startup,tenant:*}`
- `max:temper_projected_entities{service:openpaw,env:isolated-core-startup,tenant:*}`
- `max:process_resident_memory_bytes{service:openpaw,env:isolated-core-startup}`

Returned binned values:

- actors: `105, 361, 1`
- indexed entities: `1187, 1659, 1683`
- projected entities: `1162, 1626, 1682`
- RSS bytes: `274432000, 274628608, 274481152`

Interpretation:

- Datadog saw the actor spike during load
- Datadog saw actors collapse back to `1` after idle
- indexed and projected entity counts tracked the workload and converged closely
- RSS stayed flat around `274 MB` during the sustained run

### Datadog Logs

Window:

- `2026-04-14T12:58:20Z` to `2026-04-14T13:00:20Z`

Query:

- `service:openpaw env:isolated-core-startup "passivated idle actors"`

Observed passivation events:

- `2026-04-14T12:59:00Z`
- `2026-04-14T12:59:10Z`
- `2026-04-14T12:59:20Z`
- `2026-04-14T12:59:30Z`

This confirms actor residency is being actively reduced after the workload rather than remaining pinned.

## Post-Run Memory

After the sustained run and idle passivation:

- RSS: `268112 KB`
- Physical footprint: `101.6 MB`

Current `vmmap` snapshot from the same isolated process:

- RSS-equivalent line: about `619.6 MB resident`
- physical footprint: `101.6 MB`

Interpretation:

- raw RSS is larger than unique memory cost on macOS
- physical footprint stayed near `100 MB`
- the sustained workload added only a modest amount over the reduced startup floor

## Conclusion

The change worked.

- OpenPaw no longer starts with the full default app surface
- startup no longer eagerly compiles the full persisted WASM corpus
- actor residency is bounded by the hot working set and returns to idle after passivation
- indexed and projected entity counts track the real entity corpus rather than implying runaway in-memory actors
- the startup memory floor is materially lower than the previous release baseline

Most importantly, the remaining memory story is no longer "runaway actor hydration." The isolated sustained run shows that actors spike under real load, then drain back down, while memory stays comparatively flat. The remaining baseline footprint is now much smaller and is more consistent with core runtime + loaded app surface rather than an architectural blow-up.
