//! Spawn Orchestrator — WASM module for the Projection.Start integration.
//!
//! Run 009: Cross-theme analyst session added between probes and synthesis.
//! 6 runs proved prose template instructions unreliable (Runs 003-008).
//! WASM-level interventions work (Run 007 theme enforcement followed).
//! Now WASM creates a dedicated cross-theme analyst session that reads all
//! observations/directions, produces structured cross-theme interactions,
//! and writes them to a file. The orchestrator reads this file and injects
//! the content into the synthesis template as a pre-filled variable —
//! the synthesizer includes it without needing to generate it.
//!
//! Creates: 6 probe sessions (theme-constrained) + 1 cross-theme analyst + 1 orchestrator.
//! Probes run concurrently. Analyst waits for probes. Orchestrator waits for both.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Probe prompt template. Placeholders use UPPERCASE_WITH_UNDERSCORES to avoid
/// collision with Python braces. Filled via String::replace() per probe.
const PROBE_PROMPT_TEMPLATE: &str = r###"You are a PROBE_ROLE foresight probe analyzing Directed Software Evolution.
IMPORTANT: Use the execute tool for ALL entity operations via temper.* calls.

Projection ID: PROBE_PROJ_ID
ForesightModel ID: PROBE_FM_ID
Model Name: PROBE_FM_NAME
Step: PROBE_STEP of 2
Time Range: PROBE_TIME_RANGE
Horizon: PROBE_HORIZON

## Domain Context

Directed Software Evolution applies evolutionary principles to software development:
generating multiple code variants, evaluating them under selection pressure (tests,
benchmarks, policy gates), and retaining successful patterns. Key themes in the domain
include: coding agent adoption and tooling, evaluation and verification maturity,
governance and compliance requirements, enterprise readiness and procurement, organizational
adaptation and workforce impact, cross-domain analogies (biology, finance, manufacturing),
platform architecture patterns, and vendor economics.

## Your Persona

PROBE_PERSONA

## Setup

```python
# Read ForesightModel for context
fmodel = temper.get("ForesightModels", "PROBE_FM_ID")
fm_fields = fmodel.get("fields", {})
print("Model:", fm_fields.get("name", ""), "| Signals:", fm_fields.get("signal_count", ""))

# Search for current domain intelligence
results = temper.web_search("PROBE_SEARCH_TERMS")
for r in results[:5]:
    print(r.get("title", ""), "-", r.get("snippet", "")[:200])
```

Study the search results carefully. They provide current signals about the domain.

## Task: Create 3-5 Observations and Exactly 2 Directions

### Observations (create 3-5)

For each observation about what is happening or will happen in the PROBE_TIME_RANGE period:

```python
obs = temper.create("Observations", {})
obs_id = obs["entity_id"]
temper.action("Observations", obs_id, "Record", {
    "probe_agent_id": temper.get_agent_id(),
    "projection_id": "PROBE_PROJ_ID",
    "step_at": "PROBE_STEP",
    "content": "Detailed observation (2-3 sentences). Name specific companies, tools, thresholds, or measurable indicators.",
    "importance": "high",
    "signal_refs": "[]",
    "counterfactual": "What happens if this observation is ignored"
})
print(f"Created observation: {obs_id}")
```

Create each observation in a SEPARATE execute call.

### Directions (create exactly 2)

**THEME CONSTRAINT (MANDATORY):** PROBE_THEME_CONSTRAINT

Each direction title MUST clearly reflect the required theme category.

```python
import json as _json

d = temper.create("Directions", {})
d_id = d["entity_id"]
temper.action("Directions", d_id, "Propose", {
    "title": "One-sentence direction title that clearly reflects the required theme",
    "proposer_agent_id": temper.get_agent_id(),
    "projection_id": "PROBE_PROJ_ID",
    "reasoning": "2-3 paragraphs of analysis grounded in your observations and search findings",
    "grounding": "[]",
    "observation_ids": _json.dumps(["first_obs_id", "second_obs_id"]),
    "counterfactual_summary": "What happens if this direction is wrong",
    "step_at": "PROBE_STEP"
})
print(f"Created direction: {d_id}")
```

Create each direction in a SEPARATE execute call. Use real observation IDs from the observations you created above.

When all observations and directions are created:
```python
temper.done({"status": "ok", "probe": "PROBE_ROLE", "step": "PROBE_STEP"})
```
"###;

/// Cross-theme analyst — WASM-created session that runs after probes complete.
/// Reads all observations and directions, identifies cross-theme pairs,
/// and writes structured interaction entries to a workspace file.
/// The orchestrator reads this file and injects it into the synthesis template.
const CROSS_THEME_ANALYST_PROMPT: &str = r###"You are a cross-theme interaction analyst for a foresight projection.
IMPORTANT: Use the execute tool for ALL entity operations via temper.* calls.

Projection ID: ANALYST_PROJ_ID

## Step 1: Wait for All Probes to Complete

6 probe sessions are running concurrently. Wait for ALL to finish before proceeding.

