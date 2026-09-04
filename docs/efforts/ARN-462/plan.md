# ARN-462 — Image-break plan

## What we are addressing

Docker cannot build main. The 455 kernel cannot reach production MCP
until a TemperPaw image exists.

## Expected end state

`chain_github_ready` compiles for `wasm32-unknown-unknown`. CI `checks`
refuse a new `rsa` crate that omits the getrandom `custom` stub.

## Steps

1. Reproduce: `cargo check --target wasm32-unknown-unknown` in
   `chain_github_ready` fails on getrandom 0.2.17. `chain_file_ready`
   does not pull getrandom.
2. Copy the `release_run_lifecycle` target-dep and
   `register_custom_getrandom!` stub onto `chain_github_ready`.
3. Add `scripts/check-wasm-rsa-getrandom.sh` and run it from CI `checks`
   plus `cargo check` of both rsa crates on `wasm32-unknown-unknown`.
4. Do not Request `arn-462-temper-deploy` until GHCR has the new tag.
