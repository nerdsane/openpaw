#!/usr/bin/env python3
"""Offline calibration of the corridor cost constants (ADR-004 commitment 1).

Pure arithmetic over recorded run records: no LLM calls, no server. The
grid search RECOMPUTES costs and classifications deterministically from the
flags recorded at run time — it never re-runs models. The flags themselves
are taken as given; if the flagging behavior changes, the calibration must
be redone on fresh records.

Run-record schema (one JSON file per hindcast/corridor run; produced by the
C6 harness — see --emit-template for an annotated example):

    {
      "run_id": "optional free-form label",
      "hindcast": {
        "mean_brier": 0.18,   # mean Brier over graded forecasts
        "graded": 9,          # forecasts graded against actuals
        "total": 14           # forecasts registered
      },
      "claims": [
        {
          "id": "c-1",
          "settled_cost": 17.5,          # best_route_cost recorded at run time
          "classification": "reachable", # "reachable" | "strained" at run time
          "flags": [                      # concatenation of the settled route's flags (a flag present in both lists counts twice)
            {"kind": "contradiction", "severity": "high"},
            {"kind": "lag", "severity": "low"}
          ],
          "outcome_correct": true        # true/false once reality graded the
                                          # claim; null when still ungraded
        }
      ],
      "paths": [
        {
          "id": "p-1",
          "flags": [{"kind": "miracle", "severity": "medium"}],
          "scored_cost": 40.0,           # repair_cost recorded at run time
          "classification": "canonical"  # "canonical" | "tail" | "reject"
        }
      ]
    }

Grid dimensions (current hardcoded values in
os-apps/paw-foresight/wasm/aggregate_costs/src/lib.rs — keep in sync):
  - kind weights: miracle, contradiction, deformation, incentive, lag
  - severity multipliers: low, high (medium is fixed at 1.0)
  - decay divisor d in endpoint weight exp(-cost / d)
  - tail ceiling (a, b): a route is Tail when cost <= a * min_cost + b

Ordering constraint enforced over the grid (ADR-004's ordering plus a placement choice for deformation (the ADR lists the deformation weight itself as tunable)):
  miracle > contradiction >= deformation > incentive >= lag

Scoring per grid point:
  1. mean Brier over graded forecasts — UNCHANGED by these constants
     (forecast probabilities were registered before costing); reported as a
     constant for honesty, never used to rank grid points.
  2. classification accuracy: fraction of claims (with non-null
     outcome_correct) where (recomputed classification == "reachable")
     == outcome_correct.
  3. ranking stability: mean Kendall tau, per run, between the path ranking
     under the candidate constants and the ranking recorded at run time
     (scored_cost) — high tau means re-tuning does not whipsaw past runs.
  4. tail agreement: fraction of paths whose recomputed
     canonical/tail/reject class (under the candidate tail ceiling) matches
     the recorded one — the only signal the tail ceiling has.

The decay divisor is NOT identifiable from these records: exp(-cost/d) is
monotone in cost for every d > 0, so no ranking-based metric can prefer one
divisor over another. Ties are broken toward the current hardcoded values,
so the report keeps d = 25 and says so.

Usage:
  python3 scripts/calibrate_constants.py --runs 'records/*.json' --out report.md
  python3 scripts/calibrate_constants.py --emit-template
  python3 scripts/calibrate_constants.py --self-test
"""

from __future__ import annotations

import argparse
import glob
import itertools
import json
import sys
from dataclasses import dataclass, field, fields

# --- Current hardcoded values --------------------------------------------------
# Source of truth: os-apps/paw-foresight/wasm/aggregate_costs/src/lib.rs
#   kind_weight():       miracle 40, contradiction 25, deformation 25,
#                        incentive 15, lag 10 (unknown kinds price as
#                        contradiction)
#   severity_multiplier(): low 0.5, high 2.0, anything else 1.0
#   endpoint_weights():  exp(-cost / 25.0)
#   classify():          tail ceiling = min_cost * 2.0 + 20.0
#   REACHABLE_MAX:       30.0 (claim classification split; fixed, not tuned
#                        here — it is the unit the weights are expressed in)
REACHABLE_MAX = 30.0