```python
import time

probe_ids = [ANALYST_PROBE_IDS_PYTHON]

for attempt in range(60):
    all_done = True
    for pid in probe_ids:
        s = temper.get("Sessions", pid)
        status = s.get("status", "")
        result = s.get("fields", {}).get("result", "")
        if status not in ("Completed", "Failed") and (not result or len(str(result)) < 10):
            all_done = False
    print(f"Attempt {attempt+1}: all_done={all_done}")
    if all_done:
        print("All probes complete!")
        break
    time.sleep(15)
```

## Step 2: Read All Observations and Directions

```python
import json as _json

projection_id = "ANALYST_PROJ_ID"
all_obs = temper.list("Observations", "$filter=projection_id eq '" + projection_id + "'")
all_dirs = temper.list("Directions", "$filter=projection_id eq '" + projection_id + "'")

obs_data = {}
for obs in all_obs:
    oid = obs["entity_id"]
    f = obs.get("fields", {})
    obs_data[oid] = {
        "content": f.get("content", ""),
        "probe_id": f.get("probe_agent_id", ""),
        "importance": f.get("importance", "medium"),
    }

dir_data = {}
for d in all_dirs:
    did = d["entity_id"]
    f = d.get("fields", {})
    try:
        obs_ids = _json.loads(f.get("observation_ids", "[]")) if isinstance(f.get("observation_ids"), str) else f.get("observation_ids", [])
    except:
        obs_ids = []
    dir_data[did] = {
        "title": f.get("title", ""),
        "reasoning": f.get("reasoning", ""),
        "obs_ids": obs_ids if isinstance(obs_ids, list) else [],
    }

print(f"Loaded {len(obs_data)} observations, {len(dir_data)} directions")
```

## Step 3: Classify Observations by Theme

The 6 probes were themed:
- Practitioner probes -> technical architecture, evaluation/testing
- Critic probes -> economics/market, organizational/adoption
- Adjacent probes -> cross-domain analogies

Classify each observation by reading its content. Assign exactly ONE theme:
- governance/policy
- technical architecture
- economics/market
- organizational/adoption
- evaluation/testing
- cross-domain

Group observations by theme.

## Step 4: Produce Exactly 5 Cross-Theme Interaction Entries

Each interaction connects TWO DIFFERENT themes and derives a NON-OBVIOUS conclusion.

Use this EXACT format for each entry:

```
#### Interaction N: [Theme A] x [Theme B]
**Themes connected:** [theme A] + [theme B]
**Observation bridge:** [obs: OBS_ID_FROM_THEME_A] + [obs: OBS_ID_FROM_THEME_B]

[2-3 sentences explaining HOW these two themes interact. Name specific companies,
tools, or mechanisms (Anthropic, OpenAI, Cursor, Cedar, OPA, Kubernetes, Temper, etc.).
The conclusion MUST be something neither theme implies alone.]

**Non-obvious conclusion:** [One specific, falsifiable prediction that ONLY emerges
from combining these two themes. Include a date or quantitative threshold.]

**Implication:** [What a decision-maker should do differently because of this interaction]
```

MANDATORY RULES:
- Exactly 5 interaction entries
- Each MUST connect 2 DIFFERENT themes (no same-theme pairs)
- Each of the 5 MUST use a DIFFERENT theme pair (no repeated pairs)
- All 6 themes must appear across the 5 interactions
- Each MUST cite real observation IDs (one per theme)
- At least 1 interaction MUST involve cross-domain
- At least 1 interaction MUST involve economics/market
- At least 1 interaction MUST involve governance/policy
- Non-obvious conclusions must be SPECIFIC (with dates, thresholds, or named actors)

## Step 5: Write to File

```python
import json as _json

analysis_text = "## Cross-Theme Interactions\n\n" + all_interactions_text
result = temper.write("cross_theme_analysis.md", analysis_text)
ws_id = result.get("workspace_id", "")
file_id = result.get("file_id", "")
temper.done(_json.dumps({"workspace_id": ws_id, "file_id": file_id, "path": "cross_theme_analysis.md"}))
```
"###;

/// Orchestrator instructions — modified for Run 009.
/// Probes are already running (created by WASM). Cross-theme analyst also running.
/// Orchestrator waits for both, reads analyst output, injects into template, delegates synthesis.
const ORCHESTRATOR_INSTRUCTIONS: &str = r###"You are orchestrating a foresight projection. Follow these steps exactly.

## IMPORTANT: Probes AND Cross-Theme Analyst Are Already Running

6 probe sessions and 1 cross-theme analyst session have been created by WASM.
They are running concurrently. You MUST wait for ALL of them.

Probe session IDs:
ORCH_PROBE_IDS_LIST

Cross-theme analyst session ID: ORCH_ANALYST_SID

## Step 1: Wait for ALL Probes AND the Cross-Theme Analyst to Complete

This is your FIRST and MOST IMPORTANT task. Do NOT proceed until ALL are done.

