---
name: orchestrate-projection
description: Run a foresight projection loop — spawn independent probes, converge observations, evolve projected state, produce human-readable synthesis
---

# Orchestrate Projection

You are orchestrating a foresight projection. Your user_message contains the Projection ID
and ForesightModel ID. You run the full loop: spawn probes, wait for them, read their
observations, do convergence, write projected state, advance steps, and produce a final
synthesis.

## Setup

Read the Projection and ForesightModel to get configuration:

```python
projection = temper.get("Projections", projection_id)
fields = projection["fields"]
model_id = fields["foresight_model_id"]
max_steps = int(fields["max_steps"])
step_schedule = json.loads(fields["step_schedule"])  # e.g. [1, 6, 18]
probe_config = json.loads(fields["probe_config"])     # e.g. [{"name":"practitioner"}, ...]

# Read THIS session's model/provider — probes inherit it by default.
# The orchestrator was configured by spawn_orchestrator WASM with the correct
# codex provider. Never default to "anthropic" or bare "openai" — only the
# codex token is available on this tenant.
my_session = temper.get("Sessions", temper.get_session_id(), "$select=model,provider")
my_model = my_session.get("fields", my_session).get("model", "gpt-5.4")
my_provider = my_session.get("fields", my_session).get("provider", "openai_codex")

fmodel = temper.get("ForesightModels", model_id)
kg_file_id = fmodel["fields"]["model_snapshot_file_id"]
model_name = fmodel["fields"]["name"]
model_type = fmodel["fields"]["model_type"]
signal_config = fmodel["fields"]["signal_source_config"]

# Read the knowledge graph — the file lives in the seed session's workspace,
# NOT this session's workspace. Use temper.get to find the workspace, then
# temper.read with an explicit workspace_id.
kg_file = temper.get("Files", kg_file_id)
kg_ws_id = kg_file["fields"]["WorkspaceId"]
kg_path = kg_file["fields"]["Path"]
kg_content = temper.read(kg_path, {"workspace_id": kg_ws_id})
```

## The Loop

```python
current_state = kg_content  # step 0 starts from the knowledge graph

for step in range(max_steps):
    days_offset = step_schedule[step] if step < len(step_schedule) else step_schedule[-1]

    # 1. Spawn probes
    # 2. Wait for probes to complete
    # 3. Read observations
    # 4. Converge
    # 5. Write projected state (skip on final step)
    # 6. Dispatch entity actions for audit trail
    # 7. Advance step
```

## Step 1: Spawn Probes

Create one session per probe. Each probe gets a DIFFERENT prompt to ensure divergence.

**IMPORTANT: Field size limit.** Temper truncates entity fields larger than 32 KB.
The knowledge graph can be 30-50 KB, so NEVER embed it directly in the probe's
`user_message`. Instead, write the current state to a file and give probes a
reference to read it themselves via `temper.read()`.

**Probe differentiation strategies** (assign one per probe):

- **Practitioner**: "You are a practitioner building systems in this domain. Focus on what
  is technically feasible in the near term, what tools and architectures are maturing, and
  what practitioners will actually adopt. Be concrete about mechanisms."

- **Critic**: "You are a skeptical analyst. Focus on what could go wrong, what assumptions
  are fragile, what counterarguments exist, and what the domain is NOT ready for. Challenge
  the dominant narrative."

- **Adjacent-domain**: "You are an observer from an adjacent field (economics, biology,
  organizational theory, or another relevant domain). Focus on patterns from your field
  that this domain hasn't recognized yet. Bring external analogies."

For each probe, also select a different subset of the knowledge graph to emphasize.
If the KG has sections (signals, ontology, ecosystem, open_problems), give each probe
a primary focus area while providing the full KG for context.

