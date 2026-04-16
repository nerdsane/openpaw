# Run 000 Session Transcripts

Extracted from `~/.local/share/openpaw/paw.db` snapshots + blobs tables.

## Engine Pipeline

| File | Session ID | Role | Turns | Status |
|------|-----------|------|-------|--------|
| orchestrator.jsonl | ss-019d9350-9059-... | Main orchestrator (spawns probes, converges, synthesizes) | 21 | Completed |
| failed-probe-attempt-1.jsonl | ss-019d9351-7f2b-... | First probe spawn attempt (failed) | 0 | Failed |
| failed-probe-attempt-2.jsonl | ss-019d9351-b4c6-... | Second probe spawn attempt (failed) | 0 | Failed |
| probe1-step0.jsonl | ss-019d9351-9246-... | Probe 1, step 0 (90 days) | 3 | Completed |
| probe2-step0.jsonl | ss-019d9351-9253-... | Probe 2, step 0 (90 days) | 4 | Completed |
| probe3-step0.jsonl | ss-019d9351-9260-... | Probe 3, step 0 (90 days) | 3 | Completed |
| probe1-step1.jsonl | ss-019d9353-a4c7-... | Probe 1, step 1 (365 days) | 1 | Completed |
| probe2-step1.jsonl | ss-019d9353-a505-... | Probe 2, step 1 (365 days) | 1 | Completed |
| probe3-step1.jsonl | ss-019d9353-a527-... | Probe 3, step 1 (365 days) | 1 | Completed |
| synthesis.jsonl | ss-019d9354-3f9b-... | Final synthesis session | - | Completed |

## Baseline

| File | Session ID | Role | Turns | Status |
|------|-----------|------|-------|--------|
| baseline.jsonl | ss-019d9356-99c7-... | Single-shot baseline | 3 | Completed |

## Judges — Round 1 (failed, pre-rubric-v3)

| File | Session ID | Role | Turns | Status |
|------|-----------|------|-------|--------|
| judge1.jsonl | ss-019d935a-eec7-... | Judge session 1 | 2 | Completed (unparseable) |
| judge2.jsonl | ss-019d935a-eef7-... | Judge session 2 | 2 | Completed (unparseable) |
| judge3.jsonl | ss-019d935a-ef28-... | Judge session 3 | 1 | Completed (unparseable) |

## Judges — Round 2 (rubric v3, 3+ cap rule, split-session)

| File | Session ID | Role | Turns | Status |
|------|-----------|------|-------|--------|
| (result field) | ss-019d9444-a7c3-... | Judge 1 — engine scoring | 1 | Has result |
| (result field) | ss-019d9444-a7cd-... | Judge 1 — baseline scoring | 1 | Has result |
| (result field) | ss-019d9444-a7da-... | Judge 2 — engine scoring | 1 | Has result |
| (result field) | ss-019d9444-a7e9-... | Judge 2 — baseline scoring | 1 | Has result |
| (result field) | ss-019d9444-a7f6-... | Judge 3 — engine scoring | 1 | Has result |
| (result field) | ss-019d9444-a7fe-... | Judge 3 — baseline scoring | 1 | Has result |

Note: Round 2 judges used split-session approach (one session per output per judge) to stay under 32KB WASM field limit. Scores extracted from entity `result` field; sessions stuck in Steering state so JSONL transcripts not finalized.

## Config

- Model: gpt-5.4 / openai_codex (all sessions)
- Engine: 3 probes, 2 steps (90d, 365d)
- Baseline prompt: `../../baseline/prompt.md`
