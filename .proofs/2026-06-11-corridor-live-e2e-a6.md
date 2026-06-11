# Proof: Live corridor e2e on OpenAI Codex (A6 flagship — in progress)

**Date:** 2026-06-11
**Branch:** codex/corridor-engine
**Provider:** openai_codex (ChatGPT subscription, device-login OAuth), model gpt-5.5
**World:** "AI coding tools — six months out" (target 2026-12-11), endpoint budget 2

## What ran live (run 8, world en-019eb7a3-56a8…)

1. **Seed** — surveyor created **15 determined skeleton nodes** (EU AI Act
   application timeline, Cyber Resilience Act reporting obligations, dated
   commitments), wrote the skeleton snapshot to PawFS, self-reported
   SeedComplete; bookmaker imported **8 market-priced nodes**. World Active
   with 23 EventNodes.
2. **Corridor** — endpoint writers produced December-2026 document bundles
   under modal/anti-modal stances; repairers worked them backward (authored
   intermediate EventNodes); adversaries challenged; aggregate_costs settled
   the pass deterministically: **canonical path en-019eb7aa-60b3…**.
3. **Forecasts** — **20 preregistered** (engine 0.2.0) from market + authored
   nodes inside the frontier; determined facts correctly excluded.
4. **Render** — renderer authored two artifacts with full inline citations:
   "Decision Brief: Governed AI Coding Agents" (23 cited nodes) and "Board
   Memorandum: AI Coding Tools Spend and Governance" (21 cited nodes, dated
   2026-12-11, real content verified via $value).
5. **Gate** — checkers ran and returned honest verdicts each round (never a
   silent pass). Final clean pass pending run 9 (below).

## The failure ladder (every wall found, fixed at root, pinned where testable)

| # | Wall | Fix |
|---|------|-----|
| 1 | Anthropic key out of credit (billing 400) | Switched to the repo-standard OpenAI Codex OAuth (device login) |
| 2 | gpt-5.4-codex unsupported on ChatGPT accounts | gpt-5.5 (matches the user's Codex CLI default) |
| 3 | WASM with no follow-up dispatch read as failure | Empty-action success result on all no-dispatch paths |
| 4 | OData row shapes (envelope vs PascalCase) | Dual-shape readers in five modules + DSF mapper; unit-pinned |
| 5 | Sessions had no workspace → PawFS writes denied | Per-world workspace threaded into every Configure |
| 6 | Workspace rows unreadable by agents → lookup impossible | Create-only workspace per spawn batch |
| 7 | paw-fs File permit compared nonexistent resource.workspaceId | Fixed to resource.workspace_id (permit was dead code platform-wide) |
| 8 | Session entity-actions arrive as service:wasm-runtime | Relay permits for session self-reports (v1 trust model, documented); File-create + Path-update decisions approved via the governed decisions API |
| 9 | Checker sessions cannot read cross-workspace/directory files | Gate inlines artifact content into the checker prompt (30KB cap, loud truncation) |
| 10 | Gate phase split keyed on stale verdict_json after resubmits | Split on ctx.trigger_action |
| 11 | Rebuilt WASM never rebinds on a persistent store | Platform bug filed (repro included); fresh store for run 9 |

## Operational notes for A7 (prod runbook additions)

- Approve the same two decision classes on prod after install (File create
  for service:wasm-runtime; Path update for system principals) — or land the
  policy equivalents in the bundle first.
- Verify prod deploys actually rebind WASM artifacts (platform bug #2 above
  suggests persistent stores may pin first-installed modules; proofs 057/074
  imply prod image deploys differ — confirm before trusting the upgrade).
- Codex device login on prod happens via the same /paw/setup/openai-codex
  endpoints (or Discord re-auth flow per ADR-009..016).

## Pending (run 9, fresh store, all current modules)

- Full clean pass end-to-end including the consistency gate verdict on real
  content, the ~2045 fiction-path world, and the 2025-vantage hindcast
  (fixtures committed at scripts/fixtures/hindcast-2025h2/).
- Session cost accounting (cost_cents) recorded once a run completes within
  a single store generation.
