# OpenPaw Vision

## What OpenPaw Is

OpenPaw is an intelligent agent platform that a human deploys to the cloud (e.g. Railway) and interacts with through Discord. The human talks to **Paw**, the orchestrator agent, and asks it to manage their software projects. Paw "hires" developer agents — autonomous software engineers that maintain repositories like a human developer would.

This is not a scripted CI/CD pipeline. The agents are intelligent. They make decisions, adapt to context, and collaborate through governed shared state. The harness guides them (what must happen before code ships), but does not prescribe step-by-step deterministic execution.

## The Human Experience

1. Human deploys OpenPaw to Railway (or similar). One binary, one service, a set of env vars.
2. Human adds the OpenPaw bot to their Discord server.
3. Human says on Discord: **"Manage deep-sci-fi for me."**
4. Paw takes it from there:
   - Creates a Developer agent (or a team of agents)
   - Provisions the agent a governed sandbox (its "laptop")
   - Clones the repo, installs dependencies
   - Sets up the harness (project-specific development workflow rules)
   - Bootstraps monitors across the entire codebase
   - Reports back to the human on Discord
5. From then on, the system runs autonomously:
   - Monitors fire when something goes wrong in production
   - SRE triages each alert (real issue or noise?)
   - Developer fixes real issues, opens PRs
   - Paw proactively reports to the human: "Found 3 bugs overnight, here's what we did"
   - Human reviews and approves PRs on Discord (or wherever they prefer)
6. The human only intervenes when they want to. The autonomous slider controls how much the agents can do without asking.

## Core Concepts

### Agents as Humans

Every agent is modeled as a human equivalent:
- **Paw** — The manager. Talks to the human, delegates work, reports progress. Does not write code.
- **Developer** — A software engineer. Gets assigned to a project, gets a computer, maintains the codebase. Sets up its own harness. Writes code, runs tests, opens PRs, deploys.
- **SRE** — A monitoring/triage specialist. Wakes up when alerts fire, diagnoses issues, hands real problems to Developer.
- **Evolution Agent** — A platform/tools engineer. Watches how other agents use their tools, identifies unmet intents (roundabout workarounds, missing capabilities, inefficiencies), and improves the tools and apps agents use.

When a developer gets "hired," it's like a new employee getting a laptop, credentials, and project assignment. Everything is governed: what the computer can access, what credentials it has, what it's allowed to do.

### The Harness (Project-Specific Governance)

Every project has a harness — the rules that govern how development work happens on that project. Think of it as the engineering playbook.

- **Who sets it up**: The Developer agent assigned to the project creates it, informed by the codebase's existing conventions. The human approves it on Discord.
- **What it contains**: Tech stack, conventions, required checks, deployment rules, testing requirements.
- **How it's enforced**: Through Temper state machines and Cedar authorization policies. The agent literally cannot skip required steps — it's not just prompt guidance, it's policy enforcement.
- **Project-specific**: deep-sci-fi has different harness rules than another project. Each project's harness reflects its unique stack and conventions.

### Monitor-Driven Self-Healing (Ramp-Style)

