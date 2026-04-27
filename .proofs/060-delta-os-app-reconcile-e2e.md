# Delta OS-App Reconcile E2E Proof

Date: 2026-04-27

Worktree: `/Users/seshendranalla/Development/openpaw-worktrees/os-app-delta-reconcile`

## Red Test

Added `startup_skips_builtin_default_agent_specs_when_paw_agent_owns_them`
before implementation.

Initial failure:

- `default_agent_specs_bootstrap_needed` did not exist, so startup had no
  decision point for letting the `paw-agent` OS app own the default agent specs.

## Verification Commands

```bash
cargo test -p temperpaw startup_skips_builtin_default_agent_specs_when_paw_agent_owns_them
```

Result: passed. `1` focused startup test passed.

```bash
cargo test -p temperpaw startup::tests
```

Result: passed. `26` startup tests passed.

## Live Local E2E

Ran a real `temperpaw-server` from a disposable OpenPaw worktree patched to use
the local Temper delta-reconcile crates. The run used a disposable home and a
file-backed Turso DB.

Cold boot:

- env included `TEMPERPAW_WASM_STARTUP_POLICY=build`
- `/healthz`: HTTP `200`
- `/readyz`: HTTP `200`
- log confirmed: `Skipping built-in default agent specs bootstrap; paw-agent OS app owns default agent specs`
- `phase_6b_os_app_reconcile`: `11,813ms`
- startup time to ready: `12,227ms`
- specs table: `default` `32` committed specs, min/max version `1/1`;
  `temper-system` `13` committed specs, min/max version `1/1`
- WASM table/blob store: `31` modules, `31` metadata-only SQL rows,
  `31` `wasm-modules/*` blobs, `15,272,520` blob bytes, min/max module
  metadata version `1/1`

Warm boot with the same DB:

- env included `TEMPERPAW_WASM_STARTUP_POLICY=load-only`
- `/healthz`: HTTP `200`
- `/readyz`: HTTP `200`
- all six startup apps logged `Skipped unchanged OS app`
- `phase_6b_os_app_reconcile`: `1,267ms`
- startup time to ready: `1,862ms`
- specs table stayed at max version `1`
- WASM metadata stayed at max version `1`

## Interpretation

The local warm boot now performs readiness recovery and digest checks, but does
not reinstall apps, rewrite WASM metadata, or churn `Agent`/`Plan` specs.
