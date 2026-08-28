# Spec: stage-3 S0 - record entities + shadow ingest (ARN-430)

This is the RFC for phase 1 of stage 3. It implements exactly the S0 "mirror-in"
row of the shadowing plan in `stack/docs/stage3-spec.md`: the record entities and
the `record_ingest` module, testable against already-merged PRs. It deliberately
does not touch CI, Cedar, the `WorkCycle -> Effort` rename, or any gate behavior.

## The contract

One module and five entity changes, all in `os-apps/paw-patrol`.

### record_ingest (new WASM module, temporary - retired at S3)

Pure function of one input.

- INPUT: `comment_body` - a raw GitHub issue/PR comment body string (from
  `ctx.trigger_params`).
- OUTPUT (the returned callback Value):
  - `kind`: `"review" | "proof" | "none"`
  - `record`: the decoded record as JSON (object; `{}` when none/malformed)
  - `parse_ok`: bool
  - `commit`: the record's commit sha (empty string when none/malformed)

Behavior:

1. Scan for a marker `<!-- sdlc-review-record-b64\n<base64>\n-->` first, then
   `<!-- sdlc-proof-record-b64\n<base64>\n-->`.
2. If a marker is found, `kind` is `review` / `proof` from the marker tag.
   Base64-decode the payload, parse it as JSON, and read `commit`.
3. `parse_ok` is true only when: a marker was found, the payload base64-decodes,
   the result is a JSON object, and `commit` is a 40-character lowercase-hex sha.
   Otherwise `parse_ok` is false.
4. No marker at all -> `kind = "none"`, `parse_ok = false`, `commit = ""`,
   `record = {}`.

The module never creates entities and never dispatches transitions. It parses and
returns; the state machine's `IngestRecord` transition writes the fields.

### ReviewRun (was review_run) - additive record lifecycle

Existing states and actions (`Requested/Claimed/Reviewing/ChangesRequested/
Approved/Escalated/Failed`, and their `review_gate_lifecycle` triggers) are
unchanged. Added:

- States: `Recorded`, `Superseded` (both indefinite).
- Fields (all `string` except the bool): `commit`, `reviewers_ran` (JSON array),
  `findings` (JSON array of `{severity, by, file_line, claim, failure_scenario,
  resolved}`), `risk`, `open_act_on_count`, `record_present` (bool).
- `IngestRecord`: `from [Requested, Reviewing, Recorded] -> Recorded`, params
  `commit, reviewers_ran, findings, risk, open_act_on_count`. Effect:
  `set_bool record_present true`.
- `Supersede`: `from [Recorded] -> Superseded`, when a newer record for a later
  head replaces this one.
- Invariant `RecordedHasRecord`: `when [Recorded, Superseded] assert
  record_present` - a run in the record states must have gone through
  `IngestRecord`, which is only dispatched on a `parse_ok` record (checked
  commit sha + non-empty reviewer set). The record-shape checks are NOT
  transition guards: the guard grammar has no string/array predicates and cannot
  see action params (see decisions.md).

### ProofRun (was proof_packet) - additive record lifecycle

Existing states/actions (`Drafting/Ready/Rejected`) unchanged. Added:

- States: `Recorded`, `Superseded` (both indefinite).
- Fields (all `string` except the bool): `commit`, `changed_surface` (JSON
  array), `blast_radius` (JSON array), `features` (JSON array of the proof.json
  feature objects), `tests` (the proof.json tests object), `independent_verifier`
  (the proof.json object), `record_present` (bool).
- `IngestProof`: `from [Drafting, Ready, Recorded] -> Recorded`, params
  `commit, changed_surface, blast_radius, features, tests, independent_verifier`.
  Effect: `set_bool record_present true`.
- `SupersedeProof`: `from [Recorded] -> Superseded`.
- Invariant `ProofRecorded`: `when [Recorded, Superseded] assert record_present`.

The proof-shape checks - a 40-char commit sha, a non-empty `changed_surface`,
and no feature with `verdict=="fail"` - are enforced in `record_ingest`'s
`parse_ok`, not as transition guards: the guard grammar cannot inspect
per-element array verdicts or read the action's params. A proof that fails any
check returns `parse_ok=false`, so `IngestProof` is never dispatched and it never
becomes Recorded.

### Adjudication (new)

`Active` (single indefinite state, initial). Fields: `scope` (file:line or a
class name), `ruling` (string), `source` (the owner who ruled), `effort_ref`.
Action `Record`: `from [Active] -> Active`, params `scope, ruling, source,
effort_ref`.

### StandingDecision (new)

`Active -> Retired`. Fields: `text` (the ruling injected into every panel
prompt). Actions: `Adopt` (`Active -> Active`, param `text`), `Retire`
(`Active -> Retired`).

### ShadowVerdict (new, temporary - retired at S3)

`Recorded` (single indefinite state, initial). Fields: `effort_ref`, `pr`,
`gate`, `temper_verdict`, `ci_verdict`, `agree` (bool). Action `Record`:
`from [Recorded] -> Recorded`, params `effort_ref, pr, gate, temper_verdict,
ci_verdict, agree`.

## Model / CSDL

`specs/model.csdl.xml` gains the three new EntityTypes + EntitySets and the new
properties on ReviewRun/ProofPacket. The foundation test asserts these by name,
so CSDL and the ioa specs move together.

## What this is not

No CI change, no Cedar policy, no rename, no gate flip. Those are later phases
(S1-S3 and the Effort state machine). ShadowVerdict is written by nothing in S0;
its writer is S1. The entities are a shadow mirror in S0 - nothing reads them to
gate a merge.

## Test plan (red-green)

A Rust test that feeds `record_ingest`'s parser the real base64 comment bodies
from merged PRs 475, 477, 480 and asserts the decoded fields (kind, commit,
reviewers_ran / changed_surface, parse_ok) match the comments; plus a
malformed-record case (marker present, payload not valid base64/JSON) asserting
`parse_ok == false`, and a no-marker case asserting `kind == "none"`.
