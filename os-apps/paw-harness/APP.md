# paw-harness

Development workflow governance. A harness is a contract between the platform and a target repository, defining conventions and the work cycle state machine.

## Entity Types

### Harness
Development workflow contract for a repository.

- **States**: Created -> Configured -> Active -> Archived
- **Key actions**: `Configure` (repo_url, tech_stack, conventions, work_cycle_type), `Activate`, `Archive`

### WorkCycle
Governed implementation loop for one planned change. Enforces gate-based progression: plan before code, test before review, review before merge.

- **States**: Planning -> Planned -> InProgress -> Testing -> Reviewing -> Complete / Failed
- **Key actions**:
  - Planning: `Configure`, `WritePlan`
  - Execution: `StartWork` (requires plan), `ReportMigrations`, `ReportTypecheck`, `ReportUnitTests`, `UpdateSandbox`
  - Testing: `BeginTesting` (requires migrations + typecheck + unit tests), `ReportDst`, `ReportPolicyGates`, `ReportE2e`
  - Review: `PassTests` (requires DST + policy gates), `Approve` (requires tests_passed), `RequestChanges`
- **Gate booleans**: `migrations_ok`, `typecheck_ok`, `unit_tests_ok`, `dst_ok`, `policy_gates_ok`, `e2e_ok`

Gate verification and reviewer spawning are project-specific. Each project's reference config (e.g., dsf-harness) defines its own gate_verifier WASM.

## Setup

Depends on `paw-agent` for agent identities. Create a Harness with `Configure` for your repo, then `Activate`. Work cycles are created within the harness context.
