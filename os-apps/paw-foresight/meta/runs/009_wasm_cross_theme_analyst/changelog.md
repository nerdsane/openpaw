# Run 009 Changelog

## Changed File
`os-apps/paw-foresight/wasm/spawn_orchestrator/src/lib.rs`

## What Changed

Added a WASM-created "cross-theme analyst" session that runs between probe completion
and synthesis. This is a STRUCTURAL change (not prose-based), addressing the failure
pattern observed across 6 runs (003-008) where prose template instructions were ignored.

### 1. New constant: CROSS_THEME_ANALYST_PROMPT
~100 lines of analyst instructions. The analyst session:
- Waits for all 6 probes to complete
- Reads all observations and directions from the API
- Classifies each by theme (governance, tech, economics, org, eval, cross-domain)
- Produces exactly 5 cross-theme interaction entries in structured format
- Writes the analysis to a workspace file
- Returns workspace reference via temper.done()

### 2. Modified ORCHESTRATOR_INSTRUCTIONS
- Added cross-theme analyst session ID (`ORCH_ANALYST_SID`)
- Step 1: Now waits for probes AND analyst (was: probes only)
- New Step 2: Reads analyst's output file from its workspace
- Step 5: Replaces `===CROSS_THEME_CONTENT===` placeholder in template with actual content
- Synthesizer receives pre-populated cross-theme section (not an instruction to generate)

### 3. Modified SYNTHESIS_TEMPLATE
- Step C4: Changed from prose instructions ("generate 4-5 cross-theme interactions")
  to pre-computed content ("cross_theme_section variable contains pre-computed content,
  include AS-IS")
- Step G: Section 5 changed from "THIS SECTION IS MANDATORY, DO NOT SKIP" to
  "PRE-COMPUTED, include cross_theme_section AS-IS"
- Rule #15: Changed from "MUST have 4-5 entries" to "include AS-IS, do NOT regenerate"

### 4. Modified run() function
- Phase 2 (NEW): Creates CrossThemeAnalyst session with probe IDs and projection ID
- Phase 3 (was Phase 2): Creates orchestrator with analyst_sid injected
- Session count: 6 probes + 1 analyst + 1 orchestrator = 8 sessions (was 7)

## Key Design Decision

The cross-theme content is delivered to the synthesizer as a PRE-FILLED PYTHON VARIABLE,
not as a template instruction. The orchestrator replaces the `===CROSS_THEME_CONTENT===`
marker with the analyst's actual output before writing the template to the workspace.
When the synthesizer reads the template, it sees:

```python
cross_theme_section = """## Cross-Theme Interactions

#### Interaction 1: Technical Architecture x Economics/Market
...actual content...
"""
```

This means the synthesizer doesn't need to generate cross-theme reasoning — it just
includes the pre-existing variable in Step G's f-string, exactly like it includes
`finding_section` and `direction_section`.

## Diff Summary
```
+const CROSS_THEME_ANALYST_PROMPT: &str = r###"You are a cross-theme interaction analyst..."###;

 const ORCHESTRATOR_INSTRUCTIONS = ...
+  Cross-theme analyst session ID: ORCH_ANALYST_SID
+  Step 1: Wait for ALL Probes AND the Cross-Theme Analyst
+  Step 2: Read Cross-Theme Analyst Output
+  Step 5: template_text.replace("===CROSS_THEME_CONTENT===", cross_theme_content)

 const SYNTHESIS_TEMPLATE = ...
-  Step C4: Generate 4-5 cross-theme interactions (prose instructions)
+  Step C4: cross_theme_section = """===CROSS_THEME_CONTENT===""" (pre-computed)

 fn run() {
+  // Phase 2: Create cross-theme analyst session
+  let analyst_sid = create_configured_session(..., "CrossThemeAnalyst", ...);
   // Phase 3: Create orchestrator with analyst_sid
   .replace("ORCH_ANALYST_SID", &analyst_sid)
 }
```
