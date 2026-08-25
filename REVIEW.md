# Review instructions

## Passes
Run four passes and tag each finding with its pass:
- Bugs: logic errors, broken edge cases, state machines that can strand
- Security: Cedar bypasses, secrets in code or logs, unvalidated external input at triggers
- Entity-first compliance: business logic in Rust, polling loops, orchestration outside os-apps, a WASM integration that dispatches transitions, a WASM body bundling several concerns - all findings, per AGENTS.md
- Determinism: WASM that reads ambient time/randomness, unbounded execution, unordered iteration where order is observable

## What Important means here
Reserve Important for findings that would break behavior, leak data, bypass governance, or violate the entity-first rule. Style and naming are nits.

## Cap the nits
At most five nits per review; summarize the rest as a count.

## Do not report
Generated artifacts (`target/`, `dashboard/build`, `Cargo.lock` churn) and anything CI already enforces.
