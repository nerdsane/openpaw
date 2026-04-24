# ADR-0042: Inline Action Triggers Hard Cut

- Status: Accepted
- Date: 2026-04-24
- Deciders: OpenPaw maintainers
- Related:
  - ADR-0001: Open Paw architecture
  - ADR-0005: Temper-native orchestration
  - temper ADR-0046: unified action triggers
  - `os-apps/paw-fs/specs/file.ioa.toml`
  - `os-apps/paw-agent/specs/session.ioa.toml`
  - `.proofs/057-merged-main-action-triggers-live-e2e.md`

## Context

OpenPaw had already started migrating to inline `[[action.triggers]]`, but the branch still carried two sources of truth in the places that mattered most for merge safety:

- `paw-fs` had inline trigger declarations while the historical migration context still implied a legacy `reactions.toml` fallback.
- the file-version cascade exposed a subtle migration bug where source-field names were passed as literal trigger params instead of copied from source state.
- merge validation originally ran against branch-local code only, even though OpenPaw's real runtime contract depends on the paired Temper branch that implements ADR-0046.

That state violated the Temper-native rule from ADR-0005: a reader could not trust the entity specs alone to understand the orchestration contract.

## Decision

OpenPaw treats inline `[[action.triggers]]` as the only supported cross-entity and post-commit orchestration surface for shipped apps on this branch.

Concretely:

- shipped specs are authoritative; legacy `reactions.toml` fallback is removed rather than documented as a temporary escape hatch
- trigger values copied from source entity state use `[action.triggers.params_from]`; literal `[action.triggers.params]` stays reserved for actual literals
- merge readiness for trigger work requires live proof on a local OpenPaw server wired to the paired local Temper worktree, not only unit tests against published or remote dependencies
- proof-only local path overrides such as ignored Cargo patch config or temporary symlink redirection are allowed for verification but must not be committed

## Consequences

### Positive

- `paw-fs` lineage is described by the spec that actually ships.
- The runtime contract between OpenPaw and Temper is exercised before merge instead of being assumed.
- Future migrations have a clear rule for when to use `params_from` versus literal params.

### Negative

- Local end-to-end proof is heavier because it requires coordinating the paired Temper checkout.
- There is no compatibility mode left for apps that still depend on legacy reaction files.

### Risks

- If a future proof run accidentally commits local patch wiring or symlink overrides, the repo would become machine-specific. The proof process must keep those changes ignored and ephemeral.

## Readiness Gates

- `os-apps/paw-fs/specs/file.ioa.toml` is the single source of truth for file-version trigger wiring.
- `cargo test -p temperpaw` passes against the paired local Temper checkout.
- `.proofs/057-merged-main-action-triggers-live-e2e.md` records a live server run proving:
  - `paw-fs` version lineage works
  - Session execution completes
  - provider-specific OpenAI secret wiring resolves through the correct key path

## Non-Goals

- Reintroducing compatibility loading for legacy reaction files.
- Treating proof-only local dependency overrides as committed repository configuration.
