# PawFS direct-key write hotfix proof

Date: 2026-06-18

## Scope

Live Katagami regeneration reached `temper.write`, then failed on PawFS directory
resolution because Monty looked up directories with a broad OData collection
filter:

`/tdata/Directories?$filter=Path eq ... and WorkspaceId eq ...`

Production rejected that shape with `413 QueryTooLarge` once the directory table
was large enough. This patch keeps the existing PawFS entity model and changes
Monty write resolution to derive deterministic directory/file IDs from
`(workspace_id, normalized_path)`, then read by key before creating missing
entities.

## ADR judgement

No new ADR. This is a scoped implementation repair under the existing PawFS
direct-access model, not a new entity type, workflow, storage model, trigger,
policy boundary, or agent capability surface.

## Red

Added `monty_pawfs_write_path_uses_direct_keys_not_broad_path_filters` in
`crates/temperpaw/tests/paw_fs_hot_path.rs`.

Initial result before implementation:

```text
cargo test -p temperpaw --test paw_fs_hot_path monty_pawfs_write_path_uses_direct_keys_not_broad_path_filters
test monty_pawfs_write_path_uses_direct_keys_not_broad_path_filters ... FAILED
Monty PawFS writes should derive deterministic directory ids
```

## Green

Local verification after implementation:

```text
cargo test -p temperpaw --test paw_fs_hot_path
test result: ok. 14 passed; 0 failed

cargo test --manifest-path os-apps/paw-agent/wasm/monty_repl/Cargo.toml
test result: ok. 73 passed; 0 failed

./os-apps/paw-agent/wasm/build.sh
All WASM modules built, including rebuilt monty_repl.
```

## Live E2E

Pending. Next step is to publish the new `temperpaw/paw-agent` Genesis ref,
update/reinstall OpenPaw production, and rerun the live Katagami regeneration
job that reproduced the PawFS write-path 413.
