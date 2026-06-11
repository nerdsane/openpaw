# Katagami Policy Reconcile TemperPaw Bump

## Summary

Katagami palette synthesis was live-stuck because spawned palette workers were Cedar-denied when reading or writing `PaletteSystem` and `TasteRule` entities. The Katagami routing fix was already live; Datadog showed palette jobs and child sessions existed, but OS-app policy permits were not active in the live tenant.

Temper PR 301 fixed the kernel class of bug by making OS-app reconcile compare bundle policies against the live tenant policy cache. If unchanged bundle policies are missing from active memory, reconcile now runs the policy phase instead of skipping.

This TemperPaw change bumps all server Temper dependencies and packaged WASM SDK pins to the merged Temper fix commit:

`dc294d819bcb8ca54778bcbe5c3d8db0de6b115c`

## ADR Judgment

No TemperPaw ADR is required for this change. It is a dependency pin update to consume the already-reviewed Temper platform behavior; it does not add a new TemperPaw architecture path, entity model, trigger, policy, or WASM integration.

## Verification

- `cargo check -p temperpaw -p paw-codex-worker`
- `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
- `cargo test -p temperpaw -p paw-codex-worker`
- `cargo clippy -p temperpaw -p paw-codex-worker -- -D warnings`

All commands passed after updating the server and nested WASM SDK lockfile pins.

## Remaining Live Step

Merge and deploy this TemperPaw bump to Railway, then verify in Datadog that Katagami no longer emits `AuthorizationDenied` for `PaletteSystems` or `TasteRules` and that the stuck palette query produces `palette_system_ids`.
