<!--
LOCKED judge prompt template for the foresight meta-loop.

This file is git-tracked and its sha256 is recorded in every run's scores.json.
The meta-agent MUST substitute the {{PLACEHOLDERS}} below and NOT alter any
other text. verify_run.py enforces the sha match.

Placeholders (exact strings, case-sensitive):
  {{RUBRIC}}          — pasted verbatim from meta/program.md "Criteria" section
  {{OUTPUT_X}}        — full text of the output assigned to X for this judge
  {{OUTPUT_Y}}        — full text of the output assigned to Y for this judge

Do NOT wrap placeholders in extra framing. Do NOT add preamble, footer, or
commentary outside the template. Any deviation invalidates the run.
-->

You are an independent evaluator of two foresight projection outputs.
Score BOTH outputs on all 12 criteria (0-4). Be strict. Enforce the 3+ cap rule.

## Evaluation Criteria (0-4 scale, 12 criteria, max 48)

Calibration: 2 = competent median, 3 = genuinely impressive, 4 = exceptional and rare.

**3+ CAP RULE:** No more than 3 criteria may score 3+ for any single output.
If more than 3 qualify, demote the weakest to 2. Document demotions.

{{RUBRIC}}

## Output X

{{OUTPUT_X}}

## Output Y

{{OUTPUT_Y}}

## Task

Score both outputs on all 12 criteria. For each criterion, explain your reasoning
with specific evidence from both outputs. Return JSON only — no prose before or
after, no markdown fences, just the JSON object:

{"criteria": [
  {
    "criterion": "<criterion name exactly as in rubric>",
    "output_x_score": <integer 0-4>,
    "output_y_score": <integer 0-4>,
    "reasoning": "<one or two sentences explaining the scores>",
    "evidence_x": ["<verbatim quote or specific reference from Output X>", ...],
    "evidence_y": ["<verbatim quote or specific reference from Output Y>", ...]
  },
  ...
], "demotions": [
  {"criterion": "<name>", "original": <score>, "demoted_to": <score>, "reason": "3+ cap rule"}
]}

Do not include any text outside the JSON object. Do not identify which output is
newer, which looks better to you, or offer an overall verdict. Score per criterion
and return the JSON.
