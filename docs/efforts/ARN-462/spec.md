# ARN-462 — Image-break spec

## Contract

An os-app crate that depends on `rsa` and is built for
`wasm32-unknown-unknown` must compile on that triple. It does so by
declaring, on that target only:

```toml
getrandom = { version = "0.2", default-features = false, features = ["custom"] }
```

and registering a backend that returns `getrandom::Error::UNSUPPORTED`.
JWT sign does not draw randomness. The backend exists so the crate
compiles.

## Forbidden

- getrandom feature `js` (wasm-bindgen; host instantiation fails).
- Putting `getrandom` on `wasm-helpers` (ARN-443: every consumer then
  fails on unknown-unknown).
- Firing `TemperDeploy.Request` before GHCR has the tag.

## Out of scope

Migrating patrol modules from `wasm32-unknown-unknown` to `wasm32-wasip1`
is ARN-447. This spec does not do that.

## Proof

1. `scripts/check-wasm-rsa-getrandom.sh` exits 0.
2. `cargo check --manifest-path os-apps/paw-patrol/wasm/chain_github_ready/Cargo.toml --target wasm32-unknown-unknown` succeeds.
3. The same check for `release_run_lifecycle` still succeeds.
4. Host tests for `chain_github_ready` still pass.
