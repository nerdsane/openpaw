# Foresight — Deep Sci-Fi

This skill document is auto-injected into Foresight Probe prompts via the ProductModel. It covers the Deep Sci-Fi platform architecture, known hot spots, testing strategy, and architectural tensions that probes should watch when projecting forward.

## Tech Stack

| Layer | Technology | Notes |
|-------|-----------|-------|
| Frontend | Next.js 14 App Router | App directory structure, server components by default |
| Styling | Tailwind CSS | Utility-first, design system tokens in `tailwind.config.ts` |
| ORM | Drizzle ORM | Type-safe schema in `platform/backend/db/models/` |
| Database | Neon Postgres + pgvector | Serverless Postgres, vector similarity search for memory/embeddings |
| Backend API | FastAPI (Python 3.12) | Async handlers, Pydantic v2 models |
| Testing | pytest + Hypothesis DST + Playwright E2E | Property-based testing for core logic, E2E for critical flows |

## Key Modules

### `platform/backend/api/`

Core API route handlers:

| Module | What it does | Hot spot risk |
|--------|-------------|---------------|
| `proposals.py` | Proposal creation, validation, voting | **High** — validation logic is complex and frequently modified |
| `worlds.py` | World lifecycle management | Medium — stable but tightly coupled to dweller state |
| `dwellers.py` | Dweller CRUD + memory attachment | **High** — memory management is the most changed code path |
| `actions.py` | Action dispatch and resolution | Medium — depends on proposal + world state |
| `stories.py` | Story generation and retrieval | Low — mostly read paths |
| `reviews.py` | Review submission and aggregation | Low — recent addition, not yet heavily used |

### `platform/backend/services/`

Business logic services:

| Service | What it does | Hot spot risk |
|---------|-------------|---------------|
| `memory.py` | Dweller memory management, embedding generation, similarity search | **High** — embedding pipeline cost vs search quality is an active tension |
| `reputation.py` | Reputation calculation, decay, leaderboard | **High** — calculation logic changes frequently, edge cases in decay |
| `notification.py` | Event-driven notifications | Low — stable fire-and-forget pattern |

### `platform/backend/db/`

Database layer:

| Component | What it does |
|-----------|-------------|
| `models/` | Drizzle schema definitions, pgvector column types |
| `migrations/` | Schema migrations (linear, no branching) |

## Known Hot Spots

These are the areas where foresight probes should pay closest attention:

### Proposal Validation
- Complex validation rules that interact with world state, dweller permissions, and timing constraints
- Frequently modified — high commit density in `proposals.py` and related tests
- Edge cases around concurrent proposals, expired proposals, and proposal dependencies

### Dweller Memory Management
- The most-changed code path in the system
- Manages embedding generation (expensive), storage, retrieval, and garbage collection
- pgvector similarity search performance degrades with index size
- Memory attachment/detachment lifecycle has subtle state management

### Reputation Calculation
- Reputation decay runs on a schedule and interacts with action resolution
- Leaderboard queries can be expensive with large dweller populations
- Edge cases: negative reputation, overflow guards, decay floor

## Testing Strategy

### pytest
- Unit and integration tests for all API routes and services
- Fixtures for database state, authenticated clients, populated worlds

### Hypothesis DST (Domain-Specific Testing)
- Property-based tests for proposal validation rules
- Stateful testing for dweller memory lifecycle
- Shrinking finds minimal failure cases — pay attention to shrunk examples in CI output

### Playwright E2E
- Critical user flows: create world, add dweller, submit proposal, vote, resolve
- Flaky test risk: world creation flow depends on embedding pipeline timing

## Architectural Tensions

These are ongoing tensions that shape the system's evolution. Probes should watch for signals that any of these are shifting:

### Testing Cost vs. Coverage
- Hypothesis tests are thorough but slow — full suite takes 8+ minutes
- Playwright E2E tests are flaky when the embedding pipeline is under load
- Trade-off: running full suite on every PR vs. running fast subset and full suite nightly
- Signal to watch: CI duration trends, flaky test rate, coverage gaps in changed code

### Frontend-Backend Type Consistency
- Drizzle types in TypeScript, Pydantic models in Python — no shared schema
- Drift between frontend expectations and backend contracts causes subtle bugs
- Signal to watch: PRs that change both `models/` and API response shapes, issues tagged "type mismatch"

### Embedding Pipeline Cost vs. Search Quality
- pgvector similarity search quality depends on embedding model and index parameters
- Better embeddings = higher cost per memory operation
- Index rebuild frequency affects search accuracy for recently added memories
- Signal to watch: embedding pipeline latency p99, similarity search recall metrics, monthly embedding API cost

## Monitoring

### Datadog Monitors

| Monitor | What it watches | Threshold |
|---------|----------------|-----------|
| Error rate | HTTP 5xx from `deep-sci-fi-api` | > 5 errors in 5 min |
| Latency p99 | Request duration across all endpoints | > 2000ms over 5 min |
| DB pool | Active Postgres connections | > 80 connections |
| Embedding pipeline | pgvector query duration p95 | > 500ms over 5 min |
| Endpoint health | Per-endpoint status codes | > 399 avg over 5 min |
| Exception tracking | Unhandled exceptions in logs | > 10 in 5 min |

### What Probes Should Watch

- Error rate trends over the projection horizon — is it stable, improving, or degrading?
- Latency p99 correlation with deployment frequency — do deploys cause latency spikes?
- DB pool utilization trends — approaching the limit suggests connection management issues
- Embedding pipeline latency correlation with memory volume — does it scale linearly or worse?
- Alert frequency from AlertCycle history — is the system getting noisier or quieter over time?
