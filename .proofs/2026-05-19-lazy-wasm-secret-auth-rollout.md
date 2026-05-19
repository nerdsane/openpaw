# Lazy WASM Secret Authorization Rollout Proof

Date: 2026-05-19

## Scope

Roll Temper PR #260 into TemperPaw by pinning all Temper server crates, all
checked-in `temper-wasm-sdk` guest manifests, checked-in guest lockfiles, and
the Docker Katagami SDK rewrite pin to:

`e45e8396884e1b88c0aec7711df1eeb2d858cab0`

This commit carries PERF-030 / ADR-0107 in Temper: lazy per-guest WASM secret
authorization with the eager secret load narrowed to bootstrap-only host
secrets.

## ADR Judgement

No new TemperPaw ADR was added for this repo change. This rollout does not
change TemperPaw architecture, entity behavior, specs, policies, trigger
boundaries, or application orchestration. The architectural decision is
recorded in Temper ADR-0107; TemperPaw is only consuming the new Temper
runtime revision through its existing pinned-dependency rollout pattern.

## Red-Green Evidence

The existing rollout contract caught an incomplete update after only manifests
were changed:

- Failing check: `wasm_sdk_dependencies_pin_same_temper_observability_revision_as_server`
- Failure: `os-apps/paw-consilium/wasm/check_and_synthesize/Cargo.lock must resolve temper-wasm-sdk to the same Temper observability rev as the server`

The nested checked-in lockfiles were then updated through tracked files, after
which the contract passed.

## Local Validation

- `cargo fmt --all -- --check` passed.
- `git diff --check` passed.
- `cargo check -p temperpaw` passed.
- `cargo test -p temperpaw --test datadog_observability_contract` passed:
  32 passed, 0 failed.
- `bash build.sh` from `os-apps/paw-agent/wasm` passed and rebuilt all Paw
  agent WASM modules against the new `temper-wasm-sdk` revision, including
  `provider_caller`.

## Notes

The Paw agent WASM build emitted existing warnings in unrelated modules:

- `sandbox_provisioner`: unused import `SandboxConfig`.
- `monty_repl`: unused doc comment on a macro invocation.

These warnings pre-existed the pin rollout behavior and did not block the build.

## Remaining Proof

This is only the rollout proof. The performance claim remains pending until the
TemperPaw rollout is merged, deployed, and production before/after evidence is
captured with controlled client timings plus Datadog spans for the
`authz_secret_resolution` phase.
