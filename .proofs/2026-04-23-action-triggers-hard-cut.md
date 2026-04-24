# Action Triggers Hard Cut Proof

Date: 2026-04-23

## Scope

This proof covers the merge-readiness cleanup for the ADR-0046 trigger
cutover across the local worktrees:

- Temper: `/Users/seshendranalla/Development/temper-action-triggers`
- OpenPaw: `/Users/seshendranalla/Development/openpaw-action-triggers`
- Katagami: `/Users/seshendranalla/Development/katagami-action-triggers`

Goals:

1. Trigger principals must be enforced through Cedar before target action
   dispatch.
2. Malformed `[[action.triggers]]` blocks must fail loudly instead of silently
   disappearing.
3. Real load/install paths must stop accepting legacy `reactions.toml`.
4. OpenPaw and Katagami must finish the migration by deleting legacy reaction
   sources and aligning policy/comments with inline triggers.

## What Changed

### Temper

- Added a fail-loud parse path for nested `[[action.triggers]]` TOML.
- Added an authz resource snapshot helper so action authorization uses the same
  Cedar resource view in both OData and trigger dispatch.
- Threaded the real `SecurityContext` through `AgentContext` when known, so
  trigger inheritance can reuse the exact caller context instead of a lossy
  reconstruction.
- Added a Cedar gate in trigger dispatch before target action execution.
- Hard-cut legacy `reactions.toml` from `load_dir` and OS app install/load
  paths.
- Added a regression test proving legacy app bundles with `reactions.toml` are
  rejected.

### OpenPaw

- Deleted `os-apps/paw-fs/reactions/reactions.toml`.
- Made `os-apps/paw-fs/specs/file.ioa.toml` explicitly authoritative for the
  File -> FileVersion / Workspace cascade.
- Added `os-apps/paw-fs/policies/file_version.cedar`.
- Added explicit `file-service` coverage for `Workspace.IncrementUsage`.
- Updated agent skill docs that still described `reactions.toml` as the
  cross-entity orchestration primitive.

### Katagami

- Deleted both legacy reaction files:
  - `katagami-curation/reactions/reactions.toml`
  - `katagami-curation/specs/reactions.toml`
- Removed the dual-source/fallback commentary from
  `katagami-curation/specs/curation_job.ioa.toml`.

## Verification

### Temper parser + runtime

Commands:

```sh
cargo test -p temper-spec action_triggers --manifest-path /Users/seshendranalla/Development/temper-action-triggers/Cargo.toml
cargo test -p temper-server inline_action_triggers_respect_tenant_cedar_denials --manifest-path /Users/seshendranalla/Development/temper-action-triggers/Cargo.toml
cargo test -p temper-server inline_action_triggers_fire_through_production_dispatcher --manifest-path /Users/seshendranalla/Development/temper-action-triggers/Cargo.toml
cargo test -p temper-platform test_load_app_bundle_rejects_legacy_reactions_file --manifest-path /Users/seshendranalla/Development/temper-action-triggers/Cargo.toml
```

Results:

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| `temper-spec` trigger parse suite | valid inline triggers parse; malformed nested trigger TOML fails loudly | 9 tests passed, including `test_action_triggers_invalid_nested_toml_fails_loud` | PASS |
| trigger deny path | denied trigger principal must not advance target entity | `inline_action_triggers_respect_tenant_cedar_denials` passed | PASS |
| trigger allow path | permitted trigger principal must dispatch target action | `inline_action_triggers_fire_through_production_dispatcher` passed | PASS |
| OS app hard cut | app bundles with legacy `reactions.toml` must be rejected | `test_load_app_bundle_rejects_legacy_reactions_file` passed | PASS |

### OpenPaw + Katagami migration assets

Commands:

```sh
rg -n "reactions\\.toml|dual-source|authoritative source" /Users/seshendranalla/Development/openpaw-action-triggers /Users/seshendranalla/Development/katagami-action-triggers --glob '!**/target/**'
find /Users/seshendranalla/Development/openpaw-action-triggers -path '*/reactions/reactions.toml' -o -path '*/specs/reactions.toml' | sort
find /Users/seshendranalla/Development/katagami-action-triggers -path '*/reactions/reactions.toml' -o -path '*/specs/reactions.toml' | sort
```

Results:

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| OpenPaw legacy reaction files | none remain | no `reactions.toml` files found | PASS |
| Katagami legacy reaction files | none remain | no `reactions.toml` files found | PASS |
| fallback commentary | migration comments no longer claim dual-source authority | no remaining matches for the old fallback wording in migrated specs | PASS |

### OpenPaw workspace smoke check

Commands:

```sh
cargo build -p temperpaw --manifest-path /Users/seshendranalla/Development/openpaw-action-triggers/Cargo.toml
/Users/seshendranalla/Development/openpaw-action-triggers/target/debug/temperpaw-server doctor
```

Results:

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| OpenPaw workspace build | repo still compiles after deleting legacy reaction files and adding FS policy changes | `cargo build -p temperpaw` passed | PASS |
| OpenPaw startup probe | determine whether a non-interactive boot proof is possible in this environment | `temperpaw-server` now requires interactive `run` setup; `doctor` confirmed missing local setup state rather than spec-load failure | PARTIAL |

## What Worked

- The original two blocking findings are now covered by tests and fixed in the
  `temper` runtime.
- The install/load path no longer silently tolerates legacy reaction files.
- The dependent repos are structurally migrated to inline triggers only.

## Limitations

- I did not complete a full live OpenPaw or Katagami runtime replay of the
  migrated cascades in this turn.
- While attempting a live OpenPaw build, it became clear that the OpenPaw
  workspace still depends on remote `temper` `main` in Cargo, not this local
  `temper-action-triggers` worktree. That means a live OpenPaw boot today would
  not be a faithful proof of the new trigger runtime until the Temper branch is
  merged or the dependency is temporarily pointed at it.
- The current `temperpaw-server run` flow is interactive on a fresh local home,
  so a hands-free boot proof needs either seeded setup state or an automated
  setup path.

## Recommended Next Runtime Proof After Merge Wiring

1. Point OpenPaw and Katagami at the merged Temper runtime (or temporarily at
   the local worktree).
2. Boot OpenPaw and exercise `File.StreamUpdated`, confirming:
   - `FileVersion.Create`
   - `FileVersion.Supersede`
   - `Workspace.IncrementUsage`
   all run under `file-service` Cedar authorization.
3. Boot Katagami and exercise `CurationJob.Complete` / `Fail`, confirming the
   parent `CurationQuery` advances only through inline triggers.

## Artifacts

- Temper branch changes in:
  - `crates/temper-spec`
  - `crates/temper-server`
  - `crates/temper-platform`
- OpenPaw migration changes in:
  - `os-apps/paw-fs`
  - `os-apps/paw-agent/system/skills`
- Katagami migration changes in:
  - `katagami-curation/specs`
