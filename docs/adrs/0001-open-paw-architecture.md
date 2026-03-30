# ADR-0001: Open Paw Architecture

## Status

Accepted

## Context

Temper-claw is the conversational agent layer inside the temper repository — Channel/AgentRoute/ChannelSession entities for multi-platform messaging, Agent/Soul/Memory for governed agent execution, and the Discord transport for I/O. This layer needs to become an independently deployable product.

Open Paw extracts this entire layer from temper, rebrands it under the `Paw` CSDL namespace, and extends it with new capabilities: persistent cloud computers for developer agents, development workflow enforcement (harness), and Ramp Inspect-style self-healing monitoring.

## Decision

### 1. Single binary embedding temper-platform

Open Paw ships as a single Rust binary that embeds the temper platform engine as a cargo git dependency. The binary boots temper, installs Paw OS apps, seeds souls, and starts the Discord transport. Deploys to Railway.

**Rationale:** Simplest deployment UX. One binary, one Railway service, one set of env vars.

### 2. OS apps pattern (not application code)

All agent logic is modeled as temper OS apps — IOA specs (state machines), WASM integrations, Cedar policies. No agent-specific Rust code in the binary. The daemon is a thin bootstrap layer.

**Rationale:** Follows temper-claw's proven pattern. Specs are verifiable, hot-reloadable, governed.

### 3. Paw-branded OS apps

Extracted and rebranded OS apps:
- `paw-agent` (namespace `OpenPaw`) — Agent, Soul, Memory, Skill
- `paw-channels` (namespace `Paw.Channel`) — Channel, AgentRoute, ChannelSession
- `paw-fs` (namespace `Paw.FS`) — File, Workspace
- `paw-pm` — Issues, Plans (project management)
- `paw-transport` — Discord Gateway + webhook listener

New OS apps:
- `paw-compute` (namespace `Paw.Compute`) — Computer entity (Fly Sprites provisioning)
- `paw-harness` (namespace `Paw.Harness`) — ProjectHarness, WorkCycle, Convention
- `paw-heal` (namespace `Paw.Heal`) — Monitor, AlertCycle, MonitorScan

### 4. Fly.io Sprites for persistent agent computers

Developer agents get persistent Linux VMs via Fly.io Sprites. The Computer entity manages provisioning, checkpointing, and lifecycle via WASM integrations that call the Fly Machines API. The sandbox HTTP protocol matches the existing local_sandbox.py interface.

**Rationale:** Sprites are persistent (survive restarts), auto-idle (low cost when sleeping), and purpose-built for coding agents. Behind a ComputerProvider abstraction for future provider swaps.

### 5. Souls as TemperFS files

Agent personalities (Paw, Developer, SRE) are system prompt markdown files in `souls/`. At boot, the daemon reads them from disk, creates TemperFS File entities, and registers Soul entities. Runtime uses the entity system — disk files are seed data only.

### 6. Turso storage

Default storage backend is Turso (cloud SQLite via libSQL). Lightest option for Railway. Local libSQL file for development, Turso Cloud for production.

### 7. Ramp Inspect-style self-healing (post-MVP)

The paw-heal OS app implements webhook-driven monitoring: auto-generated Datadog monitors fire alerts, SRE agents triage (real issue or noise), and developer agents fix or tune. Follows Ramp's pattern of 1 monitor per ~75 lines of code.

## Consequences

### Positive
- Independently deployable agent product
- All agent logic verifiable via temper's verification cascade
- Cedar governance on all agent actions
- Hot-reloadable specs without binary restart
- Multi-tenant capable (different users, different souls, same daemon)

### Negative
- Cargo git dependency on temper means compile times scale with temper's workspace
- WASM modules must be pre-compiled and included in the repo
- Namespace rename creates a one-time migration burden for existing data

### Risks
- Cross-references between OS apps (e.g., route_message WASM references Agent entity type) must be updated consistently
- temper repo cleanup after extraction needs coordination with other work on that repo
