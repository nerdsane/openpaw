//! Spawn Orchestrator — WASM module for the Projection.Start integration.
//!
//! Creates an orchestrator Agent+Session that runs the full projection loop:
//! spawn probes, wait, converge observations, write projected state, synthesize.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// The orchestration instructions embedded directly in the user_message.
/// This avoids the TemperFS skill lookup that failed in Run 001.
///
/// NOTE: Uses r##"..."## delimiters to allow " and "# inside content.
const ORCHESTRATION_INSTRUCTIONS: &str = r##"You are orchestrating a foresight projection. Follow these instructions exactly.

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

## CRITICAL: Final Synthesis — Data-Driven Construction

After the last step completes, build the synthesis from actual observation and direction data.
This is the MOST IMPORTANT part. The synthesis structure is MANDATORY.

### Step A: Load Data

```python
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

### Step C: Build Active Directions

For each active direction, include its FULL reasoning text from the entity, not a summary:

```
#### [Direction Title]
**Direction ID:** [direction entity ID]

[Full reasoning text from direction entity — do NOT truncate]

Supporting observations: [obs: ID1], [obs: ID2]

**Counterfactual:** [counterfactual from direction entity]
```

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

### Step F: What Surprised Us

Include 3-5 observations that challenge assumptions. Each must cite its obs ID.

### Step G: Assemble Complete Synthesis

The MANDATORY section order is:

1. Executive Summary (3 paragraphs, cite obs IDs, name 6+ companies/tools across 3+ categories, include quant claims)
2. Key Findings (from Step B)
3. Temporal Progression (4 phases: 0-3mo, 3-6mo, 6-9mo, 9-12mo)
   - Phases 2-4 MUST have a "Revisions to earlier predictions" subsection
4. Active Directions (from Step C)
5. What Surprised Us (from Step F)
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
6. Temporal phases 2-4 MUST revise earlier predictions
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
"##;

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

        // Build user_message
        let user_message = format!(
            "IMPORTANT: You MUST use the execute tool for ALL actions. \
             ALL entity operations must go through temper.* calls inside execute.\n\n\
             Projection ID: {}\n\
             ForesightModel ID: {}\n\
             Model Name: {}\n\
             Horizon: {}\n\n\
             {}",
            entity_id, foresight_model_id, fm_name, horizon, ORCHESTRATION_INSTRUCTIONS
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
