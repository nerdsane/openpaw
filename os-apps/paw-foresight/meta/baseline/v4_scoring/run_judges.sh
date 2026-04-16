#!/bin/bash
# Score baseline vs engine (Run 010) under rubric v4 with 3 Claude Code subagent judges
# Each judge sees both outputs side-by-side, randomized X/Y assignment
set -e

BASEDIR="/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-foresight/meta"
OUTDIR="$BASEDIR/baseline/v4_scoring"
BASELINE="$BASEDIR/baseline/synthesis.md"
ENGINE="$BASEDIR/runs/010_decision_analyst_session/engine-output/synthesis.md"
RUBRIC="$BASEDIR/program.md"

mkdir -p "$OUTDIR"

# Extract rubric criteria section (from "### 1. Specificity" through end of criteria)
# We'll include the full rubric in the prompt since there's no size limit

build_prompt() {
    local judge_num=$1
    local output_x_file=$2
    local output_y_file=$3
    local x_label=$4  # "engine" or "baseline"
    local y_label=$5

    cat <<PROMPT_EOF
You are an independent evaluator of two foresight projection outputs about "Directed Software Evolution."

Score BOTH outputs on all 12 criteria (0-4). Be strict. A 2 is the expected score for competent output.

## Evaluation Criteria (0-4 scale, 12 criteria, max 48 per output)

Calibration: 2=competent median, 3=genuinely impressive (most outputs won't reach this), 4=exceptional and rare.

**3+ CAP RULE:** No more than 3 criteria may score 3 or higher for any single output. If more than 3 initially qualify for 3+, demote the weakest to 2. Document which were demoted and why.

### 1. Specificity
Does it name real actors, dates, and quantitative thresholds?
0=No named entities/timelines/mechanisms | 1=Generic categories with vague timing | 2=Named actors OR approximate timelines, but not both | 3=Named actors + specific timelines + causal mechanisms | 4=Named actors + specific dates + quantitative thresholds

### 2. Novelty
Does it produce insights not in the input AND not obvious from general domain knowledge?
0=Only restates input | 1=Minor extensions using common knowledge | 2=1-2 original insights | 3=Multiple original insights from OUTSIDE the input with cross-signal connections | 4=Reframes the domain with non-obvious framework

### 3. Falsifiability
Are predictions stated so they can be proven wrong?
0=All hedged/vague/unfalsifiable | 1=Few checkable predictions | 2=Several with clear evaluation conditions | 3=Most name what would confirm/disconfirm them | 4=Explicit falsification criteria with dates and reasons

### 4. Breadth
How many distinct analytical dimensions, and are they connected?
0=Single theme | 1=2-3 independent themes | 2=4-5 themes with some connections | 3=6+ themes with explicit cross-theme interactions producing non-obvious conclusions | 4=Comprehensive system where removing one theme weakens others

### 5. Plausibility
Are claims grounded in named evidence?
0=Unsupported assertions | 1=Mix of grounded and floating; uncertainty rarely acknowledged | 2=Most reference mechanisms/evidence; some explicit uncertainty | 3=Named signals/evidence with confidence levels and stated assumptions | 4=Every claim traced to evidence with confidence levels, assumptions, AND what would change confidence

### 6. Progression
Genuine temporal development where later predictions build on earlier ones?
0=No temporal structure | 1=Time periods but independent snapshots | 2=Phases reference earlier ones superficially | 3=Each phase causally depends on prior AND explicitly revises/qualifies earlier predictions | 4=Later phases confirm, falsify, or revise earlier predictions — analysis evolves

### 7. Actionability
Could a specific decision-maker take a specific action?
0=No decision-relevant content | 1=Vague implications | 2=Conditional recommendations without timing/tradeoffs | 3=Decision points with timing triggers AND options (generic tradeoffs) | 4=Decision framework: who decides, when (observable triggers), options, costs/risks in concrete terms

### 8. Decision Clarity
Could a VP read this in 15 minutes and know the #1 thing to do, by when, and what it costs?
0=Raw data/unstructured | 1=Structured but needs expertise to extract decisions | 2=Clear narrative, "so what" implicit | 3=Prioritized findings with top recommendation and timing | 4=Opens with single most important decision, deadline, quantified tradeoff

### 9. Completeness
Full foresight pipeline from evidence to recommendation?
0=Observations only | 1=Observations + some theses | 2=Observations + theses + temporal development; assumptions implicit | 3=Full pipeline + explicit assumptions, limitations, confidence | 4=Full pipeline + assumptions + limitations + confidence + what-would-change-my-mind

### 10. Grounding
Does the evidence actually support the claim? (Not "is it cited?" but "does the reasoning chain hold?")
0=Claims asserted without evidence or reasoning | 1=Evidence present but doesn't logically support claims | 2=Most claims have relevant evidence but reasoning chain has gaps | 3=Claims supported with explicit reasoning chains — reader can follow evidence→conclusion | 4=Complete chains: evidence→mechanism→conclusion with stated assumptions and what would break it

### 11. Challenge
Does it make predictions that go against the source material's thesis?
0=Echo chamber | 1=Token caveats without substantive disagreement | 2=Genuine tensions or fragile assumptions identified | 3=Specific contradicting prediction with evidence from source + mechanism explanation | 4=Overturns source assumption using evidence from OUTSIDE the source

### 12. Information Density
Does every claim add unique analytical value? (Penalizes redundancy.)
0=>60% of claims restate others | 1=Significant redundancy, many claims mergeable | 2=Some redundancy but most claims contribute distinct info | 3=Every claim adds unique information; removing any reduces value | 4=Maximum compression — no overlap, every sentence earns its place

---

## Output X

$(cat "$output_x_file")

---

## Output Y

$(cat "$output_y_file")

---

## Task

Score BOTH outputs on all 12 criteria. For each criterion:
- Give a score (0-4) for each output
- Explain your reasoning with SPECIFIC evidence (quote or cite sections)
- Enforce the 3+ cap rule for each output independently

Return ONLY valid JSON (no markdown, no commentary outside JSON):

{"criteria": [
  {
    "criterion": "Specificity",
    "output_x_score": 2,
    "output_y_score": 3,
    "reasoning": "detailed comparison with specific evidence from both outputs...",
    "evidence_x": ["quote or cite specific sections from X"],
    "evidence_y": ["quote or cite specific sections from Y"]
  }
]}
PROMPT_EOF
}

echo "=== Scoring baseline vs engine under rubric v4 ==="
echo "Baseline: $BASELINE ($(wc -c < "$BASELINE") bytes)"
echo "Engine: $ENGINE ($(wc -c < "$ENGINE") bytes)"

for JUDGE_NUM in 1 2 3; do
    echo ""
    echo "--- Judge $JUDGE_NUM ---"

    # Randomize X/Y assignment: odd judges get X=engine, even get X=baseline
    if [ $((JUDGE_NUM % 2)) -eq 1 ]; then
        X_FILE="$ENGINE"
        Y_FILE="$BASELINE"
        X_LABEL="engine"
        Y_LABEL="baseline"
    else
        X_FILE="$BASELINE"
        Y_FILE="$ENGINE"
        X_LABEL="baseline"
        Y_LABEL="engine"
    fi

    PROMPT_FILE="$OUTDIR/judge_${JUDGE_NUM}_prompt.txt"
    RESULT_FILE="$OUTDIR/judge_${JUDGE_NUM}_raw.txt"
    MAPPING_FILE="$OUTDIR/judge_${JUDGE_NUM}_mapping.json"

    build_prompt "$JUDGE_NUM" "$X_FILE" "$Y_FILE" "$X_LABEL" "$Y_LABEL" > "$PROMPT_FILE"
    echo "{\"x_is\": \"$X_LABEL\", \"y_is\": \"$Y_LABEL\"}" > "$MAPPING_FILE"

    echo "Prompt: $(wc -c < "$PROMPT_FILE") bytes (X=$X_LABEL, Y=$Y_LABEL)"
    echo "Running claude -p..."

    # Run the judge — capture full output
    claude -p "$(cat "$PROMPT_FILE")" --output-format text \
        > "$RESULT_FILE" 2>/dev/null || {
        echo "Judge $JUDGE_NUM FAILED (exit code $?)"
        continue
    }

    echo "Judge $JUDGE_NUM complete: $(wc -c < "$RESULT_FILE") bytes"
done

echo ""
echo "=== All judges complete. Results in $OUTDIR ==="
