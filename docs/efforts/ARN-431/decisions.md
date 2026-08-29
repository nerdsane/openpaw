## Decisions & Tradeoffs

**Decision:** The planning and decision-log gates' "Temper verdict" in the sweep
is a mirror computed from PR content with the gates' own stack scripts, not read
from entity state.
**Came up because:** S0 only added record entities for review and proof; there is
no planning or decision-log entity, but the shadow needs a Temper verdict for all
four gates to compare against CI.
**Options:** invent planning/decision entities now (rejected - that is new S0
work, and this phase changes no entities); skip those two gates (rejected - the
shadow must cover every required gate); recompute them from PR content with the
same gate scripts (chosen).
**Chose the mirror because:** it keeps the phase entity-free and still produces a
verdict for all four gates. It is stated plainly (spec + README) that review and
proof are the true state-machine shadow while planning and decisions are a
consistency check on the gate scripts - so a planning/decisions disagreement would
mean the script is non-deterministic, not that Temper and CI differ. Given up: a
"pure" entity-derived verdict for those two, which waits for the Effort state
machine (a later phase).
**Where:** `stack/shadow/shadow-sweep.py`, `docs/efforts/ARN-431/spec.md`.