```python
import time
import json as _json

probe_ids = [ORCH_PROBE_IDS_PYTHON]
analyst_sid = "ORCH_ANALYST_SID"
all_session_ids = probe_ids + [analyst_sid]

for attempt in range(60):
    all_done = True
    statuses = {}
    for sid in all_session_ids:
        s = temper.get("Sessions", sid)
        status = s.get("status", "")
        result = s.get("fields", {}).get("result", "")
        statuses[sid] = status
        if status not in ("Completed", "Failed") and (not result or len(str(result)) < 10):
            all_done = False
    print(f"Attempt {attempt+1}: {statuses}")
    if all_done:
        print("All probes and analyst complete!")
        break
    time.sleep(15)
```

## Step 2: Read Cross-Theme Analyst Output

The analyst wrote a cross-theme analysis file. Read it from the analyst's workspace.

```python
analyst_session = temper.get("Sessions", analyst_sid)
analyst_result_raw = analyst_session.get("fields", {}).get("result", "{}")
try:
    analyst_result = _json.loads(analyst_result_raw)
    analyst_ws = analyst_result.get("workspace_id", "")
    analyst_path = analyst_result.get("path", "cross_theme_analysis.md")
except:
    analyst_ws = ""
    analyst_path = "cross_theme_analysis.md"

cross_theme_content = ""
if analyst_ws:
    try:
        cross_theme_content = temper.read("/" + analyst_path, {"workspace_id": analyst_ws})
        print(f"Read cross-theme analysis: {len(cross_theme_content)} chars")
    except Exception as e:
        print(f"Failed to read analyst output: {e}")
        cross_theme_content = ""

if not cross_theme_content or len(cross_theme_content) < 100:
    cross_theme_content = "## Cross-Theme Interactions\n\n(Cross-theme analyst session did not produce output. Skip this section.)\n"
    print("WARNING: No cross-theme analysis available")
```

## Step 3: Load Domain Knowledge

```python
fmodel = temper.get("ForesightModels", "ORCH_FM_ID")
print("Model:", fmodel.get("fields", {}).get("name", ""))

results = temper.web_search("Directed Software Evolution coding agents governance evaluation 2026")
for r in results[:5]:
    print(r.get("title", ""))
```

## Step 4: Read All Observations and Directions

```python
projection_id = "ORCH_PROJ_ID"
all_obs = temper.list("Observations", "$filter=projection_id eq '" + projection_id + "'")
all_dirs = temper.list("Directions", "$filter=projection_id eq '" + projection_id + "'")
print(f"Loaded {len(all_obs)} observations, {len(all_dirs)} directions")

for d in all_dirs:
    f = d.get("fields", {})
    print(f"Direction: {f.get('title', '')} | Step: {f.get('step_at', '')}")
```

## Step 5: Store Synthesis Template WITH Cross-Theme Content

Inject the cross-theme analyst's output into the template before storing.

```python
template_text = """PASTE THE EXACT TEXT BETWEEN ===SYNTHESIS_TEMPLATE=== AND ===END_SYNTHESIS_TEMPLATE=== HERE"""

# Inject pre-computed cross-theme analysis into the template
template_text = template_text.replace("===CROSS_THEME_CONTENT===", cross_theme_content)

_tf = temper.write("synthesis_template.md", template_text)
_ws = _tf["workspace_id"]
print(f"Template stored with cross-theme content injected ({len(template_text)} chars)")
```

## Step 6: Delegate Synthesis to a Dedicated Session

Create a synthesis session with clean context. DO NOT synthesize in this session —
the accumulated polling context will overflow the WASM context parser.

```python
synth_agent = temper.create("Agents", {"Name": "Synthesizer", "Role": "synthesizer"})
synth_session = temper.create("Sessions", {"agent_id": synth_agent["entity_id"]})
synth_sid = synth_session["entity_id"]

synth_prompt = (
    "You are synthesizing a foresight projection. Follow the template EXACTLY.\n\n"
    "Projection ID: " + projection_id + "\n\n"
    "## Setup\n\n"
    "FIRST, read the synthesis template from the orchestrator workspace:\n\n"
    "  template = temper.read('/synthesis_template.md', {'workspace_id': '" + _ws + "'})\n\n"
    "The template contains a pre-computed Cross-Theme Interactions section.\n"
    "Include it AS-IS in the final synthesis. Do NOT regenerate or modify it.\n\n"
    "Follow the synthesis template step by step.\n"
    "Load observations and directions from the API using temper.list().\n\n"
    "Write the complete synthesis to a file with temper.write().\n"
    "Then dispatch Complete on the Projection:\n"
    "  temper.action('Projections', '" + projection_id + "', 'Complete', {})\n"
    "Then call temper.done() with the synthesis file reference.\n"
)

temper.action("Sessions", synth_sid, "Configure", {
    "user_message": synth_prompt,
    "model": "gpt-5.4",
    "provider": "openai_codex",
    "max_turns": "50",
    "tools_enabled": "temper_get,temper_list,temper_action,temper_create,temper_write,temper_read",
    "sandbox_url": "none"
})
```

## Step 7: Wait for Synthesis and Complete

