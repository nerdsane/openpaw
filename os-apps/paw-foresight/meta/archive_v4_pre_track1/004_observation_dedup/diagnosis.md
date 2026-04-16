# Run 004 Diagnosis

## Summary
**Challenger (Run 004): 28.0/48 | Incumbent (Run 002): 30.3/48 | Delta: -2.3**
**Borda: Challenger 50.5/72, Incumbent 57.5/72 | Winner: Incumbent**

The observation deduplication change was never exercised. All 3 projection attempts failed before reaching the convergence step where dedup would execute. The challenger synthesis was assembled from engine-produced observations and directions (13 obs, 3 dirs from the first probe set), same count as the incumbent. The incumbent wins on Breadth (-3 Borda), Progression (-2), Completeness (-1), and Novelty (-1), while tying on 8 criteria. The challenger wins on zero criteria outright.

## Intervention Assessment

**Target criteria from Run 003 diagnosis:**
| Criterion | Target | Result | Verdict |
|-----------|--------|--------|---------|
| Information Density | 1 → 2+ | Tied 2/2 (Borda 4.5/4.5) | **Neutral** — dedup never ran; the 13-obs count from partial execution happened to match incumbent's count |
| Completeness (spillover) | 1.7 → 2+ | Challenger 2.7 vs Incumbent 3.0 (Borda 4.0/5.0) | **Miss** |
| Decision Clarity (spillover) | 1.7 → 2 | Tied 2/2 (Borda 4.5/4.5) | **Neutral** |

## Where the Incumbent Wins

### Breadth (Challenger: 2.0/4, Incumbent: 3.0/4) — Borda: 3.0 vs 6.0 (-3)
All 3 judges unanimously scored the incumbent at 3 and the challenger at 2. The incumbent covers 6+ distinct themes (platform integration, standards convergence, verifiability, trust deficit, adversarial harness risk, safety-liveness asymmetry, coordination bottleneck, labor market recomposition, governance-gated adoption, role evolution) with explicit cross-theme interactions. The challenger covers similar themes but treats them more independently without the systemic interplay.

**Root cause:** The incumbent's 3-probe design (practitioner, security/trust critic, adjacent-domain) with a successful orchestrator synthesis produced integrated cross-theme analysis. The challenger's probes produced similarly diverse observations, but the assembled synthesis (without orchestrator judgment) lacks the cross-theme integration that the orchestrator's synthesis step provides.

### Progression (Challenger: 2.3/4, Incumbent: 3.0/4) — Borda: 3.5 vs 5.5 (-2)
Two judges scored the incumbent at 3, one at 3. The incumbent has a 4-phase temporal progression with explicit "Causal links to next phase," "Revisions to earlier predictions," and "What has NOT changed" sections in each phase. The challenger has a 4-phase structure but with weaker causal chaining and revision language.

**Root cause:** The incumbent's orchestrator completed the synthesis step, which has a template mandating phase-to-phase causal dependencies and revision sections. The challenger's assembled synthesis follows the template structure but the revision and causal-link content is thinner without orchestrator analysis.

### Novelty (Challenger: 2.3/4, Incumbent: 2.7/4) — Borda: 4.0 vs 5.0 (-1)
Two judges favored the incumbent. The incumbent introduces distinctive framings (safety-liveness asymmetry, harness as attack surface, protocol-trust gap) while the challenger introduces different framings (control-tower architecture, certified flow, benchmark ecology) — but judges found the incumbent's novelty more distinctive from the source material.

### Completeness (Challenger: 2.7/4, Incumbent: 3.0/4) — Borda: 4.0 vs 5.0 (-1)
The incumbent has a more complete pipeline with explicit assumptions, confidence levels, and "what would change the confidence" statements. The challenger has assumptions but with less granular confidence assessment.

## Tied Criteria (8 of 12)

