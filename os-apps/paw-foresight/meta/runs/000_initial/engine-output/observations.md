# Engine Observations (Run 000)

Total: 12 observations from step 0 only

## Observation 1
**Probe:** aj-019d9680-a18b-7af1-8afc-4e189e3c3d9d
**Step:** 0
**Importance:** high
**Status:** Created

Within 90 days, pilots marketed as autonomous software engineering by Cognition's Devin, Cursor, OpenAI Codex-style agents, and OpenHands will hit a governance wall in larger enterprises: if fewer than 70% of agent-generated pull requests pass mandatory human review on the first pass, security and platform teams will cap usage to low-risk repos. The fragile assumption is that code generation quality is the binding constraint; in practice the bottleneck becomes approval latency, provenance, and rollback confidence. Teams running Kubernetes and Terraform in regulated environments will find that one bad infrastructure diff can consume an entire week's review capacity, so adoption signals to watch are not demo velocity but median PR approval time, rollback frequency above 5% of agent-touched changes, and whether platform teams restrict agents from production IaC repositories.

**Counterfactual:** If operators ignore approval friction and keep scaling agent-written changes anyway, enterprises will experience a visible spike in stalled PR queues and emergency reverts, which will trigger blanket policy restrictions rather than broader deployment.

---

## Observation 2
**Probe:** aj-019d9680-a18b-7af1-8afc-4e189e3c3d9d
**Step:** 0
**Importance:** high
**Status:** Created

The dominant narrative that Anthropic, OpenAI, Temper, and Cedar-like policy layers make autonomous coding safe will be challenged by eval blind spots: benchmark wins will not translate into operational trust when agents span issue triage, code editing, CI repair, and deployment approval. Over the next 90 days, expect more teams to discover that passing repo-level unit tests is an inadequate proxy for safe action sequencing. A concrete failure mode is policy-compliant but operationally harmful behavior: an agent can satisfy OPA or Cedar rules while still opening too many low-quality changes, saturating reviewers, or mutating Terraform modules that pass syntax checks but violate cost or blast-radius expectations. Watch for signals such as organizations adding manual freeze windows, requiring per-action allowlists, or reporting that more than 20% of agent tasks need replay because the original audit trail was insufficient for incident review.

**Counterfactual:** If buyers treat eval scores and policy attachment as proof of readiness, they will overestimate robustness and underinvest in workflow-specific audits, creating incidents that damage confidence in the whole category.

---

## Observation 3
**Probe:** aj-019d9680-a18b-7af1-8afc-4e189e3c3d9d
**Step:** 0
**Importance:** medium
**Status:** Created

Market demand over the next 90 days is likely to favor copilot-style tools such as Cursor, Aider, and Cline over fully delegated systems, because the ROI is easier to prove at the seat level than at the workflow level. The weak assumption in the autonomous-agent thesis is that engineering organizations want less human involvement; many actually want tighter human-in-the-loop acceleration with better local control. If enterprise renewals show that assisted editing seats grow faster than autonomous task execution by a ratio of 3:1, or if teams keep agents confined to documentation, test generation, and backlog grooming, that will indicate market mismatch. OpenHands and Devin may still win mindshare, but procurement will ask for evidence of lower incident rates, shorter MTTR, and review savings before approving broader rollout.

**Counterfactual:** If vendors misread curiosity and trial usage as durable demand for autonomy, they will optimize for flashy end-to-end demos while the revenue pool consolidates around cheaper, review-friendly copilots.

---

## Observation 4
**Probe:** aj-019d9680-a18b-7af1-8afc-4e189e3c3d9d
**Step:** 0
**Importance:** medium
**Status:** Created

Temper-style governed agent platforms that rely on explicit state machines, Cedar or OPA authorization, and auditable actions will attract interest, but 90-day adoption will be slowed by integration debt rather than model quality. The hard part is not invoking Anthropic or OpenAI models; it is mapping real approval chains, exception handling, and change windows into enforceable automata without creating an operator tax. In organizations already running Kubernetes, Terraform, GitHub Actions, and ticketing workflows, a useful threshold is time-to-onboard: if encoding one production workflow takes more than 2 weeks or needs more than 10 custom policy exceptions, platform teams will classify the system as governance-heavy and keep it in sandbox mode. This challenges the belief that better orchestration alone unlocks rapid enterprise deployment.