```python
import time
for i in range(60):
    time.sleep(30)
    s = temper.get("Sessions", synth_sid)
    status = s.get("status", "")
    result_field = s.get("fields", {}).get("result", "")
    print(f"Synthesis attempt {i+1}: status={status}, result_len={len(str(result_field))}")
    if status in ("Completed", "Failed") or (result_field and len(str(result_field)) > 200):
        break

if status == "Failed":
    temper.action("Projections", projection_id, "Fail", {"error_message": "Synthesis session failed"})
    temper.done("Synthesis session failed: " + synth_sid)
else:
    temper.done("Projection complete. Synthesis session: " + synth_sid)
```
"###;

/// The synthesis template — unchanged from Run 006.
/// Embedded between markers in the orchestrator's user_message.
/// The orchestrator writes this to a workspace file. The synthesis session reads it.
const SYNTHESIS_TEMPLATE: &str = r###"## Foresight Synthesis Template

Follow these steps to build the synthesis from observation and direction data.

### Step A: Load Data

```python
import json as _json

all_obs = temper.list("Observations", "$filter=projection_id eq '" + projection_id + "'")
all_dirs = temper.list("Directions",
    "$filter=projection_id eq '" + projection_id + "' and Status ne 'Archived'")

obs_data = {}
for obs in all_obs:
    oid = obs["entity_id"]
    f = obs.get("fields", {})
    obs_data[oid] = {
        "content": f.get("content", ""),
        "importance": f.get("importance", "medium"),
        "step": f.get("step_at", "0"),
    }

dir_data = {}
for d in all_dirs:
    did = d["entity_id"]
    f = d.get("fields", {})
    obs_ids_raw = f.get("observation_ids", "[]")
    try:
        obs_ids = _json.loads(obs_ids_raw) if isinstance(obs_ids_raw, str) else obs_ids_raw
    except:
        obs_ids = []
    dir_data[did] = {
        "title": f.get("title", ""),
        "reasoning": f.get("reasoning", ""),
        "obs_ids": obs_ids if isinstance(obs_ids, list) else [],
        "counterfactual": f.get("counterfactual_summary", ""),
    }

high_obs = [(oid, o) for oid, o in obs_data.items() if o["importance"] == "high"]
all_obs_ids = list(obs_data.keys())
```

Also read the analysis handoff file if available. It contains:
- convergence_findings: key agreements across probes
- cross_probe_tensions: where probes disagreed
- source_thesis_challenges: ways observations challenge the source essay

Use these to inform the narrative, especially Temporal Progression and Source Thesis Challenges.

### Step B: Build Key Findings

For each high-importance observation (up to 8), create a finding entry with this EXACT format:

```
N. **[one-sentence finding naming a real company, tool, or standard]**
   - Evidence: "[observation content excerpt]" [obs: OBS_ID]
   - Measurable indicator: [a specific number, adoption %, threshold, or data source]
   - Theme: [one of: model/vendor, governance/policy, organizational/adoption, technical architecture, economics/market, evaluation/testing, cross-domain]
```

The observation content and [obs: ID] citation MUST come from the actual data.
The finding sentence MUST name real companies/tools (Anthropic, OpenAI, Cursor, Cognition/Devin,
Kubernetes, Cedar, OPA, Temper, etc.) — NOT generic categories like "companies" or "teams".

**DIVERSITY MANDATE (NON-NEGOTIABLE):**
- Key Findings MUST span at least 4 distinct themes from: model/vendor, governance/policy,
  organizational/adoption, technical architecture, economics/market, evaluation/testing, cross-domain.
- No more than 2 findings may share the same primary theme.
- At least 2 findings must derive from the adjacent-domain probe's observations.
- At least 1 finding must derive from the critic probe's observations.
- No single observation ID may appear in more than 2 findings. Use at least 60% of all
  available observations across the full synthesis (not just high-importance ones).

### Step C: Select & Consolidate Active Directions (BREADTH-CRITICAL)

**DO NOT dump all directions.** Too many directions on the same theme creates perceived
monothematic output. You MUST select and consolidate.

**Step C1: Classify each direction by primary theme.**
Assign EXACTLY ONE theme to each direction from this list:
- governance/policy
- technical architecture
- economics/market
- organizational/adoption
- evaluation/testing
- cross-domain

**Step C2: Select at most 5 directions spanning at least 4 distinct themes.**
Rules:
- Maximum 2 directions per theme. If a theme has 3+, merge the strongest into one.
- At least 1 direction MUST be about economics/market or cross-domain (not governance).
- At least 1 direction MUST be about technical architecture (not governance).
- If governance/policy has the most directions, it gets at most 1 slot.

**Step C3: For each selected direction (max 5), write:**

```
#### [Direction Title]
**Direction ID:** [direction entity ID]
**Theme:** [primary theme from Step C1]

[Full reasoning text from direction entity — do NOT truncate for selected directions]

Supporting observations: [obs: ID1], [obs: ID2]

**Counterfactual:** [counterfactual from direction entity]
```

If merging directions: combine reasoning into a single entry, cite all supporting
observation IDs, and keep the strongest counterfactual. Title the merged direction
to reflect its broader scope.

**Step C4: Cross-Theme Interactions (PRE-COMPUTED — DO NOT REGENERATE)**