@dataclass(frozen=True)
class Constants:
    miracle: float = 40.0
    contradiction: float = 25.0
    deformation: float = 25.0
    incentive: float = 15.0
    lag: float = 10.0
    sev_low: float = 0.5
    sev_high: float = 2.0
    decay_divisor: float = 25.0
    tail_a: float = 2.0
    tail_b: float = 20.0

    def ordering_ok(self) -> bool:
        return (
            self.miracle > self.contradiction >= self.deformation
            and self.deformation > self.incentive >= self.lag
            and self.sev_low < 1.0 < self.sev_high
        )


CURRENT = Constants()

# The search grid. Small and centered on the current values: with the tiny
# run library we have, a wide grid would just overfit noise.
GRID = {
    "miracle": [30.0, 40.0, 50.0],
    "contradiction": [20.0, 25.0, 30.0],
    "deformation": [15.0, 20.0, 25.0],
    "incentive": [10.0, 15.0],
    "lag": [5.0, 10.0, 15.0],
    "sev_low": [0.25, 0.5],
    "sev_high": [1.5, 2.0, 3.0],
    "decay_divisor": [15.0, 25.0, 35.0],
    "tail_ab": [(1.5, 10.0), (2.0, 20.0), (2.5, 30.0)],
}


# --- Recompute math (mirrors aggregate_costs exactly) ---------------------------


def kind_weight(kind: str, c: Constants) -> float:
    return {
        "miracle": c.miracle,
        "contradiction": c.contradiction,
        "deformation": c.deformation,
        "incentive": c.incentive,
        "lag": c.lag,
        # Unknown kinds price like contradictions so malformed flags are
        # never free (mirrors aggregate_costs::kind_weight's fallback arm).
    }.get(kind, c.contradiction)


def severity_multiplier(severity: str, c: Constants) -> float:
    return {"low": c.sev_low, "high": c.sev_high}.get(severity, 1.0)


def recompute_cost(flags: list[dict], c: Constants) -> float:
    """Sum over the concatenation of recorded flags of kind_weight x severity
    multiplier — the exact formula of aggregate_costs::repair_cost."""
    return sum(
        kind_weight(f.get("kind", ""), c)
        * severity_multiplier(f.get("severity", "medium"), c)
        for f in flags
    )


def claim_classification(cost: float) -> str:
    return "reachable" if cost <= REACHABLE_MAX else "strained"


def classify_paths(costed: list[tuple[str, float]], c: Constants) -> dict[str, str]:
    """aggregate_costs::classify: exactly one canonical (cheapest, ties by
    lexicographic id); tail when cost <= a * min + b; else reject."""
    if not costed:
        return {}
    ordered = sorted(costed, key=lambda x: (x[1], x[0]))
    canonical_id, min_cost = ordered[0]
    ceiling = c.tail_a * min_cost + c.tail_b
    out = {}
    for pid, cost in ordered:
        if pid == canonical_id:
            out[pid] = "canonical"
        elif cost <= ceiling:
            out[pid] = "tail"
        else:
            out[pid] = "reject"
    return out


def kendall_tau(a: list[float], b: list[float]) -> float | None:
    """Pairwise concordance between two cost vectors over the same items.
    Pairs tied in either vector are skipped; None when no usable pairs."""
    n = len(a)
    concordant = discordant = 0
    for i in range(n):
        for j in range(i + 1, n):
            da, db = a[i] - a[j], b[i] - b[j]
            if da == 0 or db == 0:
                continue
            if (da > 0) == (db > 0):
                concordant += 1
            else:
                discordant += 1
    total = concordant + discordant
    if total == 0:
        return None
    return (concordant - discordant) / total


# --- Scoring ---------------------------------------------------------------------


@dataclass
class Scores:
    classification_accuracy: float | None
    ranking_stability: float | None
    tail_agreement: float | None
    n_claims_graded: int
    n_paths: int


