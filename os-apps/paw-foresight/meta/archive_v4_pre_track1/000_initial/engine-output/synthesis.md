# Foresight Projection: Directed Software Evolution v2
## Horizon: 1 year | Steps: 1 of 2 completed (engine failed before step 2)
## Date: 2026-04-16

**NOTE: The orchestrator session failed with "fuel exhausted -- module exceeded instruction budget" after completing step 0 (0-90 day projection). No convergence analysis, no step 1 (91-365 days), and no final synthesis were produced. This document compiles the raw engine output from step 0 only.**

### Executive Summary

The engine spawned 3 independent probes (practitioner, critic, adjacent-domain) to analyze Directed Software Evolution over a 90-day window. All 3 probes completed successfully, producing 12 observations and 3 directions. However, the orchestrator crashed before performing convergence analysis, advancing to step 1, or writing a final synthesis. The output below represents unprocessed probe output without cross-validation or temporal development.

### Key Findings (from 12 observations, step 0 only)

- Within 90 days, pilots marketed as autonomous software engineering by Cognition's Devin, Cursor, OpenAI Codex-style agents, and OpenHands will hit a governance wall in larger enterprises: if fewer than 70% of agent-generated pull requests pass mandatory human review on the first pass, security and platform teams will cap usage to low-risk repos.
- The dominant narrative that Anthropic, OpenAI, Temper, and Cedar-like policy layers make autonomous coding safe will be challenged by eval blind spots: benchmark wins will not translate into operational trust when agents span issue triage, code editing, CI repair, and deployment approval.
- Market demand over the next 90 days is likely to favor copilot-style tools such as Cursor, Aider, and Cline over fully delegated systems, because the ROI is easier to prove at the seat level than at the workflow level.
- Temper-style governed agent platforms that rely on explicit state machines, Cedar or OPA authorization, and auditable actions will attract interest, but 90-day adoption will be slowed by integration debt rather than model quality.
- Biology analogy: Directed software evolution is moving from artisanal coding toward an immune-system style control loop, where Cedar or OPA policies act like innate immunity and verification harnesses act like adaptive screening.
- Economics analogy: this domain is converging on portfolio selection, not single-agent supremacy. Sophisticated buyers will allocate work by risk-adjusted expected value: cheap tools for broad search, expensive tools for narrow synthesis, and Temper-like orchestrators for governed execution.
- Industrial process control analogy: the dominant narrative says better coding agents automatically produce autonomous software factories, but the nearer-term bottleneck is instrumentation density.
- Organizational theory analogy: the first successful adopters will behave less like software teams and more like supply-chain operators, with explicit work-in-process limits, quarantine lanes, and supplier qualification for agents.
- Within 90 days, the most deployable pattern for directed software evolution will be harness-first control-plane generation rather than full app autonomy.
- The verification cascade will mature faster than end-to-end autonomy: practitioners will combine repo-specific evals, property tests, fuzzing, type checks, and containerized integration replay.
- A near-term breakthrough area is WASM-mediated action execution, not unrestricted shell access.
- Challenge to the dominant narrative: Anthropic, OpenAI, Cursor, and Cognition or Devin will not make fully autonomous software factories common in the next 90 days; instead, the winning teams will narrow scope and reduce ambition.

### Active Directions

#### Direction 1: Attention will outpace operational trust
Over the next 90 days, directed software evolution will gain attention faster than it earns operational trust because approval economics and workflow brittleness will dominate model capability gains.

The near-term failure mode for directed software evolution is not that Anthropic, OpenAI, Cursor, Devin, or OpenHands suddenly stop producing useful code; it is that enterprises discover the cost of supervising action chains is still higher than the value of delegating them. When agent output moves from draft code into Kubernetes changes, Terraform plans, or CI remediation, the real unit of work becomes approval and rollback, not generation. Cedar and OPA can restrict who is allowed to act, but they do not by themselves prove that the chosen action sequence is safe, cost-effective, or reviewable under production conditions. In that environment, every escalation, exception, and revert becomes evidence that workflow reliability is lagging behind model capability.

