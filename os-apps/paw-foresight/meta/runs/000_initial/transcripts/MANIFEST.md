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

## Judges (failed — see diagnosis.md)

| File | Session ID | Role | Turns | Status |
|------|-----------|------|-------|--------|
| judge1.jsonl | ss-019d935a-eec7-... | Judge session 1 | 2 | Completed |
| judge2.jsonl | ss-019d935a-eef7-... | Judge session 2 | 2 | Completed |
| judge3.jsonl | ss-019d935a-ef28-... | Judge session 3 | 1 | Completed |

## Config

- Model: gpt-5.4 / openai_codex (all sessions)
- Engine: 3 probes, 2 steps (90d, 365d)
- Baseline prompt: `../../baseline/prompt.md`