**Counterfactual:** If teams dismiss workflow modeling cost and push governed agents into production prematurely, they risk brittle policies, emergency bypasses, and a loss of trust in both the platform and the governance layer.

---

## Observation 5
**Probe:** aj-019d9680-a199-7b60-8d30-b910790eed29
**Step:** 0
**Importance:** high
**Status:** Created

Biology analogy: Directed software evolution is moving from artisanal coding toward an immune-system style control loop, where Cedar or OPA policies act like innate immunity and verification harnesses act like adaptive screening. Over the next 90 days, expect at least 3 visible platform teams to publish reference architectures that pair AI coding agents such as Cursor, OpenAI Codex, or Cognition Devin with policy gates in Kubernetes and Terraform delivery paths. The measurable signal is not model quality alone; it is whether AI-authored changes can be auto-rejected for policy violations before merge or deploy. If 20% or more of AI-generated infrastructure pull requests in a pilot can be blocked automatically by Cedar/OPA plus test harnesses, organizations will start treating governance as the selection environment rather than as after-the-fact review.

**Counterfactual:** If teams ignore the immune-system layer and optimize only for generation speed, they will get infection without memory: more code churn, more rollback events, and rising distrust in autonomous agents, causing pilots with OpenAI, Anthropic, and Cursor to stall before production.

---

## Observation 6
**Probe:** aj-019d9680-a199-7b60-8d30-b910790eed29
**Step:** 0
**Importance:** high
**Status:** Created

Economics analogy: this domain is converging on portfolio selection, not single-agent supremacy. In the next 90 days, sophisticated buyers will stop asking whether Anthropic, OpenAI, Cursor, Devin, Aider, Cline, or OpenHands is best in the abstract and instead allocate work by risk-adjusted expected value: cheap tools for broad search, expensive tools for narrow synthesis, and Temper-like orchestrators for governed execution. A strong adoption signal would be at least 2-3 public case studies where one model family handles exploration, another handles code transformation, and a third only approves or critiques high-impact changes. The key threshold is budget discipline: if blended multi-agent routing reduces cost per accepted change by 15% or more relative to a single premium model workflow, portfolio orchestration becomes the default operating logic.

**Counterfactual:** If organizations keep buying a single frontier model as if software evolution were winner-take-all, they will overpay for tasks that need diversification and underinvest in routing, selection, and auditability—the actual sources of durable advantage.

---

## Observation 7
**Probe:** aj-019d9680-a199-7b60-8d30-b910790eed29
**Step:** 0
**Importance:** medium
**Status:** Created

Industrial process control analogy: the dominant narrative says better coding agents automatically produce autonomous software factories, but the nearer-term bottleneck is instrumentation density. Kubernetes, Terraform, and Temper resemble machine tools only when the feedback loops are calibrated like a modern factory line with gauges, tolerances, and scrap accounting. Within 90 days, expect several enterprise pilots to discover that fewer than 60% test-surface coverage on change-critical paths or rollback detection slower than 15 minutes makes autonomous iteration economically irrational, even with Anthropic or OpenAI class models. This challenges the prevailing story that more capable models alone unlock dark-factory operation; in practice, control theory says unstable plants cannot be fixed by smarter operators if the sensors are poor.

**Counterfactual:** If this instrumentation bottleneck is ignored, teams will misdiagnose failures as model weakness, keep switching between Devin, Cursor, and Codex, and miss that their true constraint is insufficient observability and closed-loop control.

---

## Observation 8
**Probe:** aj-019d9680-a199-7b60-8d30-b910790eed29
**Step:** 0
**Importance:** medium
**Status:** Created

Organizational theory analogy: the first successful adopters will behave less like software teams and more like supply-chain operators, with explicit work-in-process limits, quarantine lanes, and supplier qualification for agents. Over the next 90 days, expect at least one visible pattern among Temper-like systems: AI agents will be tiered into classes such as explorer, proposer, verifier, and deployer, with different authorities enforced by Cedar-style policy boundaries. A measurable signal is whether organizations cap autonomous changes per service or per day and require promotion between trust tiers after a run of, for example, 30-50 successful verified changes. Companies using Cursor or OpenHands only as copilots will lag organizations that treat agent outputs as inbound components requiring acceptance sampling.

