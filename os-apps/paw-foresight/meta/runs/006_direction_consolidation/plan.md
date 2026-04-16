# Run 006 Plan

## Target Criteria

- **Breadth** (Engine: 3.0/6.0 Borda, Baseline: 6.0/6.0): persistent -3.0 gap unchanged
  across Runs 003-005. Root cause: all 12 directions are governance-themed. The synthesis
  template's Step C direction selection constraint was NOT followed (same failure mode as
  Run 001 with prose mandates).

## Planned Change

**Add direction consolidation to the orchestrator's convergence step.**

Instead of trying to enforce direction diversity in the synthesis template (output-time
prose constraint, proven ineffective in Runs 001 and 005), add a post-convergence step
where the orchestrator **archives excess directions** using the Direction entity's Archive
action. This is structural enforcement:

1. After convergence, the orchestrator classifies each Direction by theme
2. It archives directions that exceed per-theme quotas (max 1 per theme, max 5 total)
3. The synthesis template already queries `$filter=Status ne 'Archived'` — it will only
   see the remaining 5 diverse directions
4. The synthesis doesn't need to "choose" to follow a prose constraint — there are
   simply fewer directions available

**File changed:** `os-apps/paw-foresight/wasm/spawn_orchestrator/src/lib.rs`
(ORCHESTRATION_INSTRUCTIONS constant, convergence section)

**Why this works better than Run 005's approach:**
- Run 005 added diversity constraints to the SYNTHESIS template (output-time, advisory)
- Run 006 adds direction archival to the CONVERGENCE step (entity-management time, structural)
- Once archived, directions are physically excluded by the OData filter
- The orchestrator doesn't need to resist including all 12 — there are only 5 to include

## Expected Impact

- **Breadth:** Should improve from 3.0 to 4.5-6.0 Borda (diverse directions → diverse output)
- **Actionability:** May improve indirectly (diverse directions produce diverse decision points)
- **Other criteria:** Should maintain current levels (structural change, not prompt change)
- **Risk:** If probes generate governance-only directions, consolidation will still pick
  governance themes. Mitigated by the existing probe differentiation (practitioner/critic/
  adjacent-domain personas) which should produce at least 3-4 distinct themes.