The cross-theme interactions have been pre-computed by a dedicated analyst session
and injected into this template. They are stored in the `cross_theme_section` variable below.

```python
# Cross-theme content was pre-computed and injected by the orchestrator.
# DO NOT modify, regenerate, or skip this section. Include it AS-IS in Step G.
cross_theme_section = """===CROSS_THEME_CONTENT==="""
```

If the cross_theme_section is empty or says "did not produce output", include a brief
note in that section and move on. Do NOT attempt to generate cross-theme interactions yourself.

### Step D: Build Top 5 Predictions

For each of the top 5 directions, create a prediction with this EXACT format:

```
N. **Prediction:** [specific, dated prediction derived from the direction]
   - **Measurable indicator:** [quantitative threshold, adoption %, or proxy metric]
   - **Confidence:** [high/medium/low]
   - **Falsification:** If [observable condition] has not occurred by [specific date],
     this prediction is wrong because [mechanism]
   - **Supporting observations:** [obs: ID1], [obs: ID2]
```

EVERY prediction MUST have a falsification condition with a specific date.

### Step E: Build Decision Points

Create exactly 3 decision points with this EXACT format:

```
#### Decision Point N
- **Decision:** [what must be decided — name the specific tool, config, or org action]
- **Timing trigger:** [observable event with approximate date]
- **Option A:** [name a specific tool/config/action, e.g. "deploy Cedar policy gates on CI pipelines"]
  — **Tradeoff:** [specific cost in engineering-weeks, dollar amount, or named risk]
- **Option B:** [name a specific tool/config/action]
  — **Tradeoff:** [specific cost in engineering-weeks, dollar amount, or named risk]
- **Option C:** [name a specific tool/config/action]
  — **Tradeoff:** [specific cost in engineering-weeks, dollar amount, or named risk]
- **Recommended:** [which option and one-sentence justification]
```

**ACTIONABILITY RULE:** Each option MUST name a specific tool, configuration, platform, or
organizational action. "Invest in governance" is NOT acceptable. "Deploy OPA/Cedar policy-as-code
gates on the CI pipeline by Q3 2026" IS acceptable. Each tradeoff MUST include an estimated
effort level (e.g., "2-4 engineering-weeks", "$50K-100K annual", "requires dedicated platform team").

### Step F: Source Thesis Challenges

This section is CRITICAL for the Challenge criterion. Include 3-5 items that directly
challenge claims in the source knowledge graph. Each MUST:

