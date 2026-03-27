# ADR-0002: MVP Self-Healing Loop

## Status

Accepted

## Context

Open Paw already boots as an embedded Temper-based daemon, but the MVP needs a fully governed self-healing loop on a real repository. The loop starts from a Paw-controlled channel interaction, provisions developer execution on a sandbox, observes real Logfire alerts, triages them with Scout, and drives code changes through GitHub pull requests.

The existing model still uses `Agent`-era names and ad hoc environment handling, which makes the API shape inconsistent with the product language in the MVP plan and harder to verify across parallel worktree implementations.

## Decision

### 1. Rename the agent model under `OpenPaw`

The agent app will expose `Agent`, `Soul`, `Memory`, and `Skill` entities in the `OpenPaw` namespace, with `Agents`, `Souls`, `Memories`, and `Skills` OData sets. Existing runtime code, startup bootstrap, and WASM integrations must use the renamed endpoints and actions consistently.

### 2. Load operator credentials from `.env`

The daemon reads local credentials from a gitignored `.env` file via `dotenv`. Supported MVP secrets include Anthropic, Discord, E2B, GitHub, and Logfire tokens. Startup seeds those values into the Temper secrets vault for both the default tenant and the active tenant.

### 3. Model the workflow as OS apps

The deep-sci-fi development workflow is represented by a new `paw-harness` OS app, and alert-driven remediation is represented by a new `paw-heal` OS app. The loop remains governed by IOA specs, Cedar policies, and WASM reactions rather than custom Rust orchestration.

### 4. Verify with proof reports

Each milestone is only complete after running its verification flow and committing a proof report under `.proofs/`. This keeps parallel implementations comparable and makes the MVP auditable.

## Consequences

### Positive

- The public API matches the Open Paw product language.
- Shared `.env` handling works cleanly for local worktrees.
- The self-healing loop is modeled in specs that can be installed and iterated without changing the binary.

### Negative

- The rename touches runtime code, specs, tests, and precompiled WASM references.
- Real verification depends on locally available credentials and external services.

### Risks

- Partial renames can leave cross-app actions pointing at stale entity sets.
- Worktree-local verification can fail if `.env` is missing from the active checkout.