Inspired by [Ramp's self-maintaining Sheets system](https://engineering.ramp.com/ramp-sheets-self-maintaining). The pattern:

1. **Bootstrap monitors**: When a developer agent first takes ownership of a project, it sets up monitors across the entire codebase — granular, one per ~75 lines of code. These watch error rates, latency, exceptions, log patterns.
2. **Ongoing monitor generation**: On every PR or change (by the agent or by human developers), new monitors are generated for the changed code. This is part of the harness — it's how the project stays covered.
3. **Alert → Triage → Fix loop**:
   - Monitor fires → webhook hits OpenPaw → AlertCycle entity created
   - SRE session wakes up, reads alert context, investigates
   - If real issue: SRE creates a WorkCycle + PM Issue, Developer session picks it up, reproduces in sandbox, fixes, opens PR
   - If noise: SRE tunes or deletes the monitor
   - Dedup: If an active Issue already exists for this monitor, add context instead of duplicating
4. **Observability platforms**: Datadog first (OpenTelemetry-native, developer-friendly), Datadog after (industry standard). Both support webhook-triggered alerts, so the integration pattern is the same.

### Governed Shared State (Temper)

All agent collaboration happens through Temper entities — governed, auditable shared state:

- **PM app** (Issues, Plans, Cycles) — agents coordinate work like humans use Jira
- **File system** (Workspaces, Files) — persistent storage that survives sandbox churn
- **Harness** (ProjectHarness, WorkCycle) — development workflow state machines
- **Heal** (Monitor, AlertCycle) — observability and remediation state machines
- **Channels** (Channel, ChannelSession) — conversation continuity across Discord threads

Cedar authorization governs every entity and action. Developer A cannot touch Developer B's project. An agent cannot merge without the harness allowing it. The human's autonomy slider adjusts these boundaries.

### The Autonomous Slider

Per-agent, per-project, as granular as it gets. Examples:

- "Developer on deep-sci-fi can open PRs without asking, but must ask before merging"
- "Developer on deep-sci-fi can merge fixes for monitor-detected bugs, but must ask for feature work"
- "SRE can tune monitors freely, but must notify me when escalating"

This is Cedar policy under the hood. The slider adjusts what actions are auto-permitted vs. require human approval via Discord.

### Computer Governance

Every agent's sandbox is governed:

- **What it can access**: Network rules, allowed domains, API endpoints
- **What credentials it has**: Scoped secrets — only the tokens needed for its assigned project
- **What it's connected to**: GitHub repos, observability platforms, deployment targets
- **Isolation**: Developer A's sandbox cannot reach Developer B's repos or credentials

### The Evolution Loop

A dedicated always-running agent (the Evolution Agent, comparable to a platform engineer) that:

1. **Watches agent activity** across all projects — tool usage, workarounds, failures, friction
2. **Identifies unmet intents** — when agents try to do something that's not there, or have to use roundabout paths for something that should be direct
3. **Improves tools and apps** — agents get their own copies of the OS apps (paw-heal, paw-harness, etc.) and can evolve them. The Evolution Agent proposes and implements these improvements.
4. **Surfaces suggestions** — primarily a background agent. Presents improvement proposals to the human for review rather than autonomously changing shared infrastructure.

This is not prescriptive optimization. It's intelligent: the Evolution Agent understands why agents are struggling and designs better abstractions.

### Agent-Created Apps on Temper

Agents don't just use pre-built tools. They can create Temper apps as tools for themselves — custom state machines, entities, and workflows that make their job easier. Examples:

- A developer agent creates a deployment-tracking app for its project
- A team of agents creates a shared code-review app with custom states

These agent-created apps are what the Evolution Agent watches and improves.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    Human (Discord)                    │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────┐
│                  Paw (Orchestrator)                   │
│  - Talks to human                                    │
│  - Hires/manages agents                              │
│  - Reports progress proactively                      │
│  - Does NOT write code                               │
└──┬────────────┬────────────┬────────────────────────┘
   │            │            │
   ▼            ▼            ▼
┌──────┐  ┌──────────┐  ┌──────────────┐
│SRE │  │Developer  │  │Evolution     │
│      │  │(per proj) │  │Agent         │
│Triage│  │Code, test │  │Improve tools │
│alerts│  │deploy, PR │  │Fix friction  │
└──┬───┘  └────┬─────┘  └──────────────┘
   │           │
   ▼           ▼
┌─────────────────────────────────────┐
│         Temper (Governed State)      │
│  PM · Harness · Heal · FS · Compute│
│  Cedar policies · State machines    │
└─────────────────────────────────────┘
   │           │
   ▼           ▼
┌──────────┐  ┌──────────────────────┐
│Datadog/  │  │Sandboxes (Modal/Fly) │
│Datadog   │  │Per-agent, governed   │
│Monitors  │  │Isolated computers    │
└──────────┘  └──────────────────────┘
```

## Demo Scenario: deep-sci-fi

The first demonstration of the full system:

1. Human on Discord: "Manage deep-sci-fi for me"
2. Paw creates a Developer agent, provisions sandbox, clones `arni-labs/deep-sci-fi`
3. Developer bootstraps harness (Next.js frontend + Python backend conventions), human approves on Discord
4. Developer bootstraps Datadog monitors across the codebase
5. A real alert fires (or a human developer pushes a bad change)
6. SRE wakes up, triages, creates Issue + WorkCycle
7. Developer reproduces the bug in sandbox, fixes it, opens PR
8. Paw reports to human on Discord: "Found a bug, here's the PR"
9. Human approves, Developer merges and monitors the deploy
10. SRE confirms the alert is resolved, closes the AlertCycle

## What Exists Today vs. Vision

| Capability | Status | Gap |
|---|---|---|
| Single binary deploys to Railway | ✅ Done | None |
| OS apps install at boot (7 apps) | ✅ Done | None |
| Paw, Developer, SRE souls | ✅ Done | Evolution Agent soul missing |
| OData API for all entities | ✅ Done | None |
| Discord transport | ⚠️ Wired | Not re-proven end-to-end on this branch |
| Paw orchestrates full flow via Discord | ❌ Not proven | Paw exists but hasn't driven the full loop |
| Developer clones repo in governed sandbox | ✅ Proven | Only clone milestone, not full remediation |
| SRE → Developer → PR (self-heal) | ✅ Proven | Manually triggered with synthetic alert |
| Webhook alert ingestion | ✅ Implemented | Native Datadog + GitHub merge webhook paths exist; needs fresh end-to-end proof |
| Datadog monitor integration | ⚠️ Partial | Datadog-backed monitor/query path exists, but monitor bootstrap and post-deploy verification are incomplete |
| Monitor generation (bootstrap + per-PR) | ⚠️ Partial | `MonitorScan` spec exists, but full bootstrap automation is not yet proven |
| Harness as Cedar-enforced policy | ⚠️ Partial | Entities work, Cedar policies are broad |
| Persistent governed sandbox (Modal/Fly) | ❌ Specs only | No Computer WASM modules |
| Computer governance (network, creds) | ❌ Not implemented | Cedar + sandbox config needed |
| Autonomous slider | ❌ Not implemented | Cedar policy adjustment mechanism needed |
| PM integration (Issues from alerts) | ⚠️ Partial | PM app exists, not wired into alert flow |
| Evolution Agent | ❌ Not started | No soul, no unmet-intent detection |
| Agent-created Temper apps | ❌ Not started | Agents don't yet create their own apps |
| Full CI/CD closure (merge → deploy → verify) | ❌ Not implemented | Stops at PR today |
| Crash/restart recovery | ⚠️ Wired | Not proven |
| Context compaction | ⚠️ Wired | Not proven under pressure |

## Priority Path to Demo

Ordered by what unblocks the demo scenario:

1. **Discord end-to-end** — Prove Paw talks to human, human says "manage deep-sci-fi", flow starts
2. **Paw orchestration** — Paw creates Developer, provisions sandbox, bootstraps project
3. **Webhook ingestion** — Real `POST /webhooks/ingest` that creates AlertCycle entities
4. **Datadog monitor bootstrap** — Developer generates monitors for existing codebase
5. **Datadog alert → webhook** — Monitor fires, webhook hits OpenPaw, SRE triages
6. **SRE → Developer → PR in a governed cloud sandbox** — Full remediation in a real remote sandbox (not local)
7. **PM integration** — Alert triage creates Issues, visible in PM
8. **Harness enforcement** — Cedar policies that actually block non-compliant actions
9. **Paw proactive reporting** — Paw messages human on Discord with status updates
10. **Autonomous slider** — Cedar policy controls for per-agent/per-project autonomy

Post-demo priorities:
- Evolution Agent and unmet intent detection
- Agent-created Temper apps
- Full CI/CD closure (merge → deploy → verify)
- Persistent governed sandbox (Fly Sprites)
- Modal sandbox integration
- Multi-project management
