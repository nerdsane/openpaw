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

# Round 1 hardening (panel found 9 act-ons on prod-relevant code)

This reconcile PR is also a hardening PR: the panel reviewed code already running
in prod via Genesis, so these fixes ship here BEFORE any future Genesis publish.

## SECURITY

### 1. Entity fields win over trigger params (no override of pinned fields)
- **Decision:** `param_or_field` → `field_or_param`: read the entity field FIRST,
  fall back to trigger params. Spec-pinned fields (LatencyDiag's canned command /
  pinned computer_id) can no longer be overridden by caller-supplied params.
- **Why:** LatencyDiag is a constrained tool; caller params overriding its pinned
  command breaks the constraint. For Exec, Run writes command/computer_id to the
  fields before the trigger fires, so field-first still yields the caller's input.
- **Where:** computer_exec/src/lib.rs `field_or_param`.

### 2. Drop caller-supplied `created_by` (one source of truth for identity)
- **Decision:** Remove `created_by` from the Exec spec (state var + Run param).
- **Why:** the kernel stamps the true agent identity on every event, non-spoofable
  (dd-computer proof: a forged created_by still recorded the real agent_id). A
  self-declared created_by field is a second, untrustworthy identity source. The
  WASM cannot read the authenticated principal, so deriving it in-module isn't
  possible; the event log is the source of truth. Dropped the field.
- **Where:** specs/exec.ioa.toml.

### 3. Computer read/list restricted to Agent (ADR-0001 scope)
- **Decision:** `permit(principal, [read,list], Computer)` → `principal is Agent`.
- **Why:** the open permit granted read/list to ANY principal, wider than
  ADR-0001 (authenticated tenant agents). Admin stays covered by the blanket
  Admin-on-Computer permit.
- **Where:** policies/compute.cedar.

### 4. Admin gets a consistent blanket on all three entities
- **Decision:** Replace the Admin-callbacks-only permits on Exec and LatencyDiag
  with blanket `permit(principal is Admin, action, resource is Exec|LatencyDiag)`,
  matching the existing Computer blanket.
- **Why:** Admin lacked create/read/list on Exec/LatencyDiag while the intent
  grants Admin the full surface; the three entities are now consistent.
- **Where:** policies/compute.cedar.

## CORRECTNESS

### 5. Fail closed on Computer status
- **Decision:** Refuse the exec unless status == "Ready" exactly; missing/empty
  status is refused (was: only non-empty non-Ready was refused, so empty passed).
- **Where:** computer_exec/src/lib.rs `sandbox_handle_from_computer`.

### 6. Run the command in a child process (control flow can't skip the epilogue)
- **Decision:** The command runs under `bash -c '<cmd>'` (a child), not inline in
  the wrapper's own shell. `exit`/`exec` in the command affect the child, so the
  wrapper's metadata epilogue always runs.
- **Where:** computer_exec/src/lib.rs `wrap_command` (+ `shell_single_quote`).

### 7. Clear result fields on failure (no stale output across re-runs)
- **Decision:** `set_error_result` → local `set_failure_result` that emits the
  error AND empty exit_code/stdout_tail/stderr_tail; both RunFailed actions declare
  those params. A failed LatencyDiag re-scan no longer shows a previous scan's
  output. (The effect system can't set string fields declaratively — only
  status/booleans/counters — so the clear is done via callback params.)
- **Where:** computer_exec/src/lib.rs; specs/exec.ioa.toml; specs/latency_diag.ioa.toml.

### 8. Kill a timed-out command (no orphaned process)
- **Decision:** Wrap with `timeout -k 5s 110s` so a runaway command is killed on
  the sandbox (its process group, no orphan); exit 124 is mapped to RunFailed
  ("exceeded the 110s limit and was terminated"). 110s sits just under the 120s
  WASM invocation cap so the kill and its result land within one invocation.
  Longer runs need the async exec path (D), not a bigger limit.
- **Where:** computer_exec/src/lib.rs `wrap_command` + the run() timeout branch.

### 9. build.sh target → wasm32-wasip1 (the forbidden target, reconciled)
- **Decision:** Change build.sh from wasm32-unknown-unknown to wasm32-wasip1 and
  verify the blob has WASI imports + zero wbindgen before copying.
- **Why + the genesis-blob story:** the standing rule (2026-07-20 incident) forbids
  wasm32-unknown-unknown. The genesis-installed blob (73646d4) is in fact
  unknown-unknown (0 WASI imports, 0 wbindgen) — it happened to work because the
  OLD computer_exec had no chrono (no wbindgen trigger) and no WASI needs, and it
  was built against an EARLIER round of the ARN-401 fix (before #468's getrandom).
  After #468, wasm-helpers pulls getrandom → WASI random_get, so computer_exec
  MUST be wasip1 now: unknown-unknown can't even compile getrandom, and the blob
  must carry the random_get import. The fixed build.sh reproduces the correct
  wasip1 blob (verified: 10 WASI imports, 0 wbindgen). The genesis blob is
  therefore stale vs final #468 and will be rebuilt (wasip1) on the next publish;
  the source still matches genesis HEAD, only the gitignored blob differs.
- **Where:** wasm/build.sh.

## Considers taken
- UTF-8-safe tail: `output_tail` already aligns to char boundaries; also trims a
  leading U+FFFD left by the sandbox's byte-level `tail -c`.
- Separate stderr capture: stdout → `<id>.log`, stderr → `<id>.err` (no more
  2>&1 conflation); stderr_tail now carries real stderr. Tradeoff: stdout_path is
  stdout-only (stderr's full log stays at the sibling `.err` on the sandbox);
  interleaving between the two streams is not preserved — acceptable for an audit
  tail, and each stream is individually complete.
- LatencyDiag's hardcoded Datadog query: noted as tech-debt in the spec comment
  (it was the prototype's learned-tool proof, not a general diagnostic).

## Round 1.1: stderr via a separate stream, not an in-band delimiter (Greptile P1)
- **Decision:** Route the command's stderr tail to the wrapper's own fd 2 (`>&2`,
  captured by the provider as result.stderr) instead of emitting it after an
  `__EXEC_ERR_TAIL` marker in the stdout stream.
- **Came up because:** Greptile P1 — a command printing a line equal to
  `__EXEC_ERR_TAIL` would be treated as the delimiter, corrupting the audit tails.
- **Options:** byte-length-delimit the two tails in stdout / put stderr on a
  separate stream.
- **Chose separate stream because:** it removes ALL command-controllable
  delimiters from the stdout data (the log-path marker stays on a fixed
  wrapper-emitted line, collision-safe), keeps stdout/stderr separated (the
  original consider), and is simpler than byte-length parsing. The parse reverts
  to the collision-safe header form; stderr_tail comes from result.stderr.
- **Where:** computer_exec/src/lib.rs `wrap_command` / `parse_captured_output` /
  `success_params`.