| Criterion | Both Score | Borda | Notes |
|-----------|-----------|-------|-------|
| Specificity | 3.0 | 4.5/4.5 | Both name specific vendors, dates, quantitative thresholds |
| Falsifiability | 3.0 | 4.5/4.5 | Both have explicit falsification criteria with dates and conditions |
| Plausibility | 2.0 | 4.5/4.5 | Both reference mechanisms and evidence |
| Actionability | 2.7 | 4.5/4.5 | Both have decision points; neither reaches full decision-framework quality |
| Decision Clarity | 2.0 | 4.5/4.5 | Both structured but "so what" implicit |
| Grounding | 2.0 | 4.5/4.5 | Evidence relevant but logical chains have gaps |
| Challenge | 2.0 | 4.5/4.5 | Both identify tensions but neither makes strong contradictory predictions |
| Information Density | 2.0 | 4.5/4.5 | Both show some redundancy but most claims contribute distinct information |

## Root Cause Analysis

### 1. Orchestrator Session Failure (Primary — same as Run 003)
Three projection attempts all failed before reaching convergence/synthesis:

- **Attempt 1** (en-019d970c): Orchestrator blocked at WaitingForApproval (Cedar policy). After meta-agent approval, orchestrator resumed but spawned duplicate probes. First probe set completed successfully (13 obs, 3 dirs). Orchestrator eventually failed with DeliveryFailed status.
- **Attempt 2** (en-019d971a): Orchestrator timed out at 900s after only 1/3 probes completed.
- **Attempt 3** (en-019d9725): Even with max_steps=1, all sessions timed out (600-900s). The openai_codex provider was consistently slow.

**Impact:** The observation deduplication change in SKILL.md was never exercised. The synthesis was assembled from raw engine artifacts, same as Run 003. Without the orchestrator's synthesis step, the output lacks the integrated cross-theme analysis, causal chaining, and confidence assessment that the incumbent's successful orchestrator produced.

### 2. Observation Deduplication Never Tested
The Run 004 change added a deduplication phase after cross-probe confirmation in Step 4 (Convergence). The orchestrator never reached Step 4. The change remains in SKILL.md and will be tested in a future run where the orchestrator completes.

Ironically, the 13-observation count from the first probe set (partial execution) was the same as the incumbent's 13 observations, so Information Density tied at 2/2 — but this was not due to deduplication working; it was due to only one probe step completing before failure.

### 3. Persistent Provider Reliability Issue
This is the third consecutive run (002, 003, 004) where the openai_codex provider causes session timeouts or failures. Run 002 succeeded only because its orchestrator happened to complete before timeout. The provider is the systemic bottleneck preventing any change from being properly tested.

## Convergence Assessment

This is the third consecutive incumbent win (Run 002 streak 1 → Run 003 streak 2 → Run 004 streak 3). The loop was already converged after Run 003 (streak 2/2). Run 004 overrode the premature convergence to test observation deduplication, but the same infrastructure failure persists.

**The loop is converged.** The current engine (v002, foresight-v002) is the final version under the current infrastructure constraints.

**However, convergence is driven by infrastructure, not by analytical ceiling.** The observation deduplication and adversarial critic changes from Runs 003-004 were never properly tested because the orchestrator never reached the steps where they would execute. The engine's analytical quality has not been meaningfully evaluated since Run 002.

## Recommended Changes (for future runs, if loop resumes)

**Priority 0: Fix provider reliability.** The openai_codex provider is the blocking constraint. Options:
- Switch to a different provider (anthropic_codex, google_codex) for the orchestrator session
- Implement retry logic in the spawn_orchestrator WASM for transient API failures
- Reduce orchestrator scope (fewer turns, simpler prompts) to complete within timeout

**Priority 1: Re-test observation deduplication.** The SKILL.md change is already in place. Once the orchestrator reliably completes, this change can be evaluated.

**Priority 2: Re-test adversarial critic.** The Run 003 change showed strong Challenge improvement (2→3 unanimous) but was reverted by protocol. If the infrastructure is fixed, this change should be retried.

**Priority 3: WASM-enforced synthesis sections.** The structured sections (falsification criteria, decision points, temporal progression with causal links, assumptions with confidence levels) are critical for Breadth, Progression, Completeness, and Falsifiability. Making them WASM-enforced rather than prompt-instructed would ensure they survive orchestrator partial failures.
