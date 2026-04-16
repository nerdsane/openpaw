//! Spawn Orchestrator — WASM module for the Projection.Start integration.
//!
//! Creates an orchestrator Agent+Session that runs the full projection loop:
//! spawn probes, wait, converge observations, write projected state.
//! Then delegates synthesis to a dedicated session to avoid context overflow.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// The orchestration instructions — probes, convergence, and synthesis delegation.
/// Run 004: synthesis is delegated to a separate session to avoid the 68KB context
/// overflow that crashed the orchestrator in Run 003.
const ORCHESTRATION_INSTRUCTIONS: &str = r###"You are orchestrating a foresight projection. Follow these instructions exactly.

## Step 0: Store Synthesis Template

Your FIRST action MUST be to save the synthesis template for later use.
The template is the section of your instructions between ===SYNTHESIS_TEMPLATE===
and ===END_SYNTHESIS_TEMPLATE===. Write it EXACTLY to a workspace file:

```python
# Copy the synthesis template from your instructions into a workspace file.
# The synthesis session will read this file later.
template_text = """PASTE THE EXACT TEXT BETWEEN ===SYNTHESIS_TEMPLATE=== AND ===END_SYNTHESIS_TEMPLATE=== HERE"""
_tf = temper.write("synthesis_template.md", template_text)
_ws = _tf["workspace_id"]
```

## Setup

Read the Projection and ForesightModel to get configuration:

```python
projection = temper.get("Projections", projection_id)
fields = projection["fields"]
model_id = fields["foresight_model_id"]
max_steps = int(fields.get("max_steps", "2"))
import json as _json
step_schedule = _json.loads(fields.get("step_schedule", "[90, 365]"))
probe_config = _json.loads(fields.get("probe_config", "[]"))
horizon = fields.get("horizon", "1 year")

fmodel = temper.get("ForesightModels", model_id)
fm_name = fmodel["fields"]["name"]
```

## The Probe Loop

For each step in range(max_steps), spawn independent probes that observe the domain,
then converge their observations. Use the same spawn_probes pattern as previous runs:

1. Write current state to a workspace file
2. For each probe in probe_config, create Agent + Session + Configure with persona
   (practitioner, critic, adjacent-domain)
3. Wait for all probe sessions to complete (poll every 15 seconds)
4. Read observations: temper.list("Observations", "$filter=projection_id eq 'PROJ_ID'")
5. Convergence: compare observations across probes, confirm converging ones
6. Dispatch audit actions: ProbeStepDone, ConvergenceComplete, ProjectionUpdated, AdvanceStep
7. If not final step, write projected state and advance

## CRITICAL: Direction Consolidation (After Final Step, Before Synthesis)

After ALL probe steps are complete, you MUST consolidate directions before synthesis.
The probes generate ~12 directions (3 probes x 2 steps x 2 per probe). These tend to
cluster on governance themes, creating monothematic output. You MUST archive excess
directions so the synthesis template only sees a diverse subset.

### Consolidation Procedure

```python
import json as _json

# 1. Load all non-archived directions
all_dirs = temper.list("Directions",
    "$filter=projection_id eq '" + projection_id + "' and Status ne 'Archived'")

# 2. Classify each direction by primary theme
#    Themes: governance/policy, technical-architecture, economics/market,
#            organizational/adoption, evaluation/testing, cross-domain
#    Read each direction's title and reasoning. Assign EXACTLY ONE theme.
#    Store as: {dir_id: {"theme": "...", "title": "...", "quality": 1-5}}
#    Rate quality 1-5 based on specificity, evidence grounding, and novelty.

classified = {}
for d in all_dirs:
    did = d["entity_id"]
    f = d.get("fields", {})
    title = f.get("title", "")
    reasoning = f.get("reasoning", "")
    # Classify by theme and rate quality (use your judgment)
    # ... assign theme and quality score ...
    classified[did] = {"theme": theme, "title": title, "quality": quality}

# 3. Select at most 5 directions spanning at least 4 distinct themes
#    Rules:
#    - Maximum 1 direction per theme (pick highest quality)
#    - If governance/policy has the most directions, it gets at most 1 slot
#    - At least 1 must be economics/market or cross-domain
#    - At least 1 must be technical-architecture
#    - Maximum 5 total

# Group by theme, pick best per theme
by_theme = {}
for did, info in classified.items():
    t = info["theme"]
    if t not in by_theme or info["quality"] > by_theme[t]["quality"]:
        by_theme[t] = {"did": did, "quality": info["quality"], "title": info["title"]}

# Select top 5 themes by quality of best direction
selected_dids = set()
# First ensure required themes are represented
for required in ["technical-architecture", "economics/market", "cross-domain"]:
    if required in by_theme:
        selected_dids.add(by_theme[required]["did"])
# Then fill remaining slots
remaining = [(info["quality"], t, info["did"]) for t, info in by_theme.items()
             if info["did"] not in selected_dids]
remaining.sort(reverse=True)
for quality, theme, did in remaining:
    if len(selected_dids) >= 5:
        break
    selected_dids.add(did)

# 4. Archive all non-selected directions
for d in all_dirs:
    did = d["entity_id"]
    if did not in selected_dids:
        temper.action("Directions", did, "Archive", {
            "archive_reason": "Consolidated: theme overlap or lower quality. Kept " + str(len(selected_dids)) + " diverse directions."
        })

# Log what was kept vs archived
kept = [classified[did]["title"] for did in selected_dids if did in classified]
archived_count = len(all_dirs) - len(selected_dids)
```

