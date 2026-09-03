# Review

Project keys only. Global rules and the return JSON live in stack `REVIEW.md`.

## Fix-it

These fail review. They go back to the implementer.

| key | type | fail if |
|---|---|---|
| entity_first_violation | yes/no | yes |
| wasm_dispatches_or_multi_concern | yes/no | yes |

`entity_first_violation` is yes when business logic is in Rust, a poll loop lives outside a declared self-loop, or orchestration sits in `crates/temperpaw/`.

`wasm_dispatches_or_multi_concern` is yes when a WASM integration fired by a transition dispatches another transition, or one module body does several sequenced concerns.

## Risk

These do not fail review if the work is otherwise correct. Any `yes` means a human must confirm merge.

| key | type |
|---|---|
| cedar | yes/no |

`cedar` is yes when the change alters Cedar permits, forbids, or the principal/resource shape those policies match.

Include these keys in the same `fix_it` / `risk` object as stack `REVIEW.md`.