That points to a likely market split. Tools like Cursor, Aider, and Cline will keep expanding because they preserve developer agency while compressing drafting time; their governance surface is smaller and their ROI is easier to explain. By contrast, platforms like Temper that try to make autonomous or semi-autonomous execution auditable will be judged on integration burden.

**Counterfactual:** If enterprises unexpectedly accept higher review overhead and tolerate imperfect auditability, full-stack autonomous coding platforms could expand faster than this thesis predicts.

#### Direction 2: Policy-governed selection systems over single-model adoption
The near-term winners in directed software evolution will be teams that build policy-governed selection systems around multiple coding agents, not teams that merely adopt the strongest single model.

Directed software evolution will advance in the next 90 days where organizations treat AI coding as a governed evolutionary process rather than as a better autocomplete market. The adjacent-domain pattern is clear: biology, portfolio economics, and industrial control all reward systems that separate variation from selection. Anthropic and OpenAI models, along with Cursor, Aider, Cline, OpenHands, and Cognition Devin, will keep improving as generators, but generators are only one half of an evolutionary machine. The operational differentiator will be infrastructure that allocates tasks across agents, constrains authority with Cedar or OPA, and evaluates changes against stable harnesses in Kubernetes and Terraform environments.

The economics of adoption will therefore look like factory modernization, not like a consumer software feature race. Firms that instrument their systems well enough to measure rollback rates, policy violations, accepted-change yield, and cost per verified change will discover that multi-agent portfolio routing beats single-model dependence.

**Counterfactual:** If the thesis is wrong, the next 90 days will show single-model workflows from one vendor outperforming governed multi-agent systems on accepted-change yield, incident rate, and cost without requiring stronger policy or instrumentation layers.

#### Direction 3: Narrow harnessed control-plane actions over broad autonomous coding
In the next 90 days, practitioners will make directed software evolution real by narrowing agent scope into harnessed control-plane actions rather than pursuing broad autonomous coding.

Over the next 90 days, directed software evolution will advance most in teams that treat Anthropic and OpenAI models as proposal generators inside a governed control plane, not as autonomous engineers. The operational pattern is already visible: use Temper to represent work as entity state transitions, route side effects through WASM integrations, evaluate authorization with Cedar and OPA, and constrain infrastructure changes through Terraform plan review plus Kubernetes admission control.

The practical winner will be the organization that reduces free-form surface area. Instead of asking a model to run a whole repo or cluster, practitioners will define bounded actions.

**Counterfactual:** If this thesis is wrong, broad autonomous coding products from Anthropic, OpenAI, Cursor, or Cognition will prove reliable across large unstructured repos without heavy harness engineering.

### What Surprised Us

No convergence analysis was performed (orchestrator crashed). The individual probe observations contain challenges to the dominant narrative:
- The assumption that better models automatically produce viable directed evolution is questioned — the constraint is trustworthy selection, not idea generation.
- Human approvals may remain durable features, not transitional bugs.
- The instrumentation bottleneck may be the true constraint, not model capability.

### Decision Points

No explicit decision points were synthesized (orchestrator crashed before synthesis phase). The observations imply:
- When agent-generated PR approval rates drop below 70%, restrict to low-risk repos
- When onboarding a governed workflow takes >2 weeks, the system needs simplification
- When rollback frequency exceeds 5% of agent-touched changes, pause and invest in harnesses

### Methodology
- 3 independent probes per step (practitioner, critic, adjacent-domain)
- 1 of 2 planned time steps completed (0-90 days only; 91-365 days never reached)
- 12 total observations, 3 total directions
- No convergence analysis (orchestrator failed)
- No temporal progression beyond step 0
- Engine failure: WASM fuel exhaustion in orchestrator session
