# Katagami Publish Path E2E Proof

- Date: 2026-06-18
- Linear: ARN-51, ARN-52, ARN-54, ARN-55, ARN-57
- TemperPaw PR: https://github.com/nerdsane/temperpaw/pull/415
- Genesis PR: https://github.com/arni-labs/genesis/pull/36

## Scope

This proof covers the Katagami curation artifact publishing path after fixing
the app publish/install path, Genesis receive-pack auth support, PawFS hotload
writes, the Ready File invariant, verification evidence, and installed app
provenance.

The final production verifier required more than a successful agent response:
the session had to create a `Files` entity in workspace `katagami` with
`Status=Ready`, and the bytes read from `/tdata/Files('{id}')/$value` had to
match the expected SHA-256.

## Issues Found

1. Genesis receive-pack could not accept an authenticated app publish when the
   pushed pack referenced a base object that existed only in the server object
   database. This blocked publishing the updated `temperpaw/paw-agent` app.
2. `context_preparer` still used broad `SessionEntry` collection reads in hot
   sessions, which hit `HTTP 413 QueryTooLarge` in production.
3. The first-turn virtual SessionEntry leaf was queried as if it already
   existed durably, which caused another bounded-query 413 on the virtual
   `u-{session_id}-0` leaf.
4. PawFS directory lookup for `/proofs` used an exact filtered collection read
   that also hit `HTTP 413 QueryTooLarge`; the agent reported done, but no Ready
   File existed.
5. The proof file and Linear state still described the old blocked state instead
   of the live installed app and Ready File evidence.
7. OpenPaw bootstrap refs could drift back to the older `paw-agent` app hash
   after restart unless the pinned hash was updated.

No duplicate Linear issues were created; updates were added to the existing
issues, primarily ARN-54 for the bounded-query PawFS hotload blocker.

## Fixes

### Genesis

- Commit: `4e48827 Fix receive-pack delta base fallback`
- PR: https://github.com/arni-labs/genesis/pull/36
- Deployment: `fa798fd2-a606-47ea-b1a8-b3ec0ac0b083`
- Image digest:
  `sha256:98e07c2ffb49b49000f798e53bf19ab12b98873e8cf7941ae955084620ad487c`
- Result: receive-pack accepts the authenticated publish path used for
  `temperpaw/paw-agent`.

### TemperPaw

- `66cd238a300cfe8601a29435097024dac620bb39`
  `Fix Genesis publish auth and bounded PawFS lookups`
- `b09358cf` `Bound SessionEntry context reads`
- `da31ed64` `Walk SessionEntry chains by leaf`
- `21fa21d4` `Skip OData for virtual first turn leaf`
- `97dff829` `Fallback PawFS directory creates on query budget`

Key behavior changes:

- `wasm-helpers` now supports bounded SessionEntry chain reads from a known
  `session_leaf_id`.
- The virtual first-turn leaf `u-{session_id}-0` is recognized without issuing
  an OData lookup.
- `context_preparer` passes the session leaf into the helper instead of
  falling back to a broad session read.
- PawFS directory creation now uses stable path-scoped entity ids and treats
  query-budget lookup failures as a signal to perform idempotent create.

## Local Verification

- Red/green:
  `cargo test -p temperpaw --test session_turn_architecture session_entry_readbacks_stay_within_bounded_query_budget -- --nocapture`
- Red/green:
  `cargo test virtual_first_turn_leaf_is_detected_without_odata_probe -- --nocapture`
- Red/green:
  `cargo test pawfs_stable_directory_ids_are_path_scoped -- --nocapture`
- Full focused tests:
  `cargo test -p temperpaw --test session_turn_architecture -- --nocapture`
  - Result: 24 passed.
- Full helper tests:
  `cargo test -- --nocapture`
  - Path: `os-apps/paw-agent/wasm/wasm-helpers`
  - Result: 39 passed.
- PawFS hot path tests:
  `cargo test -p temperpaw --test paw_fs_hot_path -- --nocapture`
  - Result: 15 passed.
- Monty REPL PawFS tests:
  `cargo test pawfs_ -- --nocapture`
  - Path: `os-apps/paw-agent/wasm/monty_repl`
  - Result: 5 passed.