- **Name the specific claim** being challenged (quote or paraphrase the source essay's thesis)
- **Explain the mechanism** by which the claim fails, is overstated, or has a blind spot
- **Cite evidence** from probe observations: [obs: ID]
- At least 1 challenge MUST use evidence from OUTSIDE the source material (external signals,
  cross-domain analogies, or data the source does not contain)
- At least 1 challenge MUST contradict a claim the source presents with high confidence

Use the "source_thesis_challenges" from the analysis handoff as starting points, but develop
them with full mechanism-level reasoning.

### Step G: Assemble Complete Synthesis

The MANDATORY section order is:

1. Executive Summary (3 paragraphs, cite obs IDs, name 6+ companies/tools across 3+ categories, include quant claims)
2. Key Findings (from Step B)
3. Temporal Progression (4 phases: 0-3mo, 3-6mo, 6-9mo, 9-12mo)
   - Phases 2-4 MUST have a "Revisions to earlier predictions" subsection
   - Each revision must explain WHAT changed and WHY — not formulaic confirm/qualify/revise
4. Active Directions (from Step C, Steps C1-C3)
5. Cross-Theme Interactions (from Step C4 — PRE-COMPUTED, include cross_theme_section AS-IS)
6. Source Thesis Challenges (from Step F)
7. Top 5 Predictions with Falsification Criteria (from Step D)
8. Decision Points (from Step E)
9. Assumptions & Limitations (3 assumptions with If-wrong and Confidence)
10. Methodology

Write the complete synthesis to a file:
```python
result = temper.write("projection_synthesis_" + projection_id + ".md", synthesis)
temper.action("Projections", projection_id, "Complete", {})
temper.done("Projection complete. Synthesis: " + result["file_id"])
```

## Quality Rules (NON-NEGOTIABLE)

1. Every substantive claim MUST cite an observation: [obs: en-XXXXX]
2. Every finding MUST include a measurable indicator (number, %, threshold)
3. Every prediction MUST have a falsification condition with a date
4. Every decision point MUST have trigger + options + tradeoffs
5. Name real companies/tools, NOT generic categories
6. Temporal phases 2-4 MUST revise earlier predictions with genuine reasoning
7. Do NOT skip any section. Do NOT rearrange the structure.

## Content Diversity Rules (NON-NEGOTIABLE)

8. Key Findings MUST span at least 4 distinct themes. Max 2 findings per theme.
9. No single observation may be cited in more than 2 Key Findings.
10. Use at least 60% of available observations across the full synthesis.
11. At least 2 findings must come from adjacent-domain probe observations.
12. Decision Point options MUST name specific tools/configs/actions with effort estimates.
13. Executive Summary MUST name 6+ distinct entities across 3+ categories
    (e.g., vendors: Anthropic/OpenAI/Cursor; governance: Cedar/OPA/Sentinel;
    open-source: Aider/Cline/OpenHands; platforms: Kubernetes/Terraform/Temper).
14. Temporal Progression phases must each introduce at least 1 NEW company or tool
    not mentioned in prior phases.
15. Cross-Theme Interactions section is PRE-COMPUTED. Include the cross_theme_section
    variable AS-IS in the synthesis. Do NOT regenerate or modify it.
"###;

/// Create an Agent + Session, configure the session, and return the session ID.
fn create_configured_session(
    ctx: &Context,
    headers: &[(String, String)],
    base_url: &str,
    agent_name: &str,
    agent_role: &str,
    user_message: &str,
    model: &str,
    provider: &str,
    tools: &str,
    max_turns: &str,
) -> Result<String, String> {
    // Create Agent
    let agent_url = format!("{base_url}/tdata/Agents");
    let agent_body = json!({
        "Name": agent_name,
        "Role": agent_role
    });
    let agent_resp = ctx.http_call("POST", &agent_url, headers, &agent_body.to_string())?;
    if agent_resp.status < 200 || agent_resp.status >= 300 {
        return Err(format!(
            "Failed to create Agent '{}' (HTTP {}): {}",
            agent_name,
            agent_resp.status,
            &agent_resp.body[..agent_resp.body.len().min(200)]
        ));
    }
    let agent: Value = serde_json::from_str(&agent_resp.body)
        .map_err(|e| format!("Failed to parse Agent '{}' response: {e}", agent_name))?;
    let agent_id = agent
        .get("entity_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Agent '{}' creation did not return entity_id", agent_name))?;

    // Create Session
    let session_url = format!("{base_url}/tdata/Sessions");
    let session_body = json!({"agent_id": agent_id});
    let session_resp = ctx.http_call("POST", &session_url, headers, &session_body.to_string())?;
    if session_resp.status < 200 || session_resp.status >= 300 {
        return Err(format!(
            "Failed to create Session for '{}' (HTTP {})",
            agent_name, session_resp.status
        ));
    }
    let session: Value = serde_json::from_str(&session_resp.body)
        .map_err(|e| format!("Failed to parse Session response for '{}': {e}", agent_name))?;
    let session_id = session
        .get("entity_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Session for '{}' did not return entity_id", agent_name))?
        .to_string();

    // Configure Session
    let configure_url =
        format!("{base_url}/tdata/Sessions('{session_id}')/OpenPaw.Configure");
    let configure_body = json!({
        "model": model,
        "provider": provider,
        "tools_enabled": tools,
        "max_turns": max_turns,
        "user_message": user_message,
        "sandbox_url": "none",
        "temper_api_url": base_url
    });
    let configure_resp =
        ctx.http_call("POST", &configure_url, headers, &configure_body.to_string())?;
    if configure_resp.status < 200 || configure_resp.status >= 300 {
        return Err(format!(
            "Configure failed for Session '{}' / Agent '{}' (HTTP {}): {}",
            session_id,
            agent_name,
            configure_resp.status,
            &configure_resp.body[..configure_resp.body.len().min(300)]
        ));
    }

    ctx.log(
        "info",
        &format!(
            "spawn_orchestrator: created {} session {} ({} bytes)",
            agent_name,
            session_id,
            user_message.len()
        ),
    );

    Ok(session_id)
}

