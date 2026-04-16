#!/usr/bin/env python3
"""Compute Borda scores across 3 judges for the foresight meta-loop tournament.

The meta-agent MUST invoke this script rather than reimplement the calculation.
The script is git-tracked; any modification shows up in the commit diff.

Input: per-judge raw JSON files (from `claude -p` output) + scores.json with
the deterministic X/Y assignments recorded.

Output: borda.json with per-criterion and total Borda scores.

Borda: per criterion, the higher-scoring output gets 2 pts, loser gets 1 pt, ties
each get 1.5 pts. 3 judges × 12 criteria = max 72 pts per output.

Usage:
  python3 borda.py --run-dir meta/runs/001_foo/
"""

import argparse
import json
import sys
from pathlib import Path

CRITERIA = [
    "Specificity",
    "Novelty",
    "Falsifiability",
    "Breadth",
    "Plausibility",
    "Progression",
    "Actionability",
    "Decision Clarity",
    "Completeness",
    "Grounding",
    "Challenge",
    "Information Density",
]


def load_judge_raw(path: Path) -> dict:
    text = path.read_text().strip()
    # `claude -p --output-format json` nests the result; bare JSON is also accepted.
    try:
        first = json.loads(text)
    except json.JSONDecodeError as e:
        raise SystemExit(f"ERROR: {path} is not valid JSON: {e}")
    if isinstance(first, dict) and "result" in first and "criteria" not in first:
        inner = first["result"]
        if isinstance(inner, str):
            try:
                inner = json.loads(inner)
            except json.JSONDecodeError:
                raise SystemExit(f"ERROR: {path} nested result is not valid JSON")
        first = inner
    if "criteria" not in first:
        raise SystemExit(f"ERROR: {path} does not contain a 'criteria' array")
    return first


def compute(run_dir: Path) -> dict:
    scores_path = run_dir / "scores.json"
    if not scores_path.exists():
        raise SystemExit(f"ERROR: scores.json missing in {run_dir} — create it before running borda")
    scores = json.loads(scores_path.read_text())
    judges_meta = scores.get("judges", {})
    if len(judges_meta) < 1:
        raise SystemExit("ERROR: scores.json has no judges recorded")

    # Aggregate per-criterion results
    per_criterion = {c: {"challenger": 0.0, "incumbent": 0.0} for c in CRITERIA}
    judges_seen = 0

    for judge_key, jmeta in judges_meta.items():
        x_is_challenger = jmeta.get("x_is_challenger")
        if x_is_challenger is None:
            raise SystemExit(f"ERROR: {judge_key} missing x_is_challenger in scores.json")

        # Pull criteria list — either directly attached, or load from raw file
        crits = jmeta.get("criteria")
        if crits is None:
            raw_path = run_dir / f"{judge_key}_raw.json"
            if not raw_path.exists():
                raise SystemExit(f"ERROR: {raw_path} not found and no inline criteria")
            raw = load_judge_raw(raw_path)
            crits = raw["criteria"]

        judges_seen += 1
        for c in crits:
            name = c.get("criterion")
            if name not in per_criterion:
                continue  # ignore extras, don't fail — judges sometimes add notes
            x = c["output_x_score"]
            y = c["output_y_score"]
            if x_is_challenger:
                ch, inc = x, y
            else:
                ch, inc = y, x
            if ch > inc:
                per_criterion[name]["challenger"] += 2
                per_criterion[name]["incumbent"] += 1
            elif inc > ch:
                per_criterion[name]["challenger"] += 1
                per_criterion[name]["incumbent"] += 2
            else:
                per_criterion[name]["challenger"] += 1.5
                per_criterion[name]["incumbent"] += 1.5

    total_challenger = sum(v["challenger"] for v in per_criterion.values())
    total_incumbent = sum(v["incumbent"] for v in per_criterion.values())

    winner = "tie"
    if total_challenger > total_incumbent:
        winner = "challenger"
    elif total_incumbent > total_challenger:
        winner = "incumbent"

    max_per_output = judges_seen * len(CRITERIA) * 2  # 3 × 12 × 2 = 72

    return {
        "methodology_note": f"{judges_seen} judges, side-by-side blind comparison. Borda: winner=2pts, loser=1pt, tie=1.5. Max per output = judges × criteria × 2.",
        "judges_counted": judges_seen,
        "challenger_borda": total_challenger,
        "incumbent_borda": total_incumbent,
        "max_per_output": max_per_output,
        "winner": winner,
        "per_criterion": [
            {
                "criterion": c,
                "challenger": per_criterion[c]["challenger"],
                "incumbent": per_criterion[c]["incumbent"],
                "delta": per_criterion[c]["challenger"] - per_criterion[c]["incumbent"],
            }
            for c in CRITERIA
        ],
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run-dir", required=True, help="Path to meta/runs/NNN_xxx/")
    ap.add_argument("--out", help="Write borda.json here (default: run_dir/borda.json)")
    args = ap.parse_args()

    run_dir = Path(args.run_dir)
    if not run_dir.is_dir():
        raise SystemExit(f"ERROR: {run_dir} is not a directory")

    result = compute(run_dir)
    result["run_dir"] = str(run_dir)

    out_path = Path(args.out) if args.out else run_dir / "borda.json"
    out_path.write_text(json.dumps(result, indent=2) + "\n")
    print(f"Wrote {out_path}: challenger={result['challenger_borda']} incumbent={result['incumbent_borda']} winner={result['winner']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
