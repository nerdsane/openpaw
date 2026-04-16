# Run 002 Changelog

## Changed Files
1. `os-apps/paw-foresight/wasm/spawn_orchestrator/src/lib.rs` — NEW (recreated from scratch)
2. `os-apps/paw-foresight/wasm/spawn_orchestrator/spawn_orchestrator.wasm` — REBUILT
3. `os-apps/paw-foresight/system/skills/orchestrate-projection/SKILL.md` — updated synthesis

## Critical Discovery

The Run 001 diagnosis was wrong about the root cause. The orchestrator didn't ignore the
template mandates — it NEVER SAW them. The SKILL.md was not installed in TemperFS. The
orchestrator tried `temper.read('/system/skills/orchestrate-projection/SKILL.md')` and got
"file not found" (visible in Run 001 orchestrator transcript, line 3).

The old spawn_orchestrator WASM (source deleted, 221KB binary only) sent a 872-byte
user_message telling the orchestrator to "Read the orchestrate-projection skill from your
available skills." The skill didn't exist, so the orchestrator improvised everything.

## Fix: Rebuild spawn_orchestrator WASM

Created `src/lib.rs` from scratch using `spawn_probes` as a template. The new WASM:
- Embeds the full synthesis instructions in the user_message (6.5KB)
- Includes the data-driven synthesis template (Steps A-G)
- Includes the Quality Rules (7 non-negotiable requirements)
- No longer depends on TemperFS skill file reading

## What Changed

Replaced the advisory "Quality mandates" prose section + vague placeholder template
with a **data-driven synthesis construction** approach.

### Before (Run 001)
The Final Synthesis section had two parts:
1. **Lines 362-397:** Prose "Quality mandates" — 7 numbered instructions telling the
   orchestrator to cite sources, add numbers, name actors, etc.
2. **Lines 398-498:** One big f-string template with bracketed placeholder instructions
   like `[Bulleted list of the strongest convergent observations...]`

The orchestrator read both and ignored them, generating its own preferred narrative structure.

### After (Run 002)
The Final Synthesis section now has:
1. **Steps A-F:** Code blocks that iterate over actual observation and direction entities,
   building each section's content from real data:
   - Step B: Key Findings — iterates `high_obs`, pre-inserts observation content + `[obs: ID]`
   - Step C: Active Directions — pre-inserts full direction reasoning text + obs references
   - Step D: Top 5 Predictions — pre-inserts mandatory falsification field structure
   - Step E: Decision Points — pre-inserts mandatory trigger/options/tradeoffs structure
   - Step F: Surprises — filters observations by challenge keywords, pre-inserts citations
2. **Step G:** Assembles sections into the final string with `[FILL: ...]` markers only
   for parts requiring the orchestrator's analytical judgment

### Key Differences
| Aspect | Before | After |
|--------|--------|-------|
| Obs citations | Prose mandate (ignored) | Pre-inserted by code |
| Direction text | Placeholder instruction | Inserted from entity data |
| Quantitative fields | Prose mandate (ignored) | Mandatory `Measurable indicator:` per finding |
| Falsification | Prose mandate (ignored) | Mandatory field per prediction |
| Decision structure | Prose mandate (ignored) | Pre-built trigger/options/tradeoffs |
| Temporal phases | Placeholder instructions | 4 explicit phases with mandatory revision subsections |

## Diff
174 insertions, 101 deletions in SKILL.md. The entire Final Synthesis section was replaced.