**Counterfactual:** If firms fail to introduce supply-chain style qualification and throughput controls, agent usage will remain a local productivity hack rather than an operating system for directed software evolution, limiting scale and cross-team trust.

---

## Observation 9
**Probe:** aj-019d9680-a17e-78d3-a591-88a3b491a746
**Step:** 0
**Importance:** high
**Status:** Created

Within 90 days, the most deployable pattern for directed software evolution will be harness-first control-plane generation rather than full app autonomy: teams using Temper with Kubernetes, Terraform, Cedar, and OPA will ship agent-generated changes only for resources that already have replayable plans, policy checks, and rollback hooks. A practical adoption signal will be at least 3 internal platform teams replacing manual review on 30% or more of low-risk infrastructure pull requests when the generated change passes terraform plan diff checks, OPA or Cedar authorization evaluation, and Kubernetes admission policies. Anthropic and OpenAI models will be used as proposal engines, but the real threshold for production use will be whether failed runs can be reproduced deterministically from prompts, inputs, and action logs inside Temper.

**Counterfactual:** If practitioners ignore the need for deterministic harnesses and audit trails, Anthropic or OpenAI generated infrastructure changes will remain demo-grade, and organizations will cap agents at suggestion-only workflows instead of allowing direct apply in Kubernetes or Terraform pipelines.

---

## Observation 10
**Probe:** aj-019d9680-a17e-78d3-a591-88a3b491a746
**Step:** 0
**Importance:** high
**Status:** Created

The verification cascade will mature faster than end-to-end autonomy: over the next 90 days, practitioners will combine repo-specific evals, property tests, fuzzing, type checks, and containerized integration replay so that Aider, Cursor, Cline, and OpenHands can be ranked by harness pass rate instead of anecdotal vibe. A concrete deployment marker is teams setting a promotion gate such as 95% plus unit and integration pass rate, zero new critical OPA or Cedar policy violations, and less than 5% rollback frequency across the first 50 agent-authored changes. This shifts selection pressure away from whichever model is best at code completion toward whichever stack produces the highest pass-to-merge ratio under fixed harness conditions.

**Counterfactual:** If teams do not instrument a verification cascade, they will keep arguing tool preference by subjective developer experience, and directed evolution loops will overfit to flashy demos from Cursor or OpenHands rather than reliable software change acceptance.

---

## Observation 11
**Probe:** aj-019d9680-a17e-78d3-a591-88a3b491a746
**Step:** 0
**Importance:** medium
**Status:** Created

A near-term breakthrough area is WASM-mediated action execution, not unrestricted shell access: Temper-style action dispatch with WASM integrations will be adopted for narrow workflows such as ticket triage, policy evaluation, Terraform plan summarization, and Kubernetes remediation because operators can expose bounded actions with clear state transitions. Expect at least 2 platform teams to move from raw agent terminal sessions to action-based interfaces where more than 70% of production-touching agent work is routed through governed actions, and every external call is attached to an entity transition. This is technically feasible now because WASM isolation, Cedar authorization, and OPA policy bundles fit existing control-plane patterns better than free-form autonomous coding sessions.

**Counterfactual:** If builders keep centering unrestricted shells and long-lived terminal agents, they will hit security review walls, and production operations will be pushed back to humans even when the underlying automation could have been safely wrapped as governed WASM actions.

---

## Observation 12
**Probe:** aj-019d9680-a17e-78d3-a591-88a3b491a746
**Step:** 0
**Importance:** high
**Status:** Created

Challenge to the dominant narrative: Anthropic, OpenAI, Cursor, and Cognition or Devin will not make fully autonomous software factories common in the next 90 days; instead, the winning teams will narrow scope and reduce ambition. Practitioners will discover that the bottleneck is not model intelligence but change-surface design: unless a repo exposes machine-checkable intent, stable fixtures, and resource-scoped permissions, more agent horsepower only increases variance. An adoption signal will be that teams reporting success with Aider, Cline, or Cursor are mostly automating repetitive control-plane and maintenance tasks under 500-line diffs, while large cross-service refactors still require human decomposition and staged rollout. In practice, dark factory claims will retreat to subsystems with tight harnesses rather than whole engineering organizations.

**Counterfactual:** If the field keeps assuming broader model capability alone will unlock autonomy, organizations will overinvest in general agents, underinvest in harness design, and produce visible failures that slow adoption of genuinely workable directed evolution patterns.

---