def score_grid_point(runs: list[dict], c: Constants) -> Scores:
    correct = total = 0
    taus: list[float] = []
    tail_hits = tail_total = 0
    for run in runs:
        for claim in run.get("claims", []):
            outcome = claim.get("outcome_correct")
            if outcome is None:
                continue
            cost = recompute_cost(claim.get("flags", []), c)
            reachable = claim_classification(cost) == "reachable"
            correct += int(reachable == bool(outcome))
            total += 1
        paths = run.get("paths", [])
        if paths:
            recomputed = [recompute_cost(p.get("flags", []), c) for p in paths]
            recorded = [float(p.get("scored_cost", 0.0)) for p in paths]
            tau = kendall_tau(recomputed, recorded)
            if tau is not None:
                taus.append(tau)
            costed = [(p.get("id", str(i)), rc) for i, (p, rc) in enumerate(zip(paths, recomputed))]
            classes = classify_paths(costed, c)
            for i, p in enumerate(paths):
                rec = str(p.get("classification", "")).lower()
                if rec == "rejected":
                    rec = "reject"
                if rec in ("canonical", "tail", "reject"):
                    tail_total += 1
                    tail_hits += int(classes.get(p.get("id", str(i))) == rec)
    return Scores(
        classification_accuracy=(correct / total) if total else None,
        ranking_stability=(sum(taus) / len(taus)) if taus else None,
        tail_agreement=(tail_hits / tail_total) if tail_total else None,
        n_claims_graded=total,
        n_paths=tail_total,
    )


def distance_from_current(c: Constants) -> float:
    """Normalized L1 distance from the current hardcoded values — the tie
    breaker. When the data cannot tell two grid points apart, keep the
    constants we already ship."""
    return sum(
        abs(getattr(c, f.name) - getattr(CURRENT, f.name)) / max(abs(getattr(CURRENT, f.name)), 1.0)
        for f in fields(Constants)
    )


def rank_key(s: Scores, c: Constants) -> tuple:
    # Lexicographic: accuracy first, then tail agreement, then stability,
    # then proximity to the shipped constants. Missing metrics rank as the
    # worst value so an empty record set never "prefers" anything.
    return (
        -(s.classification_accuracy if s.classification_accuracy is not None else -1.0),
        -(s.tail_agreement if s.tail_agreement is not None else -1.0),
        -(s.ranking_stability if s.ranking_stability is not None else -2.0),
        distance_from_current(c),
    )


def grid_points() -> list[Constants]:
    points = []
    for m, co, de, inc, la, lo, hi, dd, (ta, tb) in itertools.product(
        GRID["miracle"],
        GRID["contradiction"],
        GRID["deformation"],
        GRID["incentive"],
        GRID["lag"],
        GRID["sev_low"],
        GRID["sev_high"],
        GRID["decay_divisor"],
        GRID["tail_ab"],
    ):
        c = Constants(
            miracle=m, contradiction=co, deformation=de, incentive=inc, lag=la,
            sev_low=lo, sev_high=hi, decay_divisor=dd, tail_a=ta, tail_b=tb,
        )
        if c.ordering_ok():
            points.append(c)
    return points


# --- Report ------------------------------------------------------------------------


def overall_brier(runs: list[dict]) -> tuple[float | None, int]:
    """Graded-count-weighted mean Brier across runs. Constant w.r.t. the
    grid: probabilities were registered before any costing."""
    num = den = 0.0
    for run in runs:
        h = run.get("hindcast") or {}
        graded = float(h.get("graded", 0) or 0)
        mb = h.get("mean_brier")
        if mb is None or graded <= 0:
            continue
        num += float(mb) * graded
        den += graded
    return (num / den if den else None), int(den)


def fmt(v, nd=4) -> str:
    if v is None:
        return "n/a"
    if isinstance(v, float):
        return f"{v:.{nd}f}"
    return str(v)