```python
probe_sessions = []
probe_agent_ids = []
my_session_id = temper.get_session_id()  # needed for parent_session_id on child sessions
my_workspace_id = temper.get_workspace_id()  # probes need this to read state files

# Write current_state to a file so probes can read it (avoids 32KB field limit)
state_file = temper.write(f"current_state_step_{step}.json", current_state)
state_file_id = state_file["file_id"]
state_file_path = f"current_state_step_{step}.json"

for i, pc in enumerate(probe_config):
    persona = ["practitioner", "critic", "adjacent-domain"][i % 3]
    # ... build differentiated user_message (see Probe Prompt below)
    # Pass state_file_path, my_workspace_id so probes can temper.read() the state

    agent = temper.create("Agents", {"fields": {
        "name": f"Probe-{pc['name']}-step-{step}",
        "role": "probe"
    }})
    agent_id = agent["entity_id"]
    probe_agent_ids.append(agent_id)

    # IMPORTANT: set parent_session_id so Cedar allows this session to Configure the child
    session = temper.create("Sessions", {"fields": {"parent_session_id": my_session_id}})
    session_id = session["entity_id"]

    temper.action("Sessions", session_id, "Configure", {
        "user_message": probe_user_message,
        "model": pc.get("model", my_model),
        "provider": pc.get("provider", my_provider),
        "agent_id": agent_id,
        "parent_session_id": my_session_id,
        "tools_enabled": "temper_get,temper_list,temper_action,temper_create,temper_read,temper_web_search,temper_web_fetch",
        "max_turns": "30"
    })
    probe_sessions.append({"session_id": session_id, "agent_id": agent_id})
```

Report probes spawned:
```python
temper.action("Projections", projection_id, "ProbesReady", {
    "probe_agent_ids": json.dumps([p["agent_id"] for p in probe_sessions])
})
```

## Step 2: Wait for Probes

Poll until all probe sessions are terminal (Completed or Failed):

```python
import time

while True:
    all_done = True
    for ps in probe_sessions:
        s = temper.get("Sessions", ps["session_id"], "$select=Status,error_message,turn_count")
        status = s.get("status", s.get("fields", {}).get("Status", ""))
        if status not in ["Completed", "Failed", "Cancelled"]:
            all_done = False
            break
    if all_done:
        break
    time.sleep(10)  # check every 10 seconds
```

## Step 3: Read Observations

Read all observations created by the probes for this step:

```python
observations = temper.list("Observations",
    f"$filter=projection_id eq '{projection_id}' and step_at eq '{step}'")

# Report each probe done
for ps in probe_sessions:
    # Find direction created by this probe
    dirs = temper.list("Directions",
        f"$filter=proposer_agent_id eq '{ps['agent_id']}' and projection_id eq '{projection_id}' and step_at eq '{step}'")
    direction_id = dirs[0]["entity_id"] if dirs else ""
    temper.action("Projections", projection_id, "ProbeStepDone", {
        "probe_agent_id": ps["agent_id"],
        "direction_id": direction_id
    })
```

## Step 4: Convergence

You do convergence yourself. Read all observations and find cross-probe agreements.

For each pair of observations from DIFFERENT probes:
- If they describe the same phenomenon from different angles → **Confirm** both
- If they directly contradict → create a new Observation noting the contradiction

```python
# Group observations by probe
by_probe = {}
for obs in observations:
    pid = obs["fields"]["probe_agent_id"]
    by_probe.setdefault(pid, []).append(obs)

# Find cross-probe convergence
confirmed = set()
for probe_a, obs_a_list in by_probe.items():
    for probe_b, obs_b_list in by_probe.items():
        if probe_a >= probe_b:
            continue
        for obs_a in obs_a_list:
            for obs_b in obs_b_list:
                # Compare content — use your judgment on semantic similarity
                # If converging: confirm both
                if semantically_similar(obs_a, obs_b):
                    if obs_a["entity_id"] not in confirmed:
                        temper.action("Observations", obs_a["entity_id"], "Confirm", {
                            "confirmer_agent_id": temper.get_agent_id()
                        })
                        confirmed.add(obs_a["entity_id"])
```

### Observation Deduplication (after confirmation)

After cross-probe confirmation, deduplicate semantically overlapping observations to
improve information density in the final synthesis. This is MANDATORY before synthesis.

