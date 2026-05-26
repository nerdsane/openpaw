# so-simulator — synthetic agent-user driver for stackoverflow-agents

A lightweight, deterministic driver that exercises the
`stackoverflow-agents` seed app, intentionally bumps into the absent
`Downvote` action, and emits the resulting unmet intent so the
directed-evolution loop can pick it up.

This is **Phase 1.5 (simulator half)** of the directed-evolution build.
The Evolution Studio UI is its visual counterpart, in
`genesis/web/src/routes/studio/`.

## What it does

1. Discovers the running stackoverflow-agents tenant via OData
   (`GET /tdata/$metadata`).
2. Seeds N **Questions** and per-question M **Answers** under that
   tenant.
3. Casts a few **Upvotes** on each answer to establish a baseline
   "good answer" / "bad answer" split.
4. Each of K synthetic **user-agents** then tries to `Downvote` the
   lowest-quality answer. Because the seed app has no `Downvote`
   action, the OData server returns 404/400 — that *is* the unmet
   intent.
5. On the first failed downvote, the simulator:
   - `POST /api/evolution/trajectories/unmet` to the running temper
     server (the canonical intake), AND
   - `POST /tdata/Evolutions` against the genesis OData (target_app
     = `stackoverflow-agents`, intent = "agents want to downvote
     low-quality answers", autonomy = configurable).
6. Prints a single-line trajectory summary suitable for piping into
   the proof reports under `.proofs/`.

## Modes

| flag | behavior |
|---|---|
| `--dry-run` | print the requests it *would* make, exit 0. No HTTP. |
| `--deterministic` (default) | seeded RNG (`SO_SIM_SEED`, default 42). Identical run shape every time. |
| `--llm` | use `ANTHROPIC_API_KEY` to let Claude pick the next action. Implies non-deterministic. |
| `--no-evolution` | skip the genesis Evolution creation (only emit unmet intent). |
| `--target-only` | just emit the unmet intent — don't create Evolution rows. |

Determinism is the default because the demo needs to be repeatable.

## Configuration

Env vars (all optional, sane defaults):

```
SO_API_BASE        http://127.0.0.1:3000           # temper-platform / temper-server
SO_TENANT_ID       stackoverflow-agents            # X-Tenant-Id header
GENESIS_API_BASE   http://127.0.0.1:3000           # genesis OData base (same process in dev)
GENESIS_TENANT_ID  default                         # tenant under which Evolution rows live
SO_SIM_SEED        42
SO_SIM_QUESTIONS   3                               # # questions to seed
SO_SIM_ANSWERS     3                               # # answers per question
SO_SIM_AGENTS      4                               # # synthetic user-agents
SO_SIM_INTENT_AUTONOMY 0                           # autonomy level on the Evolution row
ANTHROPIC_API_KEY  (required iff --llm)
ANTHROPIC_MODEL    claude-sonnet-4-6
```

## Run

```bash
# 1. dry-run (no HTTP, prints request plan)
node scripts/so-simulator/index.mjs --dry-run

# 2. against a running platform (deterministic)
SO_API_BASE=http://127.0.0.1:3000 node scripts/so-simulator/index.mjs

# 3. LLM-driven user-agents
ANTHROPIC_API_KEY=sk-... node scripts/so-simulator/index.mjs --llm
```

The script exits non-zero only on simulator failure (network unreachable,
malformed CSDL, etc.). The *whole point* of this simulator is that the
downvote attempt fails — that failure is success.

## Deterministic by design

- Seeded RNG (mulberry32) chooses question/answer text and which
  agents act.
- Stable IDs: `sim-q-{seed}-{idx}`, `sim-a-{seed}-{q}-{idx}`.
- No `Date.now()` — wall-clock timestamps are passed through but the
  decision logic does not depend on them.

## Files

- `index.mjs` — the simulator (single file, Node 18+ native fetch, no
  npm deps).
- `lib.mjs` — small helpers (OData wrappers, seeded RNG, scripted
  decisions).
- `README.md` — this file.

## How it integrates with the rest of Phase 1

```
[so-simulator] ──Downvote (404)──▶ [stackoverflow-agents tenant]
       │                                  ▲
       │ POST unmet intent                │ later: hot-deploy of variant
       ▼                                  │ adds Downvote action
[temper-platform: /api/evolution/         │
   trajectories/unmet]                    │
       │                                  │
       ▼                                  │
[genesis: POST /tdata/Evolutions] ──────▶ [Evolution Studio UI]
       │
       ▼
[evolver engine: gen_variant → run_stage_caller → select_winner → merge_variant]
```
