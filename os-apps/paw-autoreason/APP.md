# paw-autoreason

Tournament-based artifact refinement with convergence detection. Implements the NousResearch/autoreason protocol with cross-model judge pools, Borda count scoring, randomized label evaluation, and principled stopping.

## Entity Types

- **Tournament** -- Orchestrates iterative refinement. Lifecycle: `Created -> InRound -> Judging -> Tallying -> Converged | MaxRounds | Failed`.
- **Round** -- One critique-revise-judge iteration. Lifecycle: `Active -> Complete`.
- **Version** -- A specific artifact version (A=incumbent, B=revised, AB=synthesized). Lifecycle: `Created -> Ready`.
- **Judgment** -- One judge's evaluation under randomized labels. Lifecycle: `Created -> Submitted`.

## WASM Modules

- **initialize_tournament** -- On Tournament.Begin: creates Version A from artifact, creates first Round. Deterministic setup.
- **run_round** -- On Tournament.NextRound: spawns one referee session that orchestrates the full round via sub-agents. One WASM, one smart agent.
- **tally_votes** -- On Tournament.AllJudged: pure math. Decodes randomized labels, computes Borda count (1st=2pts, 2nd=1, 3rd=0), conservative tiebreak (incumbent wins ties), checks convergence (k consecutive A wins). No LLM.

## Agents (5 Isolated -- context firewalls are the protocol)

| Agent | Skill | Sees | Does NOT See |
|-------|-------|------|-------------|
| referee | referee | Everything needed to orchestrate | N/A (orchestrator) |
| critic | critique | Version A | Previous rounds, fixes |
| author | revise | Version A + critique | Previous rounds, judge output |
| synthesizer | synthesize | Version A + Version B | Critique |
| judge | judge | 3 versions (randomized labels) | Real labels, other judges |

## Key Autoreason Properties

- "Do nothing" is first-class -- Version A always competes unchanged
- Synthesis (AB) wins ~50% of rounds -- primary improvement driver
- Convergence detection: k consecutive incumbent wins = principled "when to stop"
- Fresh agent isolation prevents sycophantic cascading
- Cross-model judge pool supports different providers per judge

## Dependencies

- `paw-agent` -- Session entity for agent spawning
- `paw-fs` -- File storage for artifacts, versions, critiques