- Formatting:
  `cargo fmt --check`
  - Result: passed.
- WASM build:
  `./build.sh`
  - Path: `os-apps/paw-agent/wasm`
  - Result: passed with pre-existing warnings in unrelated modules.

## Publish And Install Evidence

Final published `paw-agent` app hash:

- `temperpaw/paw-agent@18f58e340795e66015e428534679222eb2afaced`

Genesis readback:

- `LatestVersionHash=18f58e340795e66015e428534679222eb2afaced`
- `NewHash=18f58e340795e66015e428534679222eb2afaced`
- `RefName=main`
- `Status=Active`

Fresh Genesis clone evidence:

- `clone_head=18f58e340795e66015e428534679222eb2afaced`
- `git fsck --full`: ok
- tracked WASM count: 21
- PawFS stable directory guard present in cloned `wasm/monty_repl` source.

OpenPaw install evidence:

- Install API: `HTTP 200`
- Installed app ref:
  `temperpaw/paw-agent@18f58e340795e66015e428534679222eb2afaced`

Production installed app provenance:

- `paw-agent`
  - `source_kind=genesis`
  - `version_hash=18f58e340795e66015e428534679222eb2afaced`
  - `pinned_version_hash=18f58e340795e66015e428534679222eb2afaced`
  - `current_version_hash=18f58e340795e66015e428534679222eb2afaced`
  - `follow_policy=pinned`
  - `status=installed`
- `paw-fs`
  - `8cc9c1a0c3959ba0555a6eac5446db76de747817`
- `katagami-curation`
  - `1e6f43993be70ca3d7dadf42c032fa6a206ac482`

Bootstrap persistence:

- `TEMPERPAW_GENESIS_BOOTSTRAP_REFS` now points `temperpaw/paw-agent` at
  `18f58e340795e66015e428534679222eb2afaced`.

OpenPaw deployment:

- Deployment: `2545e028-efe2-4700-9e07-7072a73a0076`
- Status: `SUCCESS`
- `/healthz`: HTTP 200
- `/readyz`: HTTP 200

## Failed Live Attempts During Diagnosis

- `ss-hotload-e2e-20260618043245`
  - Failed in `context_preparer`.
  - Error: `SessionEntry list failed (HTTP 413): QueryTooLarge`.
- `ss-hotload-e2e-20260618045510`
  - Failed while reading the virtual first-turn leaf.
  - Error: `SessionEntry chain read failed for EntryId=u-...-0 (HTTP 413): QueryTooLarge`.
- `ss-hotload-e2e-20260618050218`
  - Session completed with `(done)`, but independent file verification failed.
  - SessionEntry forensic evidence showed `temper.write` failed on directory
    lookup:
    `GET /tdata/Directories?$filter=Path eq '/proofs' and WorkspaceId eq 'katagami' ...`
    returned `413 QueryTooLarge`.
  - Exact Ready File query returned zero rows.

## Final Live E2E

Production session:

- Session: `ss-hotload-e2e-20260618051545`
- Proof path: `/proofs/katagami-hotload-e2e-20260618051545.md`
- Workspace: `katagami`
- Runtime app hash:
  `temperpaw/paw-agent@18f58e340795e66015e428534679222eb2afaced`
- Session final status: `Completed`
- Session result: `(done)`

Independent Ready File verification:

- Ready file count: 1
- File id: `fl-019ed928-b94c-7953-adaa-37981496cad3`
- Expected SHA-256:
  `ce32628b7cd1c561ca7a23401a1e3998c1c0820f4b23150bfbafb41ec1033e70`
- Actual SHA-256:
  `ce32628b7cd1c561ca7a23401a1e3998c1c0820f4b23150bfbafb41ec1033e70`
- Result: `ready_file_content_match=true`

## ADR Note

No new ADR was added for the final two TemperPaw code changes because they are
bounded implementation corrections to already-accepted SessionEntry and PawFS
hot-path architecture decisions:

- bounded SessionEntry reads are covered by the existing SessionEntry hot-path
  ADR set;
- stable PawFS directory creation preserves the existing entity model and only
  makes the create path idempotent when the exact lookup exceeds query budget.

The judgement is recorded here and should also be linked from the PR notes.
