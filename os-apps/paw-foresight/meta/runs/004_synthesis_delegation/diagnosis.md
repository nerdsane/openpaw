# Run 004 Diagnosis

## Summary

**Engine: 27.0/48 | Baseline: 26.0/48 | Delta: +1.0 raw**
**Engine Borda: 55.5/72 | Baseline Borda: 52.5/72 | Delta: +3.0**
**Winner: Engine** (second consecutive engine win)

The synthesis delegation change worked as intended: the orchestrator completed in 13 turns
without crashing (vs crash at turn 16 in Run 003). However, it did NOT spawn a separate
synthesis session — instead, it performed synthesis directly within its own context. The
context overflow was avoided because the WASM instructions were restructured into a shorter
orchestration section + a separate synthesis template constant, reducing the per-turn
overhead. Progression improved from a baseline win to an engine win; Challenge moved from
a baseline win to a tie. Overall Borda dropped slightly (55.5 vs 56.0) due to Breadth
regression, but the targeted criteria improved.

## What Improved

Criteria where engine gained Borda vs Run 003:

- **Progression: E=5.5 B=3.5** (was E=4.0 B=5.0). Flipped from baseline win to engine win.
  J1 and J2 both scored engine 3 vs baseline 2. The temporal phases now include explicit
  "Revisions to earlier predictions" subsections that confirm, qualify, or revise prior
  claims. Phase 4 explicitly falsifies the Phase 1 vendor-moat thesis. This was the #1
  target criterion and the improvement is decisive.

- **Challenge: E=4.5 B=4.5** (was E=4.0 B=5.0). Moved from baseline win to tie. The
  "Source Thesis Challenges" section (renamed from "What Surprised Us") now includes
  observations that contradict specific assumptions: integration debt vs vendor value
  capture, coordination costs vs labor replacement, exception ownership vs policy syntax.
  Still not scoring 3+ but no longer a loss.

- **Quant Precision: E=5.5 B=3.5** (was E=5.5 B=3.5 in Run 003). Maintained. J2 and J3
  scored baseline 1 (down from previous), widening the gap. Engine's quantitative claims
  (20-30% cycle-time, >=33% policy-gated runners, 3-5 engineering-weeks) remain strong.

## What Regressed or Failed to Improve

- **Breadth: E=3.0 B=6.0** (was E=4.0 B=5.0). Regressed. All 3 judges scored baseline 3
  vs engine 2. The engine's 8 findings span 6 theme categories (technical architecture x2,
  governance/policy x2, organizational/adoption, economics/market, evaluation/testing,
  cross-domain), meeting the diversity mandate. But the baseline covers themes more
  independently — the engine's governance thesis dominates and creates perceived convergence
  even across distinct themes. The diversity mandate is met structurally but not perceptually.

- **Plausibility: E=4.0 B=5.0** (was E=4.5 B=4.5 in Run 003). Regressed from tie to
  baseline win. J1 scored baseline 3 vs engine 2. The engine cites observation IDs
  extensively but the truncated [obs: ...] references may appear as mechanical attribution
  rather than genuine evidence grounding. The baseline's prose-style evidence feels more
  organically grounded.

- **Actionability: E=3.5 B=5.5** (was E=4.0 B=5.0). Marginal regression. J1 and J2
  scored baseline 3 vs engine 2. Despite the actionability mandate naming specific tools
  and effort estimates (e.g., "3-5 engineering-weeks"), the baseline's decision points
  still read as more practically useful because they connect timing triggers to concrete
  organizational milestones.

## Platform Issue: Engine Output Too Large for Judging

The engine synthesis at 44,812 bytes exceeds the 32KB WASM field limit for judge sessions.
The initial 3 engine judge sessions all failed with "user_message is empty" (truncation).
A condensed version was created by trimming Active Directions to titles + counterfactuals
only (29KB), bringing the total prompt under 32KB. This means judges saw slightly less
of the engine output than the full version. The condensed sections (direction body text)
are unlikely to affect Transparency, Breadth, or Specificity scores since the Key Findings,
Temporal Progression, Predictions, and Decision Points were preserved in full.

## Root Cause Analysis

**The synthesis delegation architecture was not exercised.** The orchestrator completed
synthesis within its own context rather than spawning a dedicated synthesis session. This
happened because:

1. The WASM instructions were restructured (split into orchestration + synthesis template),
   reducing per-turn context growth
2. The orchestrator's 13-turn conversation stayed under the ~64KB WASM limit
3. The synthesis template was embedded in the user_message, not loaded from a workspace file

The intended delegation path (write handoff file → spawn synthesis session → poll) was
coded in the instructions but the orchestrator chose to do synthesis directly. The
architectural change still had a positive effect through cleaner instruction structure:
the synthesis template is now a self-contained block that the orchestrator can follow
without accumulating reasoning artifacts from earlier probes.

## Structural Observations

1. **46 observations** (vs 75 in Run 003) — fewer but higher quality. Run 003 had some
   duplicate observations from the crash-recovery context.

2. **12 directions** (same as Run 003) — the convergence phase still produces 2 per probe
   per step (6 probes × 2 steps = 12).

3. **No malformed citations** — all [obs: en-XXXXX] references are valid entity IDs.
   This was a key improvement target from Run 003.

4. **Progression revision quality** — Phase 2-4 each have genuine "Revisions to earlier
   predictions" that confirm, qualify, or falsify earlier claims. Phase 4 even uses the
   word "falsified" for the vendor-moat thesis. This directly addresses the Run 003
   diagnosis criticism of "formulaic revision sections."

## Recommended Changes for Run 005

**Priority 1:** Fix Breadth. The engine's governance-dominant thesis causes perceptual
convergence even across distinct themes. Two approaches:
- (a) Add an explicit instruction: "At least 2 Key Findings must contradict or be in
  tension with the dominant thesis. Do not let governance subsume all themes."
- (b) Require that each temporal phase introduce a DIFFERENT primary theme, preventing
  governance from dominating every phase.

**Priority 2:** Fix Actionability. The current mandate names specific tools but the
decision points still read as strategic. Try: embed concrete operational playbook
examples in the template (e.g., "Step 1: Run `cedar authorize --policy agent-ci.cedar`,
Step 2: Configure timeout=300s on the runner, Step 3: ...").

Per meta-loop rules: make ONE targeted change per iteration. Priority 1 is highest
leverage — Breadth is now a -3.0 Borda gap, the engine's largest deficit.

## Convergence Status

Engine Borda 55.5 vs Baseline 52.5. Engine wins two consecutive runs (003, 004).
A-wins streak remains 0 (engine is challenger-turned-incumbent).
Next run: if baseline (challenger) loses again, streak goes to 1.
Convergence requires 2 consecutive incumbent (engine) wins.
