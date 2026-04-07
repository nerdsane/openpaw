# Proof Report: 034 — Foresight Engine Honest Status

## Date
2026-04-07

## What Works (Temper-Native, No Human Intervention)

### Entity Lifecycle — FULLY AUTONOMOUS
- ProductModel: Created → Seeding → Active (seed_model WASM fetches GitHub + Datadog)
- Projection: Created → Running (spawn_probes WASM creates Agent+Session pairs)
- Probes: Run independently via GPT-5 (OpenAI Codex) or Claude Sonnet (Anthropic)
- Probes create Observations and Directions via temper.create()
- Probes self-report via ProbeStepDone action on Projection
- handle_probe_done WASM detects all probes reported, spawns Convergence Analyst
- Direction versioning: probes archive old Direction, create revision with parent_direction_id

### What the Pipeline Produces
- 3 independent Probes produce 3-4 Observations each + exactly 1 Direction per Probe
- Convergence Analyst runs convergence analysis (65+ turns, confirms/contradicts)
- Across 2 steps: 26 Observations, 9 Directions (3 per step, with version chains)

## What Required Manual Intervention

### 1. ConvergenceComplete callback — MANUAL every step
**What happens**: The Convergence Analyst finishes its analysis (65-74 turns) but does NOT call `temper.action("Projections", id, "ConvergenceComplete", {...})`. It just calls `temper.done("complete")` and the Projection stalls.

**What I did**: Manually fired `POST /tdata/Projections('{id}')/OpenPaw.Foresight.ConvergenceComplete` via curl after each step.

**Root cause**: The instruction was buried at line 47 of the analyst prompt. GPT-5 lost focus after 60+ turns of convergence work.

**Fix applied**: Moved the CRITICAL callback instruction to line 1 of the prompt. Not yet tested — needs a new E2E run to confirm.

### 2. Projected State Production — NEVER WORKED
**What happens**: The Convergence Analyst was supposed to produce an updated projected state JSON, upload it to TemperFS, and pass the file_id in ConvergenceComplete. It never did this. The PHASE 2 instructions were too complex (JSON schema template in the prompt, file upload sequence).

**What I did**: Fired ConvergenceComplete with `projected_state_file_id: ""` (empty). Probes on step 1+ got no projected state — they saw the original knowledge graph, not an evolved model.

**Impact**: The "simulation" aspect (model evolving between steps) did not work. Probes at step 1 re-analyzed the same data as step 0.

**Fix applied**: Removed PHASE 2 from the analyst prompt entirely. Projected state production needs a different approach — either a simpler format or a dedicated WASM module that isn't an LLM agent.

### 3. max_steps boundary — WRONG
**What happens**: handle_convergence checks `current_step + 1 >= max_steps` but after AdvanceStep increments the counter, the Projection ends up at step N+1 instead of completing at step N.

**What I did**: Manually fired `POST /tdata/Projections('{id}')/OpenPaw.Foresight.Complete` to finalize.

**Fix needed**: The completion check logic in handle_convergence needs to account for the counter increment.

## What Probes Actually See (and Why It's Wrong)

### Current Knowledge Graph Contains:
- Last 20 GitHub PRs (all about telemetry/observability)
- Last 20 commits (Datadog setup, APM, middleware)
- README (was truncated to 2000 chars — barely covers the intro)
- Directory listing (filenames only, no content)
- Datadog monitors and events
- NO deployment URL
- NO product description beyond commit messages
- NO actual source code content

### What Probes Produced:
All 9 Directions across 3 steps were about telemetry → product conversion:
- "Event-sourced timelines"
- "Trace-native simulation"
- "Span-first timeline explorer"

None of them mentioned: world-building, sci-fi, collaborative storytelling, dwellers, AI social platform — which is what Deep Sci-Fi actually IS.

### Fixes Applied (Not Yet Tested):
- Full README (10KB limit instead of 2000 chars)
- Fetch deployed website (`deep-sci-fi.world`) via GitHub homepage field
- Probe prompt: "UNDERSTAND THE PRODUCT FIRST", "DO NOT just analyze telemetry"
- Source code reading via web_fetch on raw GitHub URLs

## Infrastructure Issues Encountered

### 1. Anthropic API Rate Limit
The Anthropic OAuth token hit a spending cap. Probes couldn't call the API for hours. Required switching to OpenAI.

### 2. .env Corruption
Line 2 of .env had user text appended (`yes that matches my intuition...`), causing `source .env` to fail silently. The daemon started without any API keys. Took hours to diagnose.

### 3. OpenAI Codex Integration (New)
Built from scratch in this session:
- SSE streaming support in Temper host (format-agnostic deframing)
- 4MB HTTP buffer (was 512KB — too small for SSE responses)
- Responses API format conversion (instructions, input, function_call/function_call_output)
- tool_choice: "required" to force GPT-5 to use the execute tool
- Proper tool_result → function_call_output conversion for multi-turn

### 4. Daemon Killed by Other Agent
The other Claude Code agent running on the same machine was killing our daemon process. Mitigated by using port 3471 and a renamed binary.

## Summary: What's Autonomous vs. What's Not

| Step | Autonomous? | Notes |
|------|------------|-------|
| ProductModel seeding | YES | seed_model WASM runs fully autonomously |
| Probe spawning | YES | spawn_probes creates Agent+Session pairs |
| Probe execution | YES | GPT-5 calls tools, creates entities, self-reports |
| ProbeStepDone detection | YES | handle_probe_done checks all reported |
| Convergence Analyst spawn | YES | handle_probe_done creates Agent+Session |
| Convergence analysis | YES | Analyst confirms/contradicts observations |
| **ConvergenceComplete callback** | **NO** | Analyst doesn't call it — projection stalls |
| **Step advancement** | **NO** | Requires manual ConvergenceComplete |
| **Projected state evolution** | **NO** | Never produced — probes re-analyze same data |
| **Projection completion** | **NO** | max_steps boundary bug + manual Complete |
| Direction versioning | YES | Archive old → create revision with parent_id |

**Bottom line**: The pipeline is ~70% autonomous. The 30% that breaks is the Convergence Analyst not completing the callback loop, which stalls the entire multi-step progression. The fix (moving the instruction to line 1) is applied but untested.