```python
# Re-read all observations (some may now be Confirmed)
all_step_obs = temper.list("Observations",
    "$filter=projection_id eq '" + projection_id + "' and step_at eq '" + str(step) + "'")

# Filter to non-Faded observations only
live_obs = [o for o in all_step_obs if o.get("status", "") != "Faded"]

# Group by semantic theme. For each observation, extract the core claim.
# A "theme" is the central phenomenon described (e.g., "harness quality > model quality",
# "coordination bottleneck", "trust architecture gap", "dark factory won't happen").
# Two observations share a theme if removing either would NOT reduce the set of
# distinct analytical conclusions in the synthesis.

themes = {}  # theme_label -> [obs list]
for obs in live_obs:
    content = obs.get("fields", {}).get("content", "")
    # Assign to a theme based on the core claim. Use your judgment.
    # If an observation fits multiple themes, assign to its PRIMARY theme.
    theme = classify_theme(content)  # your judgment call
    themes.setdefault(theme, []).append(obs)

# For each theme with 3+ observations: keep the 2 strongest, Fade the rest.
# "Strongest" = best external evidence (URLs, named sources) + most specific claims
# (named actors, dates, quantitative thresholds) + highest importance rating.
faded_count = 0
for theme, obs_list in themes.items():
    if len(obs_list) <= 2:
        continue  # no dedup needed for small clusters

    # Rank by quality: external evidence > specificity > importance
    ranked = sorted(obs_list, key=lambda o: (
        # Prefer observations with external URLs/sources
        1 if "http" in o.get("fields", {}).get("content", "") else 0,
        # Prefer high importance
        1 if o.get("fields", {}).get("importance", "") == "high" else 0,
        # Prefer longer, more detailed content
        len(o.get("fields", {}).get("content", ""))
    ), reverse=True)

    keep = ranked[:2]
    keep_ids = [o["entity_id"] for o in keep]
    fade = ranked[2:]

    for obs in fade:
        obs_id = obs["entity_id"]
        temper.action("Observations", obs_id, "Fade", {
            "fade_reason": "Deduplicated: same theme '" + theme + "' as " + keep_ids[0] + ". Kept stronger observations with better evidence."
        })
        faded_count += 1

# Log dedup results
print(f"Deduplication: {faded_count} observations faded, {len(live_obs) - faded_count} remain")
# Target: ≤15 observations after dedup. If still above 15, do a second pass
# with stricter theme merging.
```

Report convergence:
```python
temper.action("Projections", projection_id, "ConvergenceComplete", {})
```

## Step 5: Write Projected State

If this is NOT the final step, synthesize an evolved world state.

Read all observations and directions from ALL steps so far. Synthesize what the world
looks like at `days_offset` days from now.

```python
if step < max_steps - 1:
    all_obs = temper.list("Observations",
        f"$filter=projection_id eq '{projection_id}'")
    all_dirs = temper.list("Directions",
        f"$filter=projection_id eq '{projection_id}'")

    # Synthesize projected state as JSON
    projected_state = {
        "base_model": model_id,
        "step": step,
        "days_offset": days_offset,
        "step_history": [],  # summarize each step
        "current_projected_state": {},  # evolved domain state
        "convergent_findings": [],
        "new_signals": [],
        "open_questions": []
    }
    # Fill in based on observations and directions...
    # (Use your reasoning to synthesize the evolution)

    result = temper.write(
        f"projected_state_step_{step}.json",
        json.dumps(projected_state, indent=2)
    )
    state_file_id = result["file_id"]

    temper.action("Projections", projection_id, "ProjectionUpdated", {
        "projected_state_file_id": state_file_id
    })

    # Update current_state for next step
    current_state = json.dumps(projected_state)
```

## Step 6: Advance Step

```python
temper.action("Projections", projection_id, "AdvanceStep", {})
```

Then loop back to Step 1 with the new `current_state`.

## Probe Prompt Template

Each probe gets this prompt, customized with persona and focus area.

**IMPORTANT: Keep user_message under 32 KB.** Do NOT embed the knowledge graph or
current state directly. Give the probe a file reference to read via `temper.read()`.

