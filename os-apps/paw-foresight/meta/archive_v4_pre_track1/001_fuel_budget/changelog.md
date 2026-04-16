# Run 001 Changelog

## Changed File
`os-apps/paw-agent/specs/session.ioa.toml`

## What Changed
Increased the WASM fuel budget for the `run_tools` integration (monty_repl) from 50 billion to 500 billion instructions (10x).

## Before
```toml
max_fuel = "50000000000"
```

## After
```toml
max_fuel = "500000000000"
```

## Why
The orchestrator session exhausted the 50B fuel budget after only 3-4 LLM tool call rounds. The full projection loop requires 20-30+ rounds: read config, write state files, spawn 3 probes (create Agent + Session + Configure each), poll for probe completion (multiple iterations), read observations, perform convergence analysis, write projected state, dispatch audit actions, advance step, and repeat for step 2 plus final synthesis.

## Scope
This is a platform-level change affecting all paw-agent sessions. Existing safeguards (`timeout_secs = "900"`, `max_turns = "100"`) remain in place to prevent runaway sessions.
