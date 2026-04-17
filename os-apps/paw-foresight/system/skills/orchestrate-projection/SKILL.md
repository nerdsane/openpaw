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

Report convergence:
```python
temper.action("Projections", projection_id, "ConvergenceComplete", {})
```

## Step 5: Write Projected State AND the Step Rollup

**You MUST produce TWO artifacts at the end of every step, in this exact order:**

1. `step_{step}_rollup.md` — a structured per-step narrative rollup (see schema below).
2. `projected_state_step_{step}.json` — the evolved world state (skipped on the final step).

The rollup is the primary progression artifact. Every step writes one. The final
synthesis will not re-author temporal progression — it will COMPOSE from these
rollups in order. Skipping a rollup or deviating from the four-section schema
breaks the synthesis contract.

### Step 5a: Write `step_{step}_rollup.md` (EVERY step, including the final one)

Read all observations and directions for THIS step, and (for step > 0) read the
prior step's rollup so you know what to confirm/revise/falsify.

```python
# Read this step's observations and directions
step_obs = temper.list("Observations",
    f"$filter=projection_id eq '{projection_id}' and step_at eq '{step}'")
step_dirs = temper.list("Directions",
    f"$filter=projection_id eq '{projection_id}' and step_at eq '{step}'")

# Read the prior rollup (step > 0 only) so you can confirm/revise/falsify from it
prior_rollup = ""
if step > 0:
    try:
        prior_rollup = temper.read(f"step_{step - 1}_rollup.md")
    except Exception:
        prior_rollup = ""  # step 0 absence is OK; a missing prior rollup means nothing to revise
```

The rollup file is plain Markdown with EXACTLY the four sections below, in this
order, using these exact heading strings. Step 0 writes only section 1 (no prior
steps exist). Later steps MUST include all four — leave a section empty with an
explicit "None." line if nothing qualifies; do not omit the heading.

```markdown
# Step {step} Rollup — day {days_offset} of {horizon}

## New predictions this step
- **{Title}** — {1-2 sentence prediction stated so it can be checked after the fact.}
  - Evidence: {observation IDs that support this, cite obs: en-...}
  - Mechanism: {why this will happen — causal chain in one sentence}
  - Falsification: {what would make this wrong, stated as an observable condition}

## Confirmed from prior steps
- **{Prior prediction title, quoted from step {step-1} rollup}** — this step's evidence strengthens it.
  - New supporting evidence: {observation IDs or external signal URLs this step surfaced}
  - Why it's now more credible: {one sentence}

## Revised from prior steps
- **{Prior prediction title}** — was: "{exact quote of prior prediction}". Now: "{new wording}".
  - What changed: {wording / scope / threshold / timeline — name which}
  - Mechanism that forced the revision: {observation ID or external signal}

## Falsified from prior steps
- **{Prior prediction title}** — was: "{exact quote}". This step breaks it.
  - Falsifying evidence: {observation ID or external signal URL}
  - Why the prior prediction fails: {one sentence — which assumption broke}
```

Rules for the rollup:
- Step 0 writes the file with only `## New predictions this step` populated. The other
  three section headings should be present with `None. (no prior step to revise.)`
  under each, so every rollup has the same shape.
- In later steps, Confirmed/Revised/Falsified items MUST quote the prior step's
  prediction title exactly so a reader can trace the chain across files.
- If nothing was confirmed/revised/falsified this step, leave the heading and
  write `None. {one sentence explaining why — "no prior predictions touched X"}`.
  Do NOT omit the heading.
- The rollup is EVIDENCE-FIRST: every new prediction must name at least one
  observation ID from this step; every confirmation/revision/falsification must
  cite the mechanism from this step's evidence pool.

Write the rollup:
```python
rollup_body = """# Step {step} Rollup — day {days_offset} of {horizon}

## New predictions this step
{new_section}

## Confirmed from prior steps
{confirmed_section}

## Revised from prior steps
{revised_section}

## Falsified from prior steps
{falsified_section}
""".format(
    step=step, days_offset=days_offset, horizon=horizon,
    new_section=new_section,
    confirmed_section=confirmed_section or "None. (no prior step to revise.)" if step == 0 else confirmed_section,
    revised_section=revised_section or "None. (no prior step to revise.)" if step == 0 else revised_section,
    falsified_section=falsified_section or "None. (no prior step to revise.)" if step == 0 else falsified_section,
)

temper.write(f"step_{step}_rollup.md", rollup_body)
```

