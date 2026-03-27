# Proof Report: 006 — Full Self-Healing Loop Working

## Date
2026-03-26

## Branch / Commit
`feat/openpaw-self-heal-loop-claude` / `7d04723`

## What Was Done

Found and fixed the root cause of the turn 13 failure:
- **blob_adapter WASM CTX_BUF_LEN was 8KB** — entity state JSON exceeded this after ~13 turns
- **Fix: increased to 128KB** — Developer agent now completes 28+ turns
- Implemented content-per-file session architecture (ADR-0003)
- Disabled legacy conversation file write in session tree mode
- Disabled fsync for local sandbox (only needed for E2B)

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Developer agent runs >13 turns | No turn 13 crash | 28 turns, Completed | **PASS** |
| Developer clones deep-sci-fi | Repo cloned | Cloned at /tmp/paw-workspace/dsf | PASS |
| Developer reads proposals API | File content read | proposals.py analyzed | PASS |
| Developer reads database config | Config found | database.py, models.py analyzed | PASS |
| Developer writes diagnosis.md | File created | Detailed diagnosis with root causes | PASS |
| Developer reports findings | Comprehensive report | 6 issues identified, recommendations given | PASS |
| Session tree stays small | Content in separate files | Content externalized to TemperFS Files | PASS |
| No fsync overhead | Local sandbox skips sync | fsync only runs for E2B | PASS |

## Developer Agent Diagnosis (actual output)

The Developer agent identified these **real issues** in deep-sci-fi's proposals API:

1. **Database Connection Pool Exhaustion** — No explicit pool_size, pool_timeout, max_overflow
2. **Embedding Service Cascading Failures** — OpenAI API timeouts in proposal similarity checks
3. **Complex N+1 Query Patterns** — Eager loading with multiple JOINs, raw SQL for vectors
4. **Missing Database Indices** — pgvector queries, status filtering, timestamp ordering
5. **Transaction Management Issues** — Explicit rollback patterns suggesting state problems
6. **No Circuit Breaker** — External dependency failures not properly isolated

## Architecture Diagram

```
Human → "Set up self-healing for deep-sci-fi"
  │
  ▼ Channel.ReceiveMessage
Paw Agent (12 turns, 49s)
  ├── temper_create → ProjectHarness (Active)
  ├── temper_create → AlertCycle (Triaging)
  ├── temper_action → DiagnoseReal
  ├── temper_create → Issue (Backlog)
  └── temper_create → WorkCycle (Planning)

Developer Agent (28 turns, 120s)
  ├── bash: git clone deep-sci-fi
  ├── bash: find proposals API files
  ├── read: proposals.py, database.py, models.py
  ├── bash: grep for connection/pool/timeout patterns
  ├── write: /tmp/paw-workspace/diagnosis.md
  └── Report: 6 issues identified with recommendations

Storage:
  Session tree: pure structural manifest (~5KB)
  Content files: 30+ TemperFS File entities
  Blob store: Turso SQLite (content-addressed)
  blob_adapter: 128KB context buffer (was 8KB → root cause)
```

## Commit Log (22 commits)
```
7d04723 fix: blob_adapter CTX_BUF_LEN 8KB → 128KB
61ee9f1 feat: content-per-file session architecture
ec1bd29 docs: ADR-0003 session storage
03c4b1e docs: proof report 004
c8cb6b2 fix: auto soul binding, turn limits, agent_config
e3cf265 feat: Developer agent clones deep-sci-fi
ae53ab6 feat: E2B sandbox provisioning works
8ae533c feat: Paw autonomously creates entities
7624d7c feat: full tool loop working
3cfc5d2 feat: paw-harness + paw-heal OS apps
7fe023a feat: Paw responds with soul personality
ac84f5f feat: Discord transport connected
1cd2449 feat: entity rename + Turso + blob_endpoint
8764e51 feat: initial scaffold
... (22 total)
```
