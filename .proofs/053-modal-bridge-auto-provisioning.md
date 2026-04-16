## Summary

This change removes `modal_bridge_url` from the human-facing setup path and makes OpenPaw deployment provision it automatically when Modal credentials are available locally.

The platform now:

- reads Modal credentials from environment variables or `~/.modal.toml`
- deploys the Modal bridge owned by OpenPaw
- infers the bridge base URL from Modal deploy output
- writes `SANDBOX_PROVIDER=modal`, `MODAL_TOKEN_ID`, `MODAL_TOKEN_SECRET`, and `MODAL_BRIDGE_URL` into Railway
- seeds `modal_bridge_url` into Temper vault through existing startup secret mirroring
- passes `modal_bridge_url` into session sandbox integrations
- keeps `modal_bridge_url` as an internal-only secret instead of exposing it in dashboard setup

## Files Changed

- `crates/openpaw-cli/src/deploy.rs`
- `crates/openpaw/src/setup_api.rs`
- `crates/openpaw/src/startup.rs`
- `crates/openpaw/tests/session_turn_architecture.rs`
- `os-apps/paw-agent/specs/session.ioa.toml`
- `os-apps/paw-agent/wasm/wasm-helpers/src/sandbox.rs`

## Key Behavior Changes

### Deploy

`openpaw deploy` now attempts to configure Modal automatically:

1. ensure the `modal` CLI exists
2. read credentials from `MODAL_TOKEN_ID` / `MODAL_TOKEN_SECRET` or `~/.modal.toml`
3. create/update the Modal secret `openpaw-bridge-auth`
4. deploy `os-apps/paw-agent/modal-bridge/modal_bridge.py`
5. infer the base URL from Modal output
6. store the resulting values in Railway env vars

If Modal credentials are not present locally, deploy logs a warning and continues instead of pretending the user should know an internal bridge URL.

### Dashboard / Setup API

`modal_bridge_url` is no longer advertised in the setup schema, so the dashboard should stop asking humans to provide it manually.

### Runtime

Session sandbox integrations now receive `modal_bridge_url` from Temper vault, so once deploy populates Railway and startup mirrors secrets into the vault, the agent can provision Modal sandboxes without a manual bridge setting.

## Verification

### Targeted Tests

Passed:

```bash
cargo test -p openpaw-cli deploy::tests --manifest-path /private/tmp/openpaw-modal-bridge-auto/Cargo.toml -- --nocapture
```

```bash
cargo test -p openpaw --manifest-path /private/tmp/openpaw-modal-bridge-auto/Cargo.toml modal_bridge_url_remains_internal_only -- --nocapture
```

```bash
cargo test -p openpaw --manifest-path /private/tmp/openpaw-modal-bridge-auto/Cargo.toml --test session_turn_architecture session_spec_passes_modal_bridge_url_to_modal_integrations -- --nocapture
```

### Real Modal Verification

I validated the Modal-side assumptions against the real provider:

- active Modal profile: `n-seshendra`
- deployed bridge app name: `openpaw-sandbox-bridge`
- inferred bridge base URL:

```text
https://n-seshendra--openpaw-sandbox-bridge
```

This confirms the deploy-time URL inference matches the actual OpenPaw bridge deployment shape.

### Local Runtime Smoke

I also started a fresh local boot with `OPENPAW_WASM_STARTUP_POLICY=build` and observed:

- startup selected `BuildIfMissing`
- the startup app surface included `paw-research`
- the process took the expected missing-WASM bootstrap path instead of failing immediately

That smoke check was useful for the startup path, but the decisive verification for this change is the combination of:

- green targeted tests
- real Modal bridge deployment and URL inference
- session spec coverage for vault-fed `modal_bridge_url`

## Outcome

Humans should no longer need to know what `modal_bridge_url` is.

The platform now owns that concern:

- deploy computes it
- Railway stores it
- startup mirrors it into Temper vault
- sessions consume it from the vault
- the dashboard no longer asks for it
