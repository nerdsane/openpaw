#!/usr/bin/env python3
"""Deterministically assign X/Y to challenger/incumbent per judge.

Why deterministic instead of random: the meta-agent generates the judge prompts,
so a random-number-based assignment could be fudged. A hash over
(run_number, judge_number, git_HEAD_sha) is unspoofable — any auditor can recompute
and verify. The assignment is unpredictable enough to prevent targeted bias
(agent doesn't know the hash until HEAD is committed), but fully reproducible
after the fact.

Usage:
  python3 assign_judges.py --run NNN --head <git-sha>
  python3 assign_judges.py --run 1 --head 9a59b3b5 --verify scores.json
"""

import argparse
import hashlib
import json
import sys
from pathlib import Path


def x_is_challenger(run_num: int, judge_num: int, head_sha: str) -> bool:
    """Hash (run_num || judge_num || head_sha) → first byte even → X=challenger."""
    payload = f"{run_num}:{judge_num}:{head_sha}".encode("utf-8")
    digest = hashlib.sha256(payload).digest()
    return (digest[0] % 2) == 0


def derive_all(run_num: int, head_sha: str) -> list[dict]:
    return [
        {
            "judge": j,
            "x_is_challenger": x_is_challenger(run_num, j, head_sha),
            "derivation": f"sha256('{run_num}:{j}:{head_sha}')[0] % 2 == 0",
        }
        for j in (1, 2, 3)
    ]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", type=int, required=True, help="Run number (integer, not zero-padded)")
    ap.add_argument("--head", required=True, help="git HEAD sha (full or abbreviated, same for all judges in this run)")
    ap.add_argument(
        "--verify",
        help="Path to a scores.json file. Check that its judge assignments match the deterministic derivation.",
    )
    args = ap.parse_args()

    derived = derive_all(args.run, args.head)

    if args.verify:
        scores_path = Path(args.verify)
        if not scores_path.exists():
            print(f"ERROR: scores file not found: {scores_path}", file=sys.stderr)
            return 2
        scores = json.loads(scores_path.read_text())
        judges = scores.get("judges", {})
        mismatches = []
        for d in derived:
            key = f"judge_{d['judge']}"
            actual = judges.get(key, {}).get("x_is_challenger")
            if actual is None:
                mismatches.append(f"{key}: missing x_is_challenger")
            elif bool(actual) != d["x_is_challenger"]:
                mismatches.append(
                    f"{key}: recorded x_is_challenger={actual}, derivation says {d['x_is_challenger']}"
                )
        if mismatches:
            print("ASSIGNMENT MISMATCH:", file=sys.stderr)
            for m in mismatches:
                print(f"  {m}", file=sys.stderr)
            return 1
        print("OK: all 3 judge assignments match deterministic derivation")
        return 0

    # Plain emit mode: print JSON the meta-agent uses when building prompts
    print(json.dumps({"run": args.run, "head_sha": args.head, "judges": derived}, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
