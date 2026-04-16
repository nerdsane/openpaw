# Run 001 Transcripts MANIFEST

## Projection: en-019d96a8-2abb-7350-9b3f-b1cf881fd399

### Orchestrator
- **orchestrator.jsonl** (ss-019d96a8-2b0d-7c63-b3bf-c575176af832): 10 turns, status=Completed, model=gpt-5.4, provider=openai_codex. Event log format (OpenAI Codex sessions don't produce JSONL conversation transcripts).

### Step 0 Probes (0-90 days)
- **probe_practitioner_step0.jsonl** (ss-019d96a8-f0ec-7080-b344-ee60c482eae3): 3 turns, status=Completed, model=gpt-5.4, provider=openai_codex. Agent: aj-019d96a8-f0e8. Created 4 step-0 obs + 1 direction; also 4 step-1 obs + 1 direction.
- **probe_critic_step0.jsonl** (ss-019d96a8-f0fb-7611-b1fc-3b7a0337a92b): 3 turns, status=Completed, model=gpt-5.4, provider=openai_codex. Agent: aj-019d96a8-f0f7. Created 4 step-0 obs + 1 direction; also 4 step-1 obs + 1 direction.
- **probe_adjacent_step0.jsonl** (ss-019d96a8-f109-7f63-848f-6908284842d4): 3 turns, status=Completed, model=gpt-5.4, provider=openai_codex. Agent: aj-019d96a8-f107. Created 4 step-0 obs + 1 direction; also 4 step-1 obs + 1 direction.

### Step 1 Probes (91-365 days)
No separate step 1 probe sessions were spawned. All 3 probes produced step 1 observations and directions within their initial sessions (3 turns total per probe, covering both steps).

## Notes
- 36 total observations (24 step 0, 12 step 1) across 3 probes
- 9 total directions (6 step 0, 3 step 1; 3 step 0 archived, 6 active)
- Orchestrator completed full loop: spawn probes -> wait -> convergence -> advance -> synthesis -> complete
- First successful full-pipeline run (Run 000 crashed on fuel exhaustion after 3 turns)
- Previous failed attempt (first Projection) had 6 probe sessions fail on provider auth errors