**This step is NON-NEGOTIABLE.** The synthesis template queries `$filter=Status ne 'Archived'`
and will only see the directions you keep. If you skip this step, the synthesis will have
12+ governance-themed directions and score poorly on Breadth.

After consolidation, verify: `temper.list("Directions", "$filter=projection_id eq '...' and Status ne 'Archived'")`
should return at most 5 directions spanning 4+ themes.

## CRITICAL: After Probes — Delegate Synthesis (DO NOT Synthesize In-Context)

After direction consolidation, DO NOT attempt to build the synthesis in this session.
The accumulated context from probe management will overflow the WASM context parser (~64KB limit).
Instead, delegate synthesis to a dedicated session with clean context.

### Step A: Write Analysis Handoff

Summarize your analytical findings. This gives the synthesis session context that
raw observation data alone cannot provide:

```python
import json as _json

all_obs = temper.list("Observations", "$filter=projection_id eq '" + projection_id + "'")
all_dirs = temper.list("Directions", "$filter=projection_id eq '" + projection_id + "' and Status ne 'Archived'")

# Count stats only — do NOT load full observation content
obs_by_step = {}
obs_by_probe = {}
high_count = 0
for o in all_obs:
    f = o.get("fields", {})
    step = f.get("step_at", "0")
    probe = f.get("probe_name", f.get("probe_agent_id", "unknown"))
    obs_by_step[step] = obs_by_step.get(step, 0) + 1
    obs_by_probe[probe] = obs_by_probe.get(probe, 0) + 1
    if f.get("importance") == "high":
        high_count += 1

handoff = {
    "projection_id": projection_id,
    "model_id": model_id,
    "model_name": fm_name,
    "horizon": horizon,
    "total_observations": len(all_obs),
    "high_importance": high_count,
    "obs_by_step": obs_by_step,
    "obs_by_probe": obs_by_probe,
    "total_directions": len(all_dirs),
    "direction_titles": [d.get("fields", {}).get("title", "") for d in all_dirs],
    "convergence_findings": [
        # Fill in your 3-5 key findings from convergence analysis.
        # These inform the synthesis narrative.
    ],
    "cross_probe_tensions": [
        # Fill in 2-3 places where probes disagreed or offered different perspectives.
    ],
    "source_thesis_challenges": [
        # Fill in 2-3 ways probe observations challenged the source essay's claims.
        # The synthesis session uses these for the "Source Thesis Challenges" section.
    ]
}

_hf = temper.write("analysis_handoff.json", _json.dumps(handoff, indent=2))
```

### Step B: Create and Configure Synthesis Session

Create a dedicated synthesis session and configure it with references to the template
and handoff files. The synthesis session will run with clean context.

```python
synth_agent = temper.create("Agents", {"Name": "Synthesizer", "Role": "synthesizer"})
synth_session = temper.create("Sessions", {"agent_id": synth_agent["entity_id"]})
synth_sid = synth_session["entity_id"]

synth_prompt = (
    "You are synthesizing a foresight projection. Follow the template EXACTLY.\n\n"
    "Projection ID: " + projection_id + "\n\n"
    "## Setup\n\n"
    "FIRST, read these two files from the orchestrator workspace:\n\n"
    "1. Synthesis template (follow this step by step):\n"
    "   template = temper.read('/synthesis_template.md', {'workspace_id': '" + _ws + "'})\n\n"
    "2. Analysis handoff (use for context: convergence findings, tensions, challenges):\n"
    "   handoff = temper.read('/analysis_handoff.json', {'workspace_id': '" + _ws + "'})\n\n"
    "Then follow the synthesis template step by step.\n"
    "Load observations and directions from the API using temper.list().\n"
    "Use the analysis handoff for narrative context.\n\n"
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

### Step C: Wait for Synthesis and Complete

```python
import time
for i in range(60):
    time.sleep(30)
    s = temper.get("Sessions", synth_sid)
    status = s.get("status", "")
    result_field = s.get("fields", {}).get("result", "")
    if status in ("Completed", "Failed") or (result_field and len(result_field) > 200):
        break

if status == "Failed":
    temper.action("Projections", projection_id, "Fail", {"error_message": "Synthesis session failed"})
    temper.done("Synthesis session failed: " + synth_sid)
else:
    temper.done("Projection complete. Synthesis session: " + synth_sid)