def build_report(runs: list[dict], run_files: list[str]) -> str:
    points = grid_points()
    scored = [(c, score_grid_point(runs, c)) for c in points]
    scored.sort(key=lambda cs: rank_key(cs[1], cs[0]))
    best_c, best_s = scored[0]
    current_s = score_grid_point(runs, CURRENT)
    mean_brier, graded_n = overall_brier(runs)

    # How many grid points tie with the best on every metric? Honesty about
    # how little the data constrains the choice.
    best_key = rank_key(best_s, best_c)[:3]
    tied = sum(1 for c, s in scored if rank_key(s, c)[:3] == best_key)

    rows = []
    for name, cur, new in [
        ("miracle weight", CURRENT.miracle, best_c.miracle),
        ("contradiction weight", CURRENT.contradiction, best_c.contradiction),
        ("deformation weight", CURRENT.deformation, best_c.deformation),
        ("incentive weight", CURRENT.incentive, best_c.incentive),
        ("lag weight", CURRENT.lag, best_c.lag),
        ("severity low", CURRENT.sev_low, best_c.sev_low),
        ("severity high", CURRENT.sev_high, best_c.sev_high),
        ("decay divisor", CURRENT.decay_divisor, best_c.decay_divisor),
        ("tail ceiling a", CURRENT.tail_a, best_c.tail_a),
        ("tail ceiling b", CURRENT.tail_b, best_c.tail_b),
    ]:
        delta = "unchanged" if cur == new else f"{cur:g} -> {new:g}"
        rows.append(f"| {name} | {cur:g} | {new:g} | {delta} |")

    lines = [
        "# Corridor cost-constant calibration (priors v2 candidate)",
        "",
        f"Tuned on **{len(runs)} run record(s)** "
        f"({best_s.n_claims_graded} ground-truthed claims, {best_s.n_paths} "
        f"classified paths, {graded_n} graded forecasts).",
        "",
        "Input files:",
        "",
        *[f"- `{p}`" for p in run_files],
        "",
        "## Chosen constants",
        "",
        "| constant | current (hardcoded) | chosen | change |",
        "|---|---|---|---|",
        *rows,
        "",
        "Ordering constraint enforced over the whole grid: "
        "`miracle > contradiction >= deformation > incentive >= lag`.",
        "",
        "## Metrics",
        "",
        "| metric | current constants | chosen constants |",
        "|---|---|---|",
        f"| mean Brier (graded forecasts) | {fmt(mean_brier)} | {fmt(mean_brier)} |",
        f"| classification accuracy | {fmt(current_s.classification_accuracy)} "
        f"| {fmt(best_s.classification_accuracy)} |",
        f"| tail agreement | {fmt(current_s.tail_agreement)} | {fmt(best_s.tail_agreement)} |",
        f"| ranking stability (mean Kendall tau) | {fmt(current_s.ranking_stability)} "
        f"| {fmt(best_s.ranking_stability)} |",
        "",
        "## Honest caveats",
        "",
        f"- **Tiny n.** {best_s.n_claims_graded} ground-truthed claims across "
        f"{len(runs)} run(s) cannot pin down 10 constants; treat the chosen "
        "values as priors v2, not a calibration result. Re-run as the "
        "hindcast library grows.",
        "- **Mean Brier is constant by construction.** Forecast probabilities "
        "are registered before costing, so the Brier column cannot move with "
        "these constants — it is shown so nobody mistakes this for a "
        "forecast-accuracy improvement.",
        "- **The decay divisor is unidentifiable from these records**: "
        "exp(-cost/d) is monotone in cost for every d, so no ranking-based "
        "metric can distinguish divisors. The tie-break keeps the current "
        "value.",
        f"- **{tied} grid point(s) tie on every metric**; ties are broken "
        "toward the currently shipped constants (smallest change wins), so "
        "an uninformative record set returns the status quo.",
        "- Costs are recomputed from the flags recorded at run time; the "
        "flagging behavior itself (which flags sessions raise) is held "
        "fixed. Models are never re-run.",
        "",
        "If adopted, update `os-apps/paw-foresight/wasm/aggregate_costs/src/"
        "lib.rs` (kind_weight, severity_multiplier, endpoint_weights, "
        "classify) and keep this script's CURRENT block in sync.",
        "",
    ]
    return "\n".join(lines)


# --- Template -----------------------------------------------------------------------

