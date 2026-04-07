# ADR-0014: Canvas Mission Control Dashboard

- Status: Accepted
- Date: 2026-04-06
- Deciders: Sesh, Claude
- Related:
  - `dashboard/` (affected code paths)
  - ADR-0005: Temper-native orchestration (entity model the canvas visualizes)

## Context

The dashboard has 8 separate page routes: factory floor, agents, permissions, platform, project detail, session detail, entity viewer, and apps. Each is a disconnected view of the same underlying data. A human operator must navigate between pages to understand what agents are doing, what they're allowed to do, and how the system is connected.

This creates friction. Permissions feel detached from the actions they govern. Project context is separate from the agents working on it. Live session activity is on a different page from the topology of who-connects-to-what.

## Decision

Replace the multi-page dashboard with a single interactive canvas using `@xyflow/svelte` (Svelte Flow). Everything — projects, agents, souls, skills, sessions, permissions — lives on one pannable/zoomable space.

### Sub-Decision 1: Two views total

- **CANVAS** (`/`): The unified view. Projects as container frames, agents as nodes, live terminal feeds inside agents, permissions inline as badges and drill-downs.
- **APPS** (`/apps`): App store for installing OS app capabilities.

All other routes (agents, permissions, platform, project/[id]) are deleted. Their content is absorbed into canvas nodes.

**Why:** A single canvas eliminates navigation friction. The user sees structure and activity simultaneously. Zoom replaces navigation — zoom out for topology, zoom in for detail.

### Sub-Decision 2: Semantic zoom

Agent nodes render differently at different zoom levels:
- Zoomed out: status dot + name (topology view)
- Medium: mini terminal feed + skill tags (monitoring view)
- Zoomed in: full TerminalFeed with authz drill-down (detail view)

**Why:** This replaces page navigation with spatial navigation. The user doesn't click to a different page — they zoom into the thing they want to inspect.

### Sub-Decision 3: Permissions as a layer

Authorization is not a separate destination. It's visible everywhere:
- Agent nodes show authz summary badges ("12 ALLOWED · 0 DENIED")
- Active denials flash the node border
- Pending decisions appear as warning badges
- Per-action Cedar policy matching is in the TerminalFeed drill-down (already built)

**Why:** Permissions are contextual. Seeing "this agent was denied" matters when you're looking at that agent, not when you're on a separate permissions page.

### Sub-Decision 4: @xyflow/svelte

Svelte Flow v1.5.2 — Svelte 5 native, custom node components, pan/zoom, minimap, grouping, edge animation. 36k GitHub stars, actively maintained by a funded team.

**Why:** Only production-grade Svelte 5 node-graph library. Custom nodes are just Svelte components, so we reuse all existing components (TerminalFeed, StatusBadge, AuthzBadge, etc.) directly inside canvas nodes.

## Consequences

### Positive
- Single view replaces 8 pages — eliminates navigation friction
- Structure and activity visible simultaneously
- Permissions visible in context, not as a separate destination
- Spatial memory: users learn where things are on the canvas

### Negative
- Significant implementation effort (new dependency, custom node components, layout algorithm)
- Canvas is client-only (no SSR) — first paint requires JS hydration
- Mobile experience limited to pan/zoom (no semantic zoom at small viewports)

### Risks
- @xyflow/svelte is relatively new (v1.x). If it has bugs with Svelte 5 runes, we'd need workarounds.
- Performance with many simultaneous TerminalFeed renders. Mitigated by only rendering feeds for visible nodes at sufficient zoom.

## Non-Goals

- 3D or WebGL rendering
- User-customizable node positions (positions are computed from data)
- Real-time collaboration (multi-user canvas)
