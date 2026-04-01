# DSF Harness — Development Governance for Deep-Sci-Fi

## What

The DSF Harness is a reference app that enforces a 3-level gate system for the Deep-Sci-Fi project through a single merged entity type: `DsfWorkCycle`. It combines repository configuration (harness setup) with a governed implementation loop (work cycle) into one entity, so agents cannot skip verification steps — the platform rejects state transitions when boolean gate fields are not satisfied.

## Entity Types

### DsfWorkCycle

A merged harness + work cycle entity. First you configure the repository contract (repo URL, tech stack, conventions), then activate it, then create work cycles within it by dispatching `CreateWorkCycle`.

**Harness fields:** `RepoUrl`, `TechStack`, `Conventions`, `LastActivatedAt`

**Work cycle fields:** `TaskSummary`, `PlannerId`, `PlanSummary`, `TestSummary`, `PrUrl`, `ApproverId`, `ErrorMessage`, `SandboxUrl`, `ReviewNotes`

**Gate booleans:** `HasPlan`, `MigrationsOk`, `TypecheckOk`, `UnitTestsOk`, `DstOk`, `PolicyGatesOk`, `E2eOk`, `TestsPassed`

## States

```
Created → Configured → Active → Planning → Planned → InProgress → Testing → Reviewing → Complete
                                    ↓          ↓          ↓           ↓          ↓
                                  Failed     Failed     Failed      Failed     Failed
```

- **Created** — Entity exists but has no repository metadata.
- **Configured** — Repository URL, tech stack, and conventions are set.
- **Active** — Harness is live and ready to accept work cycles.
- **Planning** — A work cycle has been started; waiting for a plan.
- **Planned** — Plan written; ready to begin implementation.
- **InProgress** — Implementation underway; agents report gate results via `Report*` actions.
- **Testing** — Level-1 gates passed (migrations, typecheck, unit tests); running level-2 checks.
- **Reviewing** — All required tests passed; awaiting human or agent approval.
- **Complete** — Approved and merged.
- **Failed** — Work cycle failed at any stage.

## Gate Verification

The `gate_verifier` WASM integration runs verification commands in a sandbox and dispatches `Report*` actions for each passing check. It is triggered as an effect on `BeginTesting` (level-1 gates) and `PassTests` (level-2 gates).

**Level 1 (required for BeginTesting):**
- `migrations_ok` — Alembic migration check
- `typecheck_ok` — TypeScript/Python type check
- `unit_tests_ok` — pytest + vitest unit tests

**Level 2 (required for PassTests):**
- `dst_ok` — Hypothesis DST simulation tests
- `policy_gates_ok` — All policy gate scripts

**Optional:**
- `e2e_ok` — Playwright E2E tests

## Usage

### 1. Create and configure the harness

```
POST /tdata/DsfWorkCycles
  → creates entity in "Created" state

POST /tdata/DsfWorkCycles('{id}')/DSF.Harness.Configure
  { "repo_url": "https://github.com/arni-labs/deep-sci-fi.git",
    "tech_stack": "Next.js + FastAPI + PostgreSQL",
    "conventions": "..." }
  → transitions to "Configured"

POST /tdata/DsfWorkCycles('{id}')/DSF.Harness.Activate
  → transitions to "Active"
```

### 2. Start a work cycle

```
POST /tdata/DsfWorkCycles('{id}')/DSF.Harness.CreateWorkCycle
  { "task_summary": "Add user profile page",
    "planner_id": "agent-ren" }
  → transitions to "Planning"
```

### 3. Plan and implement

```
POST .../WritePlan    { "plan_summary": "...", "planner_id": "agent-ren" }  → Planned
POST .../StartWork                                                          → InProgress
POST .../UpdateSandbox { "sandbox_url": "https://sandbox.example.com" }     → (stays InProgress)
```

### 4. Report gates and advance

Agents report gate results as self-loop actions in InProgress:

```
POST .../ReportMigrations    → sets migrations_ok = true
POST .../ReportTypecheck     → sets typecheck_ok = true
POST .../ReportUnitTests     → sets unit_tests_ok = true
POST .../BeginTesting        → Testing (requires migrations_ok, typecheck_ok, unit_tests_ok)
POST .../ReportDst           → sets dst_ok = true
POST .../ReportPolicyGates   → sets policy_gates_ok = true
POST .../PassTests           → Reviewing (requires dst_ok, policy_gates_ok)
```

### 5. Review and complete

```
POST .../Approve  { "approver_id": "human-sesh", "pr_url": "https://..." }  → Complete
  — or —
POST .../RequestChanges { "review_notes": "Fix the migration" }              → Planning (resets tests_passed)
```

### 6. Failure

Any stage can fail:

```
POST .../Fail { "error_message": "Sandbox crashed" }  → Failed
```