TEMPLATE = {
    "run_id": "hindcast-2024h2-run-001",
    "hindcast": {"mean_brier": 0.18, "graded": 9, "total": 14},
    "claims": [
        {
            "id": "c-1",
            "settled_cost": 17.5,
            "classification": "reachable",
            "flags": [
                {"kind": "contradiction", "severity": "low"},
                {"kind": "lag", "severity": "low"},
            ],
            "outcome_correct": True,
        },
        {
            "id": "c-2",
            "settled_cost": 55.0,
            "classification": "strained",
            "flags": [
                {"kind": "miracle", "severity": "medium"},
                {"kind": "incentive", "severity": "medium"},
            ],
            "outcome_correct": False,
        },
        {
            "id": "c-3",
            "settled_cost": 25.0,
            "classification": "reachable",
            "flags": [{"kind": "deformation", "severity": "medium"}],
            "outcome_correct": None,
        },
    ],
    "paths": [
        {
            "id": "p-1",
            "flags": [{"kind": "lag", "severity": "low"}],
            "scored_cost": 5.0,
            "classification": "canonical",
        },
        {
            "id": "p-2",
            "flags": [{"kind": "contradiction", "severity": "medium"}],
            "scored_cost": 25.0,
            "classification": "tail",
        },
        {
            "id": "p-3",
            "flags": [
                {"kind": "miracle", "severity": "high"},
                {"kind": "contradiction", "severity": "medium"},
            ],
            "scored_cost": 105.0,
            "classification": "reject",
        },
    ],
}

TEMPLATE_NOTES = """\
# Field notes (the JSON above is a valid run record):
#   hindcast.mean_brier / graded / total — copied from the Hindcast entity
#     after Grade (ScoreComplete params).
#   claims[].flags — the concatenation of cost_flags + challenge_flags (a flag present in both counts twice) on the
#     claim's settled route, as recorded; kinds: miracle | contradiction |
#     deformation | incentive | lag; severities: low | medium | high.
#   claims[].outcome_correct — true/false once reality graded the claim
#     (was a "reachable" claim actually right?); null while ungraded.
#     Claims with null are excluded from classification accuracy.
#   claims[].settled_cost / classification — what the engine recorded at
#     run time (for reference and drift checks; the script recomputes).
#   paths[].scored_cost / classification — repair_cost and the
#     canonical/tail/reject class recorded at run time.
"""


# --- Self-test ------------------------------------------------------------------------


