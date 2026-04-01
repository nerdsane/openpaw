# DSF Team — Reference App

## What is this?

The DSF Team app defines a 7-role agent team for the **Deep Sci-Fi** platform — a production web application combining speculative world-building with AI-driven narrative experiences. The team is designed to operate under Temper's entity state machines with Cedar policy governance, where each role has explicit autonomy boundaries and tool access controls.

## Team Composition

| Role | Agent | Description |
|------|-------|-------------|
| **Lead** | Ren (INTP) | Product lead and only Soul. Holds creative vision and engineering discipline. Merges PRs, triggers deploys, escalates to human on model/deployment changes. |
| **SWE** | (role, no soul) | Software engineer. Creates PRs, pushes branches, runs tests. Cannot merge or modify deploy config. |
| **SRE** | (role, no soul) | Site reliability engineer. Creates/tunes/deletes Datadog monitors, queries deploy status. Cannot trigger redeploys or merge PRs. |
| **Design** | (role, no soul) | Design reviewer. Proposes UI changes, reviews PRs against the neo-editorial design system. Cannot merge PRs. |
| **Librarian** | (role, no soul) | Content quality analyst. Runs content scans, flags coherence drift, assesses scientific grounding. Cannot modify content directly. |
| **Code Reviewer** | (role, no soul) | Reviews PR diffs for code quality, plan alignment, and DSF conventions. Reports verdict to the WorkCycle harness. |
| **DST Reviewer** | (role, no soul) | Reviews PR diffs for deterministic simulation testing compliance. Ensures Hypothesis DST coverage, invariant integrity, and BUGGIFY annotations. |

Ren is the only agent with a Soul document (personality, worldview, tradeoff style). The other six are skill-driven roles — they receive skill documents that define their expertise but have no persistent identity.

## Included Skills

| Skill file | Injected into | What it covers |
|------------|--------------|----------------|
| `skills/swe-conventions.md` | SWE | Repo layout, tech stack (Next.js 14, FastAPI, SQLAlchemy), coding standards, migration workflow, test expectations |
| `skills/sre-monitoring.md` | SRE | Datadog monitor types, coverage targets, alert triage, health scan workflows, Logfire observability patterns |
| `skills/design-system.md` | Design | Neo-editorial design tokens (typography, color, spacing, motion), component patterns, TASTE.md reference |
| `skills/content-standards.md` | Librarian | World coherence assessment, scientific grounding standards, coherence drift detection, content quality metrics |
| `skills/reviewer-code.md` | Code Reviewer | PR diff review process, plan alignment checks, backend/frontend quality standards, verdict reporting |
| `skills/reviewer-dst.md` | DST Reviewer | Hypothesis DST integrity, invariant coverage, BUGGIFY fault injection review, simulation reproducibility |

## Policies

| Policy file | What it governs |
|-------------|----------------|
| `policies/autonomy.cedar` | Per-role autonomy boundaries — what each role can do without approval (e.g., SWE can create PRs but not merge them, only Lead can trigger redeploys) |
| `policies/tool_governance.cedar` | Per-role tool access — which CLI commands and API endpoints each role can execute (e.g., SWE can run `gh pr create` but not `gh pr merge`) |

## Souls

| Soul file | Agent |
|-----------|-------|
| `souls/ren/SOUL.md` | Ren's identity, sensibility, domain fluency, tradeoff style, worldview, boundaries, and INTP traits |
| `souls/ren/STYLE.md` | Ren's communication style guide |

## Setup

To set up the DSF team in a Temper workspace:

1. **Install the paw-agent app** (provides Soul and Skill entity types):
   ```
   install_app("your-workspace", "paw-agent")
   ```

2. **Install the dsf-team reference app**:
   ```
   install_app("your-workspace", "dsf-team")
   ```

3. **Create Ren's Soul entity**:
   ```
   create("your-workspace", "Souls", {
     "Id": "ren",
     "Name": "Ren",
     "Role": "Lead",
     "SoulDocument": <contents of souls/ren/SOUL.md>,
     "StyleDocument": <contents of souls/ren/STYLE.md>
   })
   ```

4. **Register each Skill entity**:
   ```
   create("your-workspace", "Skills", {
     "Id": "swe-conventions",
     "Role": "SWE",
     "Document": <contents of skills/swe-conventions.md>
   })
   ```
   Repeat for each skill file (sre-monitoring, design-system, content-standards, reviewer-code, reviewer-dst).

5. **Policies are installed automatically** when the app is installed. Cedar policies in `policies/` are loaded into the workspace's policy store and enforced on every action dispatch.

## How It Works

The team operates through the **paw-harness** WorkCycle state machine:

1. Ren (Lead) creates a WorkCycle with a task summary and plan
2. SWE implements the plan, pushes a branch, creates a PR
3. Code Reviewer and DST Reviewer review the PR diff independently
4. SRE verifies monitoring coverage for any new endpoints
5. Design reviews UI changes against the design system
6. Librarian checks content changes for world coherence
7. Ren approves and merges (or requests changes)

Every action is governed by Cedar policies — agents that exceed their autonomy boundaries are blocked, and the pending decision surfaces to Ren or the human operator.