```
"###;

/// The synthesis template — embedded in the orchestrator's user_message between markers.
/// The orchestrator writes this to a workspace file at Step 0. The synthesis session reads it.
///
/// Run 004 changes vs Run 003:
/// - "What Surprised Us" → "Source Thesis Challenges" (strengthened per Challenge criterion)
/// - Added explicit instructions to use analysis handoff for narrative context
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

Also read the analysis handoff file (provided by the orchestrator). It contains:
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
4. Active Directions (from Step C)
5. Source Thesis Challenges (from Step F)
6. Top 5 Predictions with Falsification Criteria (from Step D)
7. Decision Points (from Step E)
8. Assumptions & Limitations (3 assumptions with If-wrong and Confidence)
9. Methodology

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
"###;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "spawn_orchestrator: starting");

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
        let headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-tenant-id".to_string(), tenant.to_string()),
            ("x-temper-principal-kind".to_string(), "agent".to_string()),
            ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        // Read ForesightModel for context
        let fm_url = format!("{temper_api_url}/tdata/ForesightModels('{foresight_model_id}')");
        let fm_resp = ctx.http_call("GET", &fm_url, &headers, "")?;
        let (fm_name, seed_model, seed_provider) =
            if fm_resp.status >= 200 && fm_resp.status < 300 {
                let fm: Value = serde_json::from_str(&fm_resp.body).unwrap_or(json!({}));
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

        ctx.log(
            "info",
            &format!(
                "spawn_orchestrator: projection {} model {} ({}) horizon {}",
                entity_id, foresight_model_id, fm_name, horizon
            ),
        );

        // Create Agent
        let agent_url = format!("{temper_api_url}/tdata/Agents");
        let agent_body = json!({
            "Name": "Orchestrator",
            "Role": "orchestrator"
        });
        let agent_resp = ctx.http_call("POST", &agent_url, &headers, &agent_body.to_string())?;
        if agent_resp.status < 200 || agent_resp.status >= 300 {
            return Err(format!(
                "spawn_orchestrator: failed to create Agent (HTTP {}): {}",
                agent_resp.status,
                &agent_resp.body[..agent_resp.body.len().min(300)]
            ));
        }
        let agent_parsed: Value = serde_json::from_str(&agent_resp.body)
            .map_err(|e| format!("spawn_orchestrator: failed to parse Agent response: {e}"))?;
        let agent_id = agent_parsed
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or("spawn_orchestrator: Agent creation did not return entity_id")?;

        ctx.log(
            "info",
            &format!("spawn_orchestrator: created Agent {agent_id}"),
        );

        // Create Session
        let session_url = format!("{temper_api_url}/tdata/Sessions");
        let session_body = json!({"agent_id": agent_id});
        let session_resp =
            ctx.http_call("POST", &session_url, &headers, &session_body.to_string())?;
        if session_resp.status < 200 || session_resp.status >= 300 {
            return Err(format!(
                "spawn_orchestrator: failed to create Session (HTTP {})",
                session_resp.status
            ));
        }
        let session_parsed: Value = serde_json::from_str(&session_resp.body)
            .map_err(|e| format!("spawn_orchestrator: failed to parse Session response: {e}"))?;
        let session_id = session_parsed
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or("spawn_orchestrator: Session creation did not return entity_id")?;

        ctx.log(
            "info",
            &format!("spawn_orchestrator: created Session {session_id}"),
        );

        // Build user_message with orchestration instructions + synthesis template
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
            ORCHESTRATION_INSTRUCTIONS, SYNTHESIS_TEMPLATE
        );

        // Configure Session
        let configure_url = format!(
            "{temper_api_url}/tdata/Sessions('{session_id}')/OpenPaw.Configure"
        );
        let provider_codex = format!("{seed_provider}_codex");
        let configure_body = json!({
            "model": seed_model,
            "provider": provider_codex,
            "tools_enabled": "temper_get,temper_list,temper_action,temper_create,temper_write,temper_read,temper_web_search,temper_web_fetch",
            "max_turns": "100",
            "user_message": user_message,
            "sandbox_url": "none",
            "temper_api_url": temper_api_url
        });
        let configure_resp = ctx.http_call(
            "POST",
            &configure_url,
            &headers,
            &configure_body.to_string(),
        )?;
        if configure_resp.status < 200 || configure_resp.status >= 300 {
            return Err(format!(
                "spawn_orchestrator: Configure failed for Session {} (HTTP {}): {}",
                session_id,
                configure_resp.status,
                &configure_resp.body[..configure_resp.body.len().min(300)]
            ));
        }

        ctx.log(
            "info",
            &format!(
                "spawn_orchestrator: configured Session {} ({} bytes)",
                session_id,
                user_message.len()
            ),
        );

        set_success_result("Running", &json!({}));

        ctx.log(
            "info",
            &format!(
                "spawn_orchestrator: done, Session {} for Projection {}",
                session_id, entity_id
            ),
        );
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
