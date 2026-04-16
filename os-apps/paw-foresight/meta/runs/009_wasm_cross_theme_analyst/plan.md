# Run 009 Plan

## Target Criteria

- **Breadth:** Engine Borda 3.0, Baseline Borda 6.0, delta -3.0. Unchanged across Runs 004-008.
  Root cause: synthesis lacks explicit cross-theme interactions. Rubric requires "6+ themes
  with explicit cross-theme interactions where the interaction produces a non-obvious conclusion."
  The engine covers 6 themes independently but doesn't connect them.

## Root Cause Analysis

Six runs of evidence (003-008) prove that **prose-based template instructions are unreliable**:
- Run 003: Diversity constraints in template -> orchestrator ignored
- Run 005: Direction consolidation in template -> orchestrator ignored
- Run 006: Direction consolidation in convergence -> orchestrator skipped
- Run 007: WASM probe theme enforcement -> **FOLLOWED** (WASM works!)
- Run 008: Cross-theme interactions section in template -> synthesizer skipped entirely

Pattern: WASM-level interventions work. Prose-level interventions do not.

## Planned Change

**ONE change:** Create a WASM-spawned "cross-theme analyst" session that runs between
probe completion and synthesis, producing pre-computed cross-theme interactions that are
injected into the synthesis template as DATA (not instructions).

### Architecture

```
WASM creates:
  6 probe sessions (existing)
  1 cross-theme analyst session (NEW)
  1 orchestrator session (modified)

Execution flow:
  Probes run concurrently (existing)
  Cross-theme analyst waits for probes, reads obs/dirs, writes cross-theme file (NEW)
  Orchestrator waits for probes + analyst, reads analyst output (MODIFIED)
  Orchestrator injects cross-theme content into synthesis template as pre-filled variable
  Synthesizer reads template with pre-populated cross-theme section (no instruction to generate)
```

### Key Insight

The synthesizer ignores NEW template instructions but faithfully assembles pre-populated
sections. By moving cross-theme reasoning to a separate WASM-created session and injecting
the output as a pre-filled Python variable in the template, the synthesizer includes it
without needing to generate it.

### Files Changed

- `os-apps/paw-foresight/wasm/spawn_orchestrator/src/lib.rs`:
  1. Add `CROSS_THEME_ANALYST_PROMPT` constant
  2. Modify `ORCHESTRATOR_INSTRUCTIONS` to wait for analyst + inject content
  3. Modify `SYNTHESIS_TEMPLATE` to use pre-filled `cross_theme_section` variable
  4. Modify `run()` to create analyst session after probes

## Expected Impact

- **Breadth:** Should improve from 2.0/4 avg to 3.0+ because the synthesis will contain
  4-5 explicit cross-theme interaction entries with non-obvious conclusions, directly
  satisfying the rubric's 3-level anchor.
- **Other criteria:** Should maintain current levels. Falsifiability (4.0), Progression (3.3),
  Transparency (2.0), Quant Precision (2.0) are driven by existing structural advantages.
- **Risk:** If the orchestrator fails to read the analyst's output or inject it properly,
  the template falls back to an empty cross-theme section (neutral, not worse).