def self_test() -> int:
    """Assert the recompute math matches aggregate_costs' formula using the
    same vectors as its Rust unit tests (aggregate_costs/src/lib.rs)."""
    c = CURRENT

    # cost_sums_kind_weights_times_severity: 25*2 + 10*0.5 + 15*1 = 70
    flags = [
        {"kind": "contradiction", "severity": "high"},
        {"kind": "lag", "severity": "low"},
        {"kind": "incentive", "severity": "medium"},
    ]
    assert recompute_cost(flags, c) == 70.0, recompute_cost(flags, c)

    # 40 + 10*2 = 60
    flags = [{"kind": "miracle", "severity": "medium"}, {"kind": "lag", "severity": "high"}]
    assert recompute_cost(flags, c) == 60.0

    # Unknown kinds price like contradictions; missing severity is medium.
    assert recompute_cost([{"kind": "mystery", "severity": "medium"}], c) == 25.0
    assert recompute_cost([{"kind": "deformation"}], c) == 25.0
    assert recompute_cost([], c) == 0.0

    # deformation prices like contradiction at current constants: 25*2 = 50
    assert recompute_cost([{"kind": "deformation", "severity": "high"}], c) == 50.0

    # Claim classification split (REACHABLE_MAX = 30, inclusive).
    assert claim_classification(30.0) == "reachable"
    assert claim_classification(30.1) == "strained"
    assert claim_classification(0.0) == "reachable"

    # classify: mirrors classification_is_deterministic_with_lexicographic_tiebreak
    classes = classify_paths(
        [("p-b", 10.0), ("p-a", 10.0), ("p-c", 35.0), ("p-d", 41.0)], c
    )
    assert classes == {"p-a": "canonical", "p-b": "tail", "p-c": "tail", "p-d": "reject"}, classes

    # Kendall tau sanity.
    assert kendall_tau([1, 2, 3], [10, 20, 30]) == 1.0
    assert kendall_tau([1, 2, 3], [30, 20, 10]) == -1.0
    assert kendall_tau([1, 1, 1], [1, 2, 3]) is None

    # Ordering constraint: the current constants satisfy it; a violation
    # does not survive the grid filter.
    assert CURRENT.ordering_ok()
    assert all(p.ordering_ok() for p in grid_points())
    bad = Constants(miracle=20.0, contradiction=25.0)
    assert not bad.ordering_ok()

    # End to end over synthetic records: a claim flagged once with
    # contradiction/medium costs 25 (reachable, correct) and a heavy one
    # costs 80 (strained, incorrect outcome) -> accuracy 1.0 at current
    # constants.
    synthetic = [
        {
            "run_id": "synthetic-1",
            "hindcast": {"mean_brier": 0.2, "graded": 4, "total": 6},
            "claims": [
                {
                    "id": "c-1",
                    "settled_cost": 25.0,
                    "classification": "reachable",
                    "flags": [{"kind": "contradiction", "severity": "medium"}],
                    "outcome_correct": True,
                },
                {
                    "id": "c-2",
                    "settled_cost": 80.0,
                    "classification": "strained",
                    "flags": [
                        {"kind": "miracle", "severity": "high"},
                        # NOTE: 40*2 = 80 at current constants
                    ],
                    "outcome_correct": False,
                },
                {
                    "id": "c-3",
                    "settled_cost": 10.0,
                    "classification": "reachable",
                    "flags": [{"kind": "lag", "severity": "medium"}],
                    "outcome_correct": None,  # ungraded: excluded
                },
            ],
            "paths": [
                {"id": "p-1", "flags": [{"kind": "lag", "severity": "low"}],
                 "scored_cost": 5.0, "classification": "canonical"},
                {"id": "p-2", "flags": [{"kind": "contradiction", "severity": "medium"}],
                 "scored_cost": 25.0, "classification": "tail"},
                {"id": "p-3", "flags": [{"kind": "miracle", "severity": "high"},
                                         {"kind": "contradiction", "severity": "medium"}],
                 "scored_cost": 105.0, "classification": "reject"},
            ],
        }
    ]
    s = score_grid_point(synthetic, c)
    assert s.n_claims_graded == 2, s
    assert s.classification_accuracy == 1.0, s
    assert s.ranking_stability == 1.0, s
    assert s.tail_agreement == 1.0, s

    # Recorded settled_cost matches the recompute at current constants for
    # every synthetic claim — the harness and this script agree on the math.
    for claim in synthetic[0]["claims"]:
        assert recompute_cost(claim["flags"], c) == claim["settled_cost"], claim["id"]
    for path in synthetic[0]["paths"]:
        assert recompute_cost(path["flags"], c) == path["scored_cost"], path["id"]

    # Brier aggregation is graded-weighted and constant across the grid.
    mb, n = overall_brier(synthetic)
    assert mb == 0.2 and n == 4

    # The emitted template must itself be internally consistent: every
    # recorded cost matches the recompute at the current constants.
    for claim in TEMPLATE["claims"]:
        assert recompute_cost(claim["flags"], c) == claim["settled_cost"], claim["id"]
    for path in TEMPLATE["paths"]:
        assert recompute_cost(path["flags"], c) == path["scored_cost"], path["id"]

    # The full report builds, and on perfectly-explained synthetic data the
    # tie-break returns the shipped constants unchanged.
    report = build_report(synthetic, ["<synthetic>"])
    assert "unchanged" in report and "Tiny n" in report

    print("self-test OK: recompute math matches aggregate_costs' formula")
    return 0


# --- CLI -----------------------------------------------------------------------------


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--runs", help="glob of run-record JSON files")
    ap.add_argument("--out", help="write the markdown calibration report here")
    ap.add_argument("--emit-template", action="store_true",
                    help="print an annotated example run record and exit")
    ap.add_argument("--self-test", action="store_true",
                    help="assert the recompute math against aggregate_costs' "
                         "test vectors and exit")
    args = ap.parse_args()

    if args.self_test:
        return self_test()
    if args.emit_template:
        print(json.dumps(TEMPLATE, indent=2))
        print()
        print(TEMPLATE_NOTES, end="")
        return 0
    if not args.runs:
        ap.error("--runs is required (or use --emit-template / --self-test)")

    run_files = sorted(glob.glob(args.runs))
    if not run_files:
        print(f"no run records match {args.runs!r}", file=sys.stderr)
        return 1
    runs = []
    for path in run_files:
        with open(path) as fh:
            runs.append(json.load(fh))

    report = build_report(runs, run_files)
    if args.out:
        with open(args.out, "w") as fh:
            fh.write(report)
        print(f"wrote {args.out}")
    else:
        print(report)
    return 0


if __name__ == "__main__":
    sys.exit(main())
