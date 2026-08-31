# Decisions - land the paw-compute app (ARN-443 part A)

## Reconcile into the branch, not from Genesis — the source already matches
- **Decision:** Land the branch as-is (no content pulled from Genesis), after
  verifying a file-by-file match against Genesis HEAD.
- **Came up because:** the reconcile rule says Genesis wins on divergence, so I had
  to check whether Genesis had newer content than the branch.
- **Options:** pull Genesis content into the branch / land the branch after
  proving equality.
- **Chose land-after-proving because:** every text file in the branch's
  os-apps/paw-compute/ is identical to Genesis HEAD (73646d4); Genesis's only newer
  content is the compiled .wasm blob, which temperpaw gitignores by design and
  rebuilds at publish. Nothing to pull. Given up: nothing.
- **Where:** diff of Genesis `temperpaw/paw-compute` HEAD vs the branch tree;
  recorded in spec.md.

## Align computer_exec's SDK rev to main (43f9379)
- **Decision:** Bump computer_exec's temper-wasm-sdk rev from a747f7d to 43f9379.
- **Came up because:** after rebasing on post-#468 main, computer_exec failed to
  build — it pinned a different SDK rev than wasm-helpers, so the shared path dep
  produced two incompatible `Context` types (E0308).
- **Options:** pin wasm-helpers back to a747f7d (wrong — main is on 43f9379) / bump
  computer_exec to 43f9379.
- **Chose bump computer_exec because:** all of main (wasm-helpers, every os-app
  module) is on 43f9379; a module can't bridge two SDK revs through a shared path
  dep. This matches what Genesis's own ARN-401 rebuild (73646d4) targeted. Given
  up: nothing — the SDK bump is a build-input alignment, no source behavior change.
- **Where:** os-apps/paw-compute/wasm/computer_exec/Cargo.toml.

## Fix the app.toml provider string
- **Decision:** "Compute provisioning for agent sandboxes via Fly.io" → "via
  Tensorlake".
- **Came up because:** the description named the wrong provider; the Computer
  entity's provider is tensorlake (dsf and arni-big both run on Tensorlake).
- **Options:** leave it / correct it.
- **Chose correct because:** it is a plainly stale, misleading string. Given up:
  nothing. (The same string is stale in Genesis too; not touched here — no Genesis
  publish in this effort.)
- **Where:** os-apps/paw-compute/app.toml.

## No live prod-install verification in this effort
- **Decision:** Verify by build + unit tests + blob inspection, not a live
  prod/Genesis install.
- **Came up because:** the Definition of Done wants live e2e, but the effort brief
  forbids any Genesis publish / prod install tonight.
- **Options:** publish to Genesis and drive the walk (forbidden) / bound
  verification to repo-side.
- **Chose repo-side because:** the brief scopes this to reconciliation only; the
  live governed-Exec walk was already proven on the dsf box (dd-computer) and the
  code runs in prod today. Given up: a fresh live walk — deferred to a publish
  effort. Not a punt: the objective here is landing the source, which is fully met.
- **Where:** build/test evidence; PR proof record.
