# Run 002 Plan

## Target Criteria

Run 001 diagnosis identified that template quality mandates were IGNORED by the orchestrator.
The prose instructions above the Python code block were treated as advisory. The orchestrator
generated free-form narrative instead of following the mandated structure.

Weakest criteria vs baseline:
- **Specificity (2.0 vs 3.0):** No named companies, tools, dates. Root cause: prose mandate "Name real companies" ignored.
- **Quantitative Precision (1.0 vs 2.0):** No numbers/thresholds. Root cause: prose mandate "include measurable indicator" ignored.
- **Actionability (2.0 vs 2.7):** Flat bullet decision points. Root cause: trigger/options/tradeoffs format mandate ignored.
- **Falsifiability (2.0 vs 2.0):** No falsification conditions. Root cause: "Top 5 Predictions" section mandate ignored.
- **Transparency (1.7 vs 2.0):** No observation citations [obs: ID]. Root cause: citation format mandate ignored.
- **Progression (2.0 vs 2.0):** No temporal phases at all. Root cause: 4-phase structure mandate ignored.

## Root Cause (DEEPER than originally diagnosed)

The Run 001 diagnosis attributed the failure to "advisory mandates." Investigation of the
Run 001 orchestrator transcript revealed a DEEPER root cause:

**The orchestrator NEVER READ the SKILL.md.** The skill file was not in TemperFS. The
orchestrator tried `temper.read('/system/skills/orchestrate-projection/SKILL.md')` and
got "file not found." It then improvised the entire projection from general knowledge.

The old spawn_orchestrator WASM sent a brief 872-byte user_message:
  "Read the orchestrate-projection skill from your available skills"
But the skill was never installed into TemperFS, so the orchestrator improvised everything.

## Planned Change

**ONE change (TWO parts):**

1. **Rebuild spawn_orchestrator WASM** with synthesis instructions embedded directly in
   the user_message (6.5KB instead of 872 bytes). The orchestrator now receives the full
   data-driven synthesis template as its instructions, instead of being told to read a
   missing skill file.

2. **Update SKILL.md** synthesis section from advisory prose to data-driven construction
   (for documentation and future WASM rebuilds).

Files changed:
- `os-apps/paw-foresight/wasm/spawn_orchestrator/src/lib.rs` (NEW — recreated from scratch)
- `os-apps/paw-foresight/wasm/spawn_orchestrator/spawn_orchestrator.wasm` (REBUILT)
- `os-apps/paw-foresight/system/skills/orchestrate-projection/SKILL.md` (updated synthesis)

The change:
1. Remove the prose "Quality mandates" section (lines 362-397) — proven ineffective
2. Replace the template with code that:
   - Builds an observation reference table from actual entities
   - Constructs key findings by iterating observations and adding mandatory fields
   - Constructs temporal progression with mandatory revision subsections
   - Constructs decision points with mandatory trigger/options/tradeoffs fields
   - Constructs top-5 predictions with mandatory falsification fields
   - Adds an assumptions/limitations section
3. The synthesis string is built programmatically from data, not generated as free-form prose

The key insight: when the template IS the code (not prose above the code), the orchestrator
must follow it because it's executing the code, not interpreting suggestions.

## Expected Impact

| Criterion | Run 001 | Expected | Why |
|-----------|---------|----------|-----|
| Specificity | 2.0 | 2.5-3.0 | Obs content already contains named actors; iteration forces them into output |
| Quant Precision | 1.0 | 1.5-2.0 | Mandatory `Measurable indicator:` field per finding forces numbers |
| Actionability | 2.0 | 2.5-3.0 | Trigger/options/tradeoffs structure is code, not suggestion |
| Falsifiability | 2.0 | 2.5-3.0 | Mandatory falsification field per prediction forces conditions |
| Transparency | 1.7 | 2.5-3.0 | Obs IDs inserted by code, not by LLM choice |
| Progression | 2.0 | 2.0-2.5 | 4-phase structure is code; revision subsections are mandatory fields |

Conservative total estimate: 27-30/48 (vs 25.4 Run 001, 27.0 baseline)