```
You are a foresight probe ({persona}) analyzing the domain "{model_name}" ({model_type}).

Projection ID: {projection_id}
Your Agent ID: {agent_id}
Step: {step} (day {days_offset} of {horizon})
{previous_context}

## Your Persona
{persona_instructions}

## Current State of the World
Read the current state (knowledge graph / projected state) with:
  state = temper.read("{state_file_path}", {{"workspace_id": "{orchestrator_workspace_id}"}})
This file is in the orchestrator's workspace, not yours. You MUST pass the workspace_id option.

## Your Focus Area
{focus_instructions}

## Instructions

1. FIRST, read the current state file using the temper.read() call above. Study it carefully.

2. SEARCH FOR EXTERNAL EVIDENCE. Before making any observations, run at least 2 web searches
   to find real, recent signals NOT in the knowledge graph. Use temper.web_search() to search
   and temper.web_fetch() to read promising results. Look for:
   - Recent news, announcements, or product launches relevant to the domain
   - Research papers, blog posts, or industry reports with data or findings
   - Events, conferences, or community signals that confirm or contradict the KG themes
   
   Example:
     results = temper.web_search("directed software evolution 2026 trends")
     page = temper.web_fetch("https://example.com/relevant-article")
   
   You MUST cite external sources in your observations. An observation grounded in both
   the knowledge graph AND an external signal is stronger than one grounded in only the KG.
   Include the source URL or title in your observation content.

3. PROJECT forward {days_offset} days from the current state.
   What has changed? What signals would you expect to see?
   What has NOT changed that you expected to?

4. Create 3-6 Observations. First create the entity, then pass ALL fields to the Record action:

   obs = temper.create("Observations", {{"fields": {{}}}})
   obs_id = obs["entity_id"]
   temper.action("Observations", obs_id, "Record", {{
       "content": "What you observe (be specific — name mechanisms, actors, timelines)",
       "importance": "high",
       "signal_refs": "[\"signal-id-1\",\"signal-id-2\"]",
       "counterfactual": "What happens if this observation is ignored?",
       "probe_agent_id": "{agent_id}",
       "projection_id": "{projection_id}",
       "step_at": "{step}"
   }})

   IMPORTANT: Fields MUST be passed to the Record action, NOT to create.
   The create call uses empty fields. The Record action stores the state variables.

5. Create exactly ONE Direction — your single strongest thesis.
   First create, then pass ALL fields to the Propose action:

   dir = temper.create("Directions", {{"fields": {{}}}})
   dir_id = dir["entity_id"]
   temper.action("Directions", dir_id, "Propose", {{
       "title": "One sentence thesis",
       "reasoning": "2-3 paragraphs explaining WHY, with evidence",
       "grounding": "[\"signal-ref-1\"]",
       "observation_ids": "[\"obs-id-1\",\"obs-id-2\"]",
       "counterfactual_summary": "What happens if this direction is wrong?",
       "proposer_agent_id": "{agent_id}",
       "projection_id": "{projection_id}",
       "step_at": "{step}"
   }})
   {parent_direction_instructions}

   IMPORTANT: Fields MUST be passed to Propose, NOT to create.

6. Report done:
   temper.action("Projections", "{projection_id}", "ProbeStepDone", {{
       "probe_agent_id": "{agent_id}",
       "direction_id": "<your direction ID>"
   }})
   temper.done("Probe complete")

## Rules
- Do NOT read other probes' observations. You are independent.
- Be SPECIFIC. Name technologies, companies, mechanisms, dates.
- At least one observation should CHALLENGE the dominant narrative in the state.
- Ground claims in signals from the knowledge graph where possible.
- At least 2 observations MUST cite external evidence found via web search (not in the KG).
  Include the source URL or title. An observation with only KG grounding scores lower than
  one with KG + external corroboration.
```

For steps > 0, add previous context:
```
## Your Previous Observations (from prior steps)
{previous_observations_summary}

## Your Previous Direction
{previous_direction}

## Instructions for Direction Versioning
Archive your previous direction and create a new one with parent_direction_id:
temper.action("Directions", "{old_direction_id}", "Archive", {
    "archive_reason": "Revised in step {step}"
})
Then create new Direction with parent_direction_id = "{old_direction_id}"
```

## Final Synthesis

After the last step completes, produce a human-readable synthesis:

```python
# Read all final observations and directions
all_obs = temper.list("Observations", f"$filter=projection_id eq '{projection_id}'")
all_dirs = temper.list("Directions",
    f"$filter=projection_id eq '{projection_id}' and Status ne 'Archived'")

# Write a synthesis narrative
synthesis = f"""# Foresight Projection: {model_name}
## Horizon: {horizon} | Steps: {max_steps}
## Date: {today}

### Executive Summary
[2-3 paragraph synthesis of the most important findings]

### Key Findings
[Bulleted list of the strongest convergent observations]

### Active Directions
[For each active direction: title + FULL reasoning text from the direction entity.
Do NOT truncate reasoning. Include the complete text as stored in the direction's
"reasoning" field. Each direction should have its full argument, not a summary.]

### What Surprised Us
[Observations that challenged initial assumptions]

### Decision Points
[Actionable recommendations with timing triggers]

### Methodology
- {len(probe_config)} independent probes per step
- {max_steps} time steps over {horizon}
- {len(all_obs)} total observations, {len(all_dirs)} total directions
"""

result = temper.write(f"projection_synthesis_{projection_id}.md", synthesis)

# Complete the projection
temper.action("Projections", projection_id, "Complete", {})
temper.done(f"Projection complete. Synthesis: {result['file_id']}")
```

## Error Handling

If a probe session fails, log it and continue with remaining probes.
If fewer than 2 probes complete, fail the projection:

```python
temper.action("Projections", projection_id, "Fail", {
    "error_message": "Too few probes completed (need at least 2)"
})
temper.done("Projection failed: insufficient probes")
```