### Step 5b: Write projected state and dispatch ProjectionUpdated (non-final steps only)

If this is NOT the final step, synthesize an evolved world state. Read all
observations and directions from ALL steps so far.

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

After the last step completes, produce a human-readable synthesis. **Temporal
Progression is COMPOSED from the per-step rollup files you wrote in Step 5 —
you do NOT author a new phase narrative here.** This is a contract: the
rollups are the canonical chain of revisions; the synthesis is their
composition plus the surrounding framing (executive summary, findings,
directions, decisions).

```python
# Read all final observations and directions
all_obs = temper.list("Observations", f"$filter=projection_id eq '{projection_id}'")
all_dirs = temper.list("Directions",
    f"$filter=projection_id eq '{projection_id}' and Status ne 'Archived'")

# Read every step's rollup — one per step, in order. Missing rollups are a
# contract violation; surface them rather than silently skipping.
step_rollups = []
for s in range(max_steps):
    try:
        body = temper.read(f"step_{s}_rollup.md")
    except Exception:
        body = f"(step_{s}_rollup.md missing — contract violation; the orchestrator failed to write this step's rollup.)"
    # Extract the days_offset for this step's heading
    s_day = step_schedule[s] if s < len(step_schedule) else step_schedule[-1]
    step_rollups.append((s, s_day, body))
```

Build the synthesis. The **Temporal Progression** section is assembled by
concatenating the per-step rollups under step-scoped headings. Do NOT
paraphrase, compress, or rewrite the rollup bodies — copy them verbatim
under their step heading so the reader can diff step N against step N+1.

```python
# Compose the Temporal Progression section from the rollups, verbatim
progression_sections = []
for (s, s_day, body) in step_rollups:
    progression_sections.append(
        f"### Step {s} (day {s_day} of {horizon})\n\n{body.strip()}\n"
    )
temporal_progression = "\n".join(progression_sections)

synthesis = f"""# Foresight Projection: {model_name}
## Horizon: {horizon} | Steps: {max_steps}
## Date: {today}

### Executive Summary
[2-3 paragraph synthesis of the most important findings. Reference the
progression chain — name at least one prior-step prediction that was
revised or falsified in a later step, and why that revision matters.]

### Key Findings
[Bulleted list of the strongest convergent observations]

### Temporal Progression
[Composed from the per-step rollups below. Do NOT rewrite them — the rollups
ARE the progression record. Include a one-paragraph preface noting which
steps produced the most consequential revisions/falsifications, then include
each rollup verbatim.]

{temporal_progression}

### Active Directions
[For each active direction: title + FULL reasoning text from the direction entity.
Do NOT truncate reasoning. Include the complete text as stored in the direction's
"reasoning" field. Each direction should have its full argument, not a summary.]

### What Surprised Us
[Observations that challenged initial assumptions. Reference specific
falsifications from the Temporal Progression section when relevant.]

### Decision Points
[Actionable recommendations with timing triggers]

### Methodology
- {len(probe_config)} independent probes per step
- {max_steps} time steps over {horizon}
- {len(all_obs)} total observations, {len(all_dirs)} total directions
- Temporal Progression composed from {len(step_rollups)} per-step rollup files
"""

result = temper.write(f"projection_synthesis_{projection_id}.md", synthesis)

# Complete the projection
temper.action("Projections", projection_id, "Complete", {})
temper.done(f"Projection complete. Synthesis: {result['file_id']}")
```

**Contract check before calling Complete:** the synthesis MUST contain a
`### Temporal Progression` section followed by one `### Step N (day X of
{horizon})` sub-heading per step, each containing the four-section rollup
body verbatim. If the rollups are missing or the composition was skipped,
do not dispatch Complete — instead write an error note to workspace and
dispatch `Fail` with `error_message` naming the missing artifacts.

## Error Handling

If a probe session fails, log it and continue with remaining probes.
If fewer than 2 probes complete, fail the projection:

```python
temper.action("Projections", projection_id, "Fail", {
    "error_message": "Too few probes completed (need at least 2)"
})
temper.done("Projection failed: insufficient probes")
```