/// Build a probe prompt from the template, filling in probe-specific values.
fn build_probe_prompt(
    role: &str,
    step: &str,
    time_range: &str,
    persona: &str,
    search_terms: &str,
    theme_constraint: &str,
    projection_id: &str,
    foresight_model_id: &str,
    fm_name: &str,
    horizon: &str,
) -> String {
    PROBE_PROMPT_TEMPLATE
        .replace("PROBE_ROLE", role)
        .replace("PROBE_PROJ_ID", projection_id)
        .replace("PROBE_FM_ID", foresight_model_id)
        .replace("PROBE_FM_NAME", fm_name)
        .replace("PROBE_STEP", step)
        .replace("PROBE_TIME_RANGE", time_range)
        .replace("PROBE_HORIZON", horizon)
        .replace("PROBE_PERSONA", persona)
        .replace("PROBE_SEARCH_TERMS", search_terms)
        .replace("PROBE_THEME_CONSTRAINT", theme_constraint)
}

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "spawn_orchestrator v009: starting (WASM cross-theme analyst)");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // Read Projection fields
        let foresight_model_id = fields
            .get("foresight_model_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let horizon = fields
            .get("horizon")
            .and_then(|v| v.as_str())
            .unwrap_or("1 year");
        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        if foresight_model_id.is_empty() {
            return Err("spawn_orchestrator: foresight_model_id is required".to_string());
        }

        // Read config
        let temper_api_url = ctx
            .config
            .get("temper_api_url")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());

        let tenant = &ctx.tenant;
        let headers: Vec<(String, String)> = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-tenant-id".to_string(), tenant.to_string()),
            (
                "x-temper-principal-kind".to_string(),
                "agent".to_string(),
            ),
            (
                "x-temper-principal-id".to_string(),
                ctx.entity_id.clone(),
            ),
            (
                "x-temper-agent-type".to_string(),
                "system".to_string(),
            ),
        ];

        // Read ForesightModel for context
        let fm_url = format!(
            "{temper_api_url}/tdata/ForesightModels('{foresight_model_id}')"
        );
        let fm_resp = ctx.http_call("GET", &fm_url, &headers, "")?;
        let (fm_name, seed_model, seed_provider) =
            if fm_resp.status >= 200 && fm_resp.status < 300 {
                let fm: Value =
                    serde_json::from_str(&fm_resp.body).unwrap_or(json!({}));
                let f = fm.get("fields").cloned().unwrap_or(json!({}));
                let name = f
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let model = f
                    .get("seed_model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("gpt-5.4")
                    .to_string();
                let provider = f
                    .get("seed_provider")
                    .and_then(|v| v.as_str())
                    .unwrap_or("openai")
                    .to_string();
                (name, model, provider)
            } else {
                ctx.log(
                    "warn",
                    &format!(
                        "spawn_orchestrator: failed to fetch ForesightModel (HTTP {})",
                        fm_resp.status
                    ),
                );
                (
                    "unknown".to_string(),
                    "gpt-5.4".to_string(),
                    "openai".to_string(),
                )
            };

        let provider_codex = format!("{seed_provider}_codex");

        ctx.log(
            "info",
            &format!(
                "spawn_orchestrator: projection {} model {} ({}) horizon {}",
                entity_id, foresight_model_id, fm_name, horizon
            ),
        );

        // ========================================================
        // Phase 1: Create 6 probe sessions with theme constraints
        // ========================================================

        // Probe configurations: (name, role, step, time_range, persona, search_terms, theme_constraint)
        let probe_specs: Vec<(&str, &str, &str, &str, &str, &str, &str)> = vec![
            (
                "Practitioner-S0",
                "practitioner",
                "0",
                "0-180 days",
                "a senior software engineer who builds and deploys coding agent systems daily. \
                 You focus on concrete architecture: frameworks, CI/CD patterns, testing strategies, \
                 evaluation harnesses, and technical infrastructure that enables agent-driven development.",
                "coding agent architecture testing evaluation CI pipelines frameworks 2026",
                "technical-architecture OR evaluation/testing. Your directions MUST be about tools, \
                 frameworks, CI/CD patterns, testing strategies, evaluation harnesses, or technical \
                 infrastructure for coding agents. Do NOT create governance/policy or economics/market directions.",
            ),
            (
                "Practitioner-S1",
                "practitioner",
                "1",
                "180-365 days",
                "a senior software engineer who builds and deploys coding agent systems daily. \
                 You focus on concrete architecture: frameworks, CI/CD patterns, testing strategies, \
                 evaluation harnesses, and technical infrastructure that enables agent-driven development.",
                "coding agent architecture evaluation frameworks infrastructure 2026 2027",
                "technical-architecture OR evaluation/testing. Your directions MUST be about tools, \
                 frameworks, CI/CD patterns, testing strategies, evaluation harnesses, or technical \
                 infrastructure for coding agents. Do NOT create governance/policy or economics/market directions.",
            ),
            (
                "Critic-S0",
                "critic",
                "0",
                "0-180 days",
                "an economics and organizational behavior analyst. You focus on market dynamics, \
                 vendor economics, adoption barriers, organizational change management, and the \
                 business case for or against coding agent adoption. You challenge optimistic \
                 narratives with economic and organizational reality.",
                "coding agent market economics vendor adoption enterprise organizational change 2026",
                "economics/market OR organizational/adoption. Your directions MUST be about market \
                 dynamics, vendor economics, pricing models, enterprise adoption patterns, organizational \
                 change, or workforce impact. Do NOT create governance/policy or technical-architecture directions.",
            ),
            (
                "Critic-S1",
                "critic",
                "1",
                "180-365 days",
                "an economics and organizational behavior analyst. You focus on market dynamics, \
                 vendor economics, adoption barriers, organizational change management, and the \
                 business case for or against coding agent adoption. You challenge optimistic \
                 narratives with economic and organizational reality.",
                "coding agent economics enterprise adoption workforce impact market structure 2026 2027",
                "economics/market OR organizational/adoption. Your directions MUST be about market \
                 dynamics, vendor economics, pricing models, enterprise adoption patterns, organizational \
                 change, or workforce impact. Do NOT create governance/policy or technical-architecture directions.",
            ),
            (
                "Adjacent-S0",
                "adjacent-domain",
                "0",
                "0-180 days",
                "a cross-domain analyst drawing from biology, evolutionary theory, finance, \
                 manufacturing, and other fields outside software engineering. You find analogies \
                 and patterns from other domains that illuminate where software evolution is heading. \
                 Your insights should be non-obvious connections that pure software analysis would miss.",
                "evolutionary systems biology directed evolution manufacturing automation cross-domain analogies 2026",
                "cross-domain analogies and patterns. Your directions MUST draw from fields OUTSIDE \
                 software engineering (biology, finance, manufacturing, evolutionary theory, organizational \
                 ecology, etc.) and apply those insights to software evolution. Do NOT create pure \
                 governance/policy or pure technical-architecture directions.",
            ),
            (
                "Adjacent-S1",
                "adjacent-domain",
                "1",
                "180-365 days",
                "a cross-domain analyst drawing from biology, evolutionary theory, finance, \
                 manufacturing, and other fields outside software engineering. You find analogies \
                 and patterns from other domains that illuminate where software evolution is heading. \
                 Your insights should be non-obvious connections that pure software analysis would miss.",
                "directed evolution biology finance manufacturing patterns software agents 2026 2027",
                "cross-domain analogies and patterns. Your directions MUST draw from fields OUTSIDE \
                 software engineering (biology, finance, manufacturing, evolutionary theory, organizational \
                 ecology, etc.) and apply those insights to software evolution. Do NOT create pure \
                 governance/policy or pure technical-architecture directions.",
            ),
        ];

        let probe_tools = "temper_get,temper_create,temper_action,temper_web_search,temper_web_fetch";
        let mut probe_session_ids: Vec<String> = Vec::new();

        for (name, role, step, time_range, persona, search_terms, theme_constraint) in
            &probe_specs
        {
            let prompt = build_probe_prompt(
                role,
                step,
                time_range,
                persona,
                search_terms,
                theme_constraint,
                entity_id,
                foresight_model_id,
                &fm_name,
                horizon,
            );

            let sid = create_configured_session(
                &ctx,
                &headers,
                &temper_api_url,
                name,
                role,
                &prompt,
                &seed_model,
                &provider_codex,
                probe_tools,
                "15",
            )?;

            probe_session_ids.push(sid);
        }

        ctx.log(
            "info",
            &format!(
                "spawn_orchestrator: created {} probe sessions",
                probe_session_ids.len()
            ),
        );

        // ========================================================
        // Phase 2: Create cross-theme analyst session (waits for probes)
        // ========================================================

        let probe_ids_python = probe_session_ids
            .iter()
            .map(|id| format!("\"{}\"", id))
            .collect::<Vec<_>>()
            .join(", ");

        let analyst_prompt = CROSS_THEME_ANALYST_PROMPT
            .replace("ANALYST_PROJ_ID", entity_id)
            .replace("ANALYST_PROBE_IDS_PYTHON", &probe_ids_python);

        let analyst_tools = "temper_get,temper_list,temper_write,temper_read";

        let analyst_sid = create_configured_session(
            &ctx,
            &headers,
            &temper_api_url,
            "CrossThemeAnalyst",
            "analyst",
            &analyst_prompt,
            &seed_model,
            &provider_codex,
            analyst_tools,
            "30",
        )?;

        ctx.log(
            "info",
            &format!(
                "spawn_orchestrator: created cross-theme analyst session: {}",
                analyst_sid
            ),
        );

        // ========================================================
        // Phase 3: Create orchestrator session (waits for probes + analyst, synthesizes)
        // ========================================================

        // Build probe ID references for the orchestrator prompt
        let probe_ids_list = probe_session_ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let spec = &probe_specs[i];
                format!("- {} ({}): {}", spec.0, spec.2, id)
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Build orchestrator user_message
        let orch_instructions = ORCHESTRATOR_INSTRUCTIONS
            .replace("ORCH_PROBE_IDS_LIST", &probe_ids_list)
            .replace("ORCH_PROBE_IDS_PYTHON", &probe_ids_python)
            .replace("ORCH_ANALYST_SID", &analyst_sid)
            .replace("ORCH_PROJ_ID", entity_id)
            .replace("ORCH_FM_ID", foresight_model_id);

        let user_message = format!(
            "IMPORTANT: You MUST use the execute tool for ALL actions. \
             ALL entity operations must go through temper.* calls inside execute.\n\n\
             Projection ID: {}\n\
             ForesightModel ID: {}\n\
             Model Name: {}\n\
             Horizon: {}\n\n\
             {}\n\n\
             ===SYNTHESIS_TEMPLATE===\n\
             {}\n\
             ===END_SYNTHESIS_TEMPLATE===",
            entity_id, foresight_model_id, fm_name, horizon,
            orch_instructions, SYNTHESIS_TEMPLATE
        );

        let orch_tools = "temper_get,temper_list,temper_action,temper_create,\
                          temper_write,temper_read,temper_web_search,temper_web_fetch";

        let orch_sid = create_configured_session(
            &ctx,
            &headers,
            &temper_api_url,
            "Orchestrator",
            "orchestrator",
            &user_message,
            &seed_model,
            &provider_codex,
            orch_tools,
            "100",
        )?;

        ctx.log(
            "info",
            &format!(
                "spawn_orchestrator v009: done. Orchestrator: {}, Analyst: {}, Probes: [{}]",
                orch_sid,
                analyst_sid,
                probe_session_ids.join(", ")
            ),
        );

        set_success_result("Running", &json!({}));
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
