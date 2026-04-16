# Foresight Projection: Directed Software Evolution v2
## Horizon: 1 year | Steps: 2

### Executive Summary

Directed software evolution moves from demo-heavy enthusiasm to infrastructure-heavy consolidation over the next 12 months. Anthropic, OpenAI, Cursor, GitHub Copilot, Claude Code, Kubernetes, GitHub Actions, and Buildkite all matter, but the center of gravity shifts from model novelty to controlled execution, replay, and selection pipelines [obs: en-019d9565-2236-7340-bf30-81e7ba17e99e] [obs: en-019d9565-1dbb-75e2-b46e-b68a335fb96f] [obs: en-019d9565-5bfb-72f2-89f3-5dd9f803adf4]. The dominant trajectory is not fully autonomous coding agents replacing engineers; it is CI-native harnesses, branch-per-variant workflows, repo-specific regression gates, and production-like environments becoming the minimum substrate for safe evolutionary search [obs: en-019d9565-83d4-7d40-906d-8d97fcc95c68] [obs: en-019d9565-7650-7812-859b-ccac41bc1205].

The main counterargument is economic and organizational, not model-centric. Several probes converge that CFOs, platform leaders, security teams, and middle managers will challenge broad rollout unless vendors and internal platform teams can show hard labor offsets, lower rollback rates, or measurable cycle-time gains; otherwise pilots stall after the first wave of excitement [obs: en-019d9565-1fd8-7343-abe2-7d4429a5ea9d] [obs: en-019d9565-5d83-7b80-988f-63f4e1731563] [obs: en-019d9565-5930-7530-b4ec-d6a35403486e] [obs: en-019d9565-7f6d-7911-bf68-815c6c2c9b5e]. The biggest surprise is that adjacent-domain probes describe the field less as software craftsmanship and more as biology, manufacturing, and portfolio management: success depends on selection pressure, process control, and risk budgeting across many code variants, not on a single brilliant agent run [obs: en-019d9565-27e5-76d2-9cd8-4b8f30c2e4c4] [obs: en-019d9565-44ca-7333-95c3-39de4857263f] [obs: en-019d9565-55ed-7ed2-b492-3c9405f18d0f].

For decision-makers, this means buying or building for governance, evaluation, and platform integration before betting on generalized autonomy. A reasonable planning baseline is that only 20-30% of current coding-agent pilots convert into standardized production workflows within 12 months unless they clear explicit ROI and control thresholds, while teams that do invest 2-4 engineering-weeks in harnessing, policy gates, and environment fidelity can move from isolated experimentation to repeatable deployment [obs: en-019d9565-2140-7481-9525-2acebb8dc7c4] [obs: en-019d9565-98ac-7f42-9209-faf03adc7cc8] [obs: en-019d9565-42b5-7e03-8289-3d9bd183e683].

### Key Findings

1. **Cursor, Claude Code, and GitHub Actions are pushing coding agents toward CI-native control planes rather than persistent chat copilots.**
   - Evidence: "By the 180-365 day window, the winning coding-agent stacks will look more like CI-native scaffolds than chat products" [obs: en-019d9565-1dbb-75e2-b46e-b68a335fb96f]
   - Measurable indicator: By Q2 2027, more than 50% of successful enterprise rollouts expose agent runs through CI jobs, branch artifacts, or replay logs rather than IDE-only sessions.
   - Theme: technical architecture

2. **Anthropic, OpenAI, and GitHub Copilot face a CFO-led reset as enterprise buyers demand labor-offset evidence before widening seat deployment.**
   - Evidence: "Across the next 180-365 days, enterprise coding-agent adoption will concentrate in organizations that can tie spend to measurable labor offsets" [obs: en-019d9565-1fd8-7343-abe2-7d4429a5ea9d]
   - Measurable indicator: Pilot-to-standard conversion falls below 35% where teams cannot document at least a 10-15% backlog-throughput gain by budget review.
   - Theme: economics/market

3. **Open evaluation stacks such as SWE-CI and repo-specific harnesses will become as foundational as pytest or Playwright for agent deployment.**
   - Evidence: "Open evaluation harnesses for software agents will become part of the build stack, similar to how pytest or Playwright became default infrastructure" [obs: en-019d9565-3c5d-7de0-9b2b-9af8eb2cb604]
   - Measurable indicator: By the end of the horizon, top-tier teams maintain at least 1 internal benchmark suite replaying historical tickets weekly before promotion.
   - Theme: evaluation/testing

4. **Devin, Cursor, and OpenAI Codex workflows will look more like high-throughput biology labs, with variant generation only mattering when selection pipelines are instrumented.**
   - Evidence: "In the next 180 days, Directed Software Evolution will start to resemble high-throughput biology labs more than conventional CI" [obs: en-019d9565-27e5-76d2-9cd8-4b8f30c2e4c4]
   - Measurable indicator: Leading teams run 5-20 candidate diffs per task family with explicit retention criteria on pass rate, regression risk, and review burden.
   - Theme: cross-domain

5. **Platform engineering, security, and QA leaders—not just model vendors—will determine whether coding-agent adoption escapes the enthusiast phase.**
   - Evidence: "Inside enterprises, the main brake on directed software evolution will be organizational redesign rather than model quality alone" [obs: en-019d9565-5930-7530-b4ec-d6a35403486e]
   - Measurable indicator: Firms that assign explicit governance ownership see rollout continue past 3 teams; firms that do not stall at 1-2 experimental teams.
   - Theme: organizational/adoption

6. **Manufacturing-style process control will outrank raw frontier-model novelty in enterprise buying decisions for Anthropic- and OpenAI-based agent stacks.**
   - Evidence: "Manufacturing-style process control will start to outrank raw model novelty in enterprise buying decisions for directed software evolution" [obs: en-019d9565-98ac-7f42-9209-faf03adc7cc8]
   - Measurable indicator: Shortlists increasingly require rollback-frequency, defect-escape, and auditability metrics, with at least 3 operational controls reviewed in procurement.
   - Theme: governance/policy

### Temporal Progression

#### Phase 1: 0-3 Months (days 0-90)
- Cursor, Claude Code, GitHub Copilot, and OpenAI Codex remain the visible front-end brands, but serious teams begin wrapping them in GitHub Actions-based or CI-native evaluation loops rather than relying on chat demos alone [obs: en-019d9565-2236-7340-bf30-81e7ba17e99e] [obs: en-019d9565-3c5d-7de0-9b2b-9af8eb2cb604].
- Expected signals: more branch-per-variant execution, ephemeral environments, and replayable regression fixtures appear in build pipelines; internal platform teams start treating agent runs as artifacts rather than conversations [obs: en-019d9565-5bfb-72f2-89f3-5dd9f803adf4] [obs: en-019d9565-83d4-7d40-906d-8d97fcc95c68].
- What has NOT changed that was expected to: broad enterprise seat expansion does not arrive immediately because review overhead, duplicated tooling, and uncertain ROI hit finance discussions faster than productivity proof does [obs: en-019d9565-5d83-7b80-988f-63f4e1731563] [obs: en-019d9565-2140-7481-9525-2acebb8dc7c4].
- Causal link to Phase 2: once teams capture run artifacts and internal ticket replays, the bottleneck shifts from access to models toward evaluation quality and environment fidelity [obs: en-019d9565-42b5-7e03-8289-3d9bd183e683].

#### Phase 2: 3-6 Months (days 90-180)
- Buildkite, Kubernetes, and repo-specific harness stacks become more central because evolutionary search requires repeatable execution contexts and production-like devcontainers, not just better prompting [obs: en-019d9565-1dbb-75e2-b46e-b68a335fb96f] [obs: en-019d9565-7650-7812-859b-ccac41bc1205].
- Expected signals: sampled historical-ticket replays, staged filters, and pass-rate dashboards become normal in mature teams; weaker agent variants are eliminated earlier to control inference cost [obs: en-019d9565-42b5-7e03-8289-3d9bd183e683] [obs: en-019d9565-5c1d-76a1-9148-ca12030c891e].
- **Revisions to earlier predictions:** The early expectation that chat-layer UX would remain the main differentiator gets revised because reproducibility pressure forces more value into orchestration, artifact capture, and environment management. Likewise, the hope that generic benchmarks would be enough is qualified: benchmarks still matter for model screening, but rollout gates move to internal harnesses because public scores miss local architecture and migration constraints [obs: en-019d9565-2236-7340-bf30-81e7ba17e99e] [obs: en-019d9565-42b5-7e03-8289-3d9bd183e683].
- Causal link to Phase 3: once infrastructure is in place, executive scrutiny turns toward cost concentration, governance, and which teams can actually absorb the organizational redesign [obs: en-019d9565-1fd8-7343-abe2-7d4429a5ea9d] [obs: en-019d9565-5930-7530-b4ec-d6a35403486e].

#### Phase 3: 6-9 Months (days 180-270)
- HashiCorp Terraform, OPA, and Cedar-style control patterns begin showing up around coding-agent programs as procurement and security teams demand auditable policy gates, rollback thresholds, and clearer approval paths [obs: en-019d9565-98ac-7f42-9209-faf03adc7cc8] [obs: en-019d9565-44ca-7333-95c3-39de4857263f].
- Expected signals: buyers ask for defect-escape rate, rollback frequency, and change-approval traceability; smaller startups face margin pressure as model cost and customer-acquisition cost squeeze undifferentiated offerings [obs: en-019d9565-39f5-7e03-9b12-67f27184ecd7] [obs: en-019d9565-98ac-7f42-9209-faf03adc7cc8].
- **Revisions to earlier predictions:** The thesis of smooth vendor expansion is revised downward because enterprise demand concentrates around a few distribution-advantaged vendors, while local champions inside firms discover that labor politics and accountability redesign are harder than standing up the tooling. Predictions of immediate workforce compression are also qualified: the nearer-term effect is budget authority moving toward smaller, instrumentation-heavy teams, not wholesale elimination of software functions [obs: en-019d9565-39f5-7e03-9b12-67f27184ecd7] [obs: en-019d9565-7a81-79c0-89f1-5a1d81469526] [obs: en-019d9565-7f6d-7911-bf68-815c6c2c9b5e].
- Causal link to Phase 4: as market winners narrow and internal controls harden, firms start managing code variants like portfolios that need allocation rules, stress scenarios, and retirement logic [obs: en-019d9565-55ed-7ed2-b492-3c9405f18d0f] [obs: en-019d9565-69e6-7a60-86fb-efb362b189a0].

#### Phase 4: 9-12 Months (days 270-365)
- Temper-like orchestration layers, internal risk dashboards, and portfolio-style selection logic become differentiators as firms treat code variants as managed populations rather than isolated outputs [obs: en-019d9565-55ed-7ed2-b492-3c9405f18d0f] [obs: en-019d9565-73a8-7f51-9875-f88a5f3a9115].
- Expected signals: teams formalize exploration budgets, loss limits, and niche role separation between research-style agent builders, platform operators, and governance owners [obs: en-019d9565-c2e3-7792-8427-ceb018ccb611] [obs: en-019d9565-83cf-7022-8e74-6057ccec67ba].
- **Revisions to earlier predictions:** Earlier predictions of one dominant agent workflow are revised because the field fragments into stable niches: some teams optimize for ticket replay and evals, some for process control, and some for portfolio-style search. Earlier confidence that model quality alone would unlock scale is effectively falsified; what persists is the combination of scaffold, environment fidelity, and organizational legitimacy [obs: en-019d9565-7650-7812-859b-ccac41bc1205] [obs: en-019d9565-c2e3-7792-8427-ceb018ccb611].
- **Final state assessment:** At day 365, the field is bigger but more disciplined. The winners are not the loudest autonomy brands; they are the vendors and internal platforms that combine Anthropic/OpenAI-class models with CI integration, repo-specific evaluation, process controls, and credible ROI narratives [obs: en-019d9565-1fd8-7343-abe2-7d4429a5ea9d] [obs: en-019d9565-98ac-7f42-9209-faf03adc7cc8].

### Active Directions

#### Technical-architecture direction: CI-native agent control planes become the standard substrate for directed software evolution
**Direction ID:** en-019d9565-acd8-7f92-b3ba-a6044865d12c
**Theme:** technical architecture

Technical architecture direction: coding-agent platforms will consolidate around scaffold-first control planes that separate the model from the execution harness. The strongest signal is that both research taxonomies and practitioner writeups are converging on the same insight: reliability comes less from prompt cleverness and more from the surrounding scaffold that controls tool invocation, state, retries, patch application, and test execution. That means teams deploying Directed Software Evolution will invest in runners, trace stores, artifact capture, and workflow orchestration on top of GitHub Actions, Buildkite, or Kubernetes job systems, while treating the underlying model as a swappable component.

In the 180-365 day window, this architecture becomes necessary because evolutionary search requires many repeatable trials. Once a team is generating multiple candidate diffs per issue, every attempt needs the same repo snapshot, dependency graph, sandbox policy, and rollback path or the comparison is meaningless. The practical winners will expose standard interfaces for task packaging, branch creation, patch scoring, and replay, so the same harness can run Claude Code, Cursor-style agents, or internal models without rewriting the pipeline. If that abstraction hardens, directed evolution stops being an experiment inside IDEs and becomes a production subsystem in CI.

Supporting observations: [obs: en-019d9565-1dbb-75e2-b46e-b68a335fb96f], [obs: en-019d9565-7650-7812-859b-ccac41bc1205]

**Counterfactual:** If coding-agent systems do not consolidate around CI-native control planes, most teams will remain stuck in ad hoc IDE workflows and directed software evolution will stay too irreproducible to scale operationally.

#### Economics/Market: Enterprise coding-agent spending shifts from experimentation to ROI-gated consolidation
**Direction ID:** en-019d9565-a942-7391-8d21-b1d9c748d630
**Theme:** economics/market

Economically, directed software evolution is heading into a procurement reset rather than a straight-line expansion curve. Current market signals point to enthusiasm around coding agents, but that enthusiasm is colliding with enterprise budget discipline, rising inference spend, and a growing expectation that tools like GitHub Copilot, Cursor, and Claude-based agent workflows must prove measurable throughput gains before they earn scaled deployment. Over the next 180-365 days, CIOs and finance leaders will increasingly evaluate these products the way they evaluate other automation investments: against backlog reduction, incident-rate stability, and developer time saved, not demo quality or developer excitement alone. That will slow conversion from pilot to enterprise standard and expose how many vendor growth narratives depend on subsidized experimentation rather than durable willingness to pay.

This matters because directed software evolution is more expensive organizationally than autocomplete-era tooling. It requires evaluation infrastructure, review capacity, and workflow changes that many firms still treat as overhead. If a company cannot show that agent-generated variants improve delivery enough to offset review, governance, and model costs, the business case weakens quickly. As a result, winners in this period will not be the vendors with the most ambitious autonomy claims, but the ones that can package agent workflows into financially legible operating models with hard ROI thresholds and lower adoption friction.

Supporting observations: [obs: en-019d9565-1fd8-7343-abe2-7d4429a5ea9d], [obs: en-019d9565-5930-7530-b4ec-d6a35403486e]

**Counterfactual:** If this direction is wrong, enterprise buyers will tolerate weak or ambiguous ROI for longer than expected and the market will expand through strategic positioning rather than proven economic value.

#### Organizational/Adoption: Coding-agent rollout plateaus unless firms redesign incentives, accountability, and workforce messaging
**Direction ID:** en-019d9565-d200-7472-bfc9-4fa49bdf1f5e
**Theme:** organizational/adoption

The deeper adoption constraint over the next 180-365 days is organizational, not purely technical. Directed software evolution asks firms to reassign responsibility across developers, platform teams, QA, security, and managers, yet most organizations still reward local control and clear authorship more than system-level experimentation. That means even as model quality improves, adoption will remain shallow unless firms redesign operating norms: who approves generated variants, who owns regression risk, how evaluation budgets are funded, and how teams are measured when output is produced with agent assistance. Enterprises that fail to answer those questions will see agents used opportunistically by enthusiasts while the broader organization stalls.

The labor dimension amplifies this problem. When employees suspect coding agents are a headcount-reduction tool, they become less willing to share workflow data, standardize practices, or trust agent-generated changes. Managers may also resist because the tools compress supervision layers and make some coordination work less valuable. So the adoption winners in this horizon will be organizations that explicitly frame agents as a redesign of team structure and incentive systems, not just a developer productivity plugin. Those that do not will likely experience a plateau in production adoption even if individual developers continue to use the tools privately.

Supporting observations: [obs: en-019d9565-5930-7530-b4ec-d6a35403486e], [obs: en-019d9565-7a81-79c0-89f1-5a1d81469526]

**Counterfactual:** If this direction is wrong, model quality and product usability will be sufficient to drive broad adoption without major organizational restructuring or workforce-management intervention.

#### Evaluation/testing direction: repo-specific staged harnesses become the deployment gate for coding agents
**Direction ID:** en-019d9565-d6b8-7b13-9d6a-2630567a649e
**Theme:** evaluation/testing

Evaluation/testing direction: repository-specific harnesses will replace generic coding benchmarks as the gating layer for agent rollout. Public benchmarks are useful for model selection, but they do not capture the realities that determine whether an agent can safely evolve a live system: flaky integration tests, hidden architectural constraints, migration ordering, and local code review norms. As a result, mature teams will build evaluation loops that replay real tickets against sampled historical tasks, score candidate diffs on pass-rate and rework metrics, and only promote agents that clear internal thresholds over repeated runs.

This shift also changes the economics of experimentation. Once evaluation is staged and cached, teams can afford multi-candidate search because weak candidates are eliminated by static checks and focused unit suites before expensive end-to-end validation. That makes selection pressure explicit: the system keeps variants that improve measurable repo outcomes rather than variants that merely look plausible in chat. In practice, the engineering organizations that move first will treat evaluation harnesses as product infrastructure with dedicated ownership, versioning, and observability, similar to how serious ML teams treat offline and online eval pipelines.

Supporting observations: [obs: en-019d9565-42b5-7e03-8289-3d9bd183e683], [obs: en-019d9565-5c1d-76a1-9148-ca12030c891e]

**Counterfactual:** If teams do not adopt repo-specific staged evaluation, agent rollout decisions will continue to rely on weak proxies, causing costly false confidence and preventing disciplined evolutionary improvement.

#### Cross-Domain Analogy: Directed software evolution becomes portfolio management for code variants
**Direction ID:** en-019d9565-f4e6-7380-8758-dc36c1eadaf0
**Theme:** cross-domain

Cross-domain pattern: directed software evolution is converging on the logic of portfolio management, not the logic of single-model craftsmanship. In finance, robust firms stopped asking whether one strategy backtested well and instead built risk desks, scenario engines, and capital-allocation rules that continually reweight many competing bets. The same pattern is emerging here: code variants will be treated as a portfolio of adaptive positions, where promotion depends on risk-adjusted survival across shifting tasks, regression regimes, and policy constraints rather than on one benchmark snapshot. Observation en-019d9565-55ed-7ed2-b492-3c9405f18d0f points to ecological tournaments and multi-scenario win rates, while en-019d9565-98ac-7f42-9209-faf03adc7cc8 shows enterprise preference for control metrics like rollback frequency and defect escape rate.

The practical implication over 180-365 days is that the winning vendors and internal platforms will expose allocation logic: how much variant budget goes to exploration, how much to exploitation, when to retire a family of mutations, and what loss limits block promotion. That resembles portfolio risk controls more than classic software release management. Firms that internalize this analogy will build evolutionary systems with explicit drawdown controls, diversity quotas, and stress scenarios; firms that miss it will keep optimizing for mean benchmark gain and suffer concentrated tail risk when one brittle mutation family dominates production.

Supporting observations: [obs: en-019d9565-55ed-7ed2-b492-3c9405f18d0f], [obs: en-019d9565-98ac-7f42-9209-faf03adc7cc8]

**Counterfactual:** If this direction is wrong, benchmark-centric evaluation will remain sufficient and firms will not need portfolio-style allocation, stress testing, or risk budgeting for code variants.

### Source Thesis Challenges

1. **Challenge to the claim that better models are the main unlock for directed software evolution.** The observations suggest the stronger limiting mechanism is environment fidelity and execution scaffolding: without production-like devcontainers, dependency graphs, and replayable repo state, higher model quality produces brittle variance rather than safe improvement [obs: en-019d9565-7650-7812-859b-ccac41bc1205] [obs: en-019d9565-1dbb-75e2-b46e-b68a335fb96f]. This challenges any source thesis that treats model progress as sufficient for operational scale.

2. **Challenge to the claim that enterprise adoption will expand smoothly once developers like the tools.** The mechanism of failure is budget governance: CFO review, inference-cost scrutiny, duplicated tooling, and review overhead arrive before broad labor savings are proven, so enthusiasm does not automatically translate into standardization [obs: en-019d9565-1fd8-7343-abe2-7d4429a5ea9d] [obs: en-019d9565-5d83-7b80-988f-63f4e1731563]. This directly contradicts a high-confidence adoption narrative centered on developer delight alone.

3. **Challenge to the claim that generic coding benchmarks are an adequate proxy for production readiness.** Repo-specific constraints, flaky integration tests, migration ordering, and local review norms create a hidden failure channel that public benchmarks cannot represent, so firms that rely on leaderboard performance alone will overestimate readiness [obs: en-019d9565-42b5-7e03-8289-3d9bd183e683] [obs: en-019d9565-83d4-7d40-906d-8d97fcc95c68].

4. **Challenge to the claim that directed software evolution is mainly a software-engineering problem.** Adjacent-domain evidence from biology, manufacturing, and portfolio management implies the deeper mechanism is controlled selection under uncertainty: variant diversity, process control, and loss-limited allocation dominate single-run cleverness [obs: en-019d9565-27e5-76d2-9cd8-4b8f30c2e4c4] [obs: en-019d9565-44ca-7333-95c3-39de4857263f] [obs: en-019d9565-55ed-7ed2-b492-3c9405f18d0f]. This uses evidence outside the likely source frame and adds a blind spot the source may not have formalized.

5. **Challenge to the claim that agent rollout is a neutral productivity upgrade.** The observations show workforce politics and managerial incentive structures can actively suppress adoption by reducing willingness to share workflow data, standardize review processes, or trust generated changes [obs: en-019d9565-7a81-79c0-89f1-5a1d81469526] [obs: en-019d9565-7f6d-7911-bf68-815c6c2c9b5e]. The failure mechanism is social legitimacy, not technical defect.

### Top 5 Predictions with Falsification Criteria

1. **Prediction:** By 2027-03-31, most successful enterprise coding-agent programs will run through CI-native orchestration layers rather than IDE-only workflows.
   - **Measurable indicator:** At least 50% of productionized programs expose agent jobs through GitHub Actions, Buildkite, or Kubernetes-style runners with replay logs.
   - **Confidence:** high
   - **Falsification:** If fewer than one-third of mature deployments have moved beyond IDE/chat execution by 2027-03-31, this prediction is wrong because execution scaffolds would not be the practical bottleneck the probes identified.
   - **Supporting observations:** [obs: en-019d9565-1dbb-75e2-b46e-b68a335fb96f], [obs: en-019d9565-7650-7812-859b-ccac41bc1205]

2. **Prediction:** By 2027-06-30, enterprise spending on coding agents will consolidate around a small vendor set and require documented ROI hurdles for expansion.
   - **Measurable indicator:** More than 60% of enterprise-standard deployments are concentrated among 3-4 vendors, and internal business cases cite at least one labor or cycle-time metric.
   - **Confidence:** medium
   - **Falsification:** If broad seat expansion continues without explicit ROI gates by 2027-06-30, this prediction is wrong because finance discipline would be weaker than the market and cost observations imply.
   - **Supporting observations:** [obs: en-019d9565-1fd8-7343-abe2-7d4429a5ea9d], [obs: en-019d9565-39f5-7e03-9b12-67f27184ecd7]

3. **Prediction:** By 2027-03-31, firms that do not redesign approval, QA, and platform ownership for agent-generated code will see production adoption plateau after a few enthusiastic teams.
   - **Measurable indicator:** Fewer than 3 business-critical teams sustain weekly agent-driven change volume in organizations without formal accountability redesign.
   - **Confidence:** high
   - **Falsification:** If large organizations scale agent usage broadly without changing incentives, review ownership, or workforce messaging by 2027-03-31, this prediction is wrong because organizational redesign would not be the binding constraint.
   - **Supporting observations:** [obs: en-019d9565-5930-7530-b4ec-d6a35403486e], [obs: en-019d9565-7a81-79c0-89f1-5a1d81469526]

4. **Prediction:** By 2027-06-30, repo-specific staged evaluation harnesses will become the default deployment gate for serious coding-agent programs.
   - **Measurable indicator:** At least 70% of mature programs require a multi-stage pipeline combining static checks, unit tests, and integration or replay-based evaluation before promotion.
   - **Confidence:** high
   - **Falsification:** If benchmark scores and ad hoc human review remain the dominant gate by 2027-06-30, this prediction is wrong because internal harness economics and reliability pressure would not have materialized as expected.
   - **Supporting observations:** [obs: en-019d9565-42b5-7e03-8289-3d9bd183e683], [obs: en-019d9565-5c1d-76a1-9148-ca12030c891e]

5. **Prediction:** By 2027-06-30, the most resilient directed software evolution platforms will manage code variants with portfolio-style risk controls instead of single-run promotion logic.
   - **Measurable indicator:** Leading internal platforms expose explicit exploration budgets, rollback limits, or diversity quotas across candidate variants.
   - **Confidence:** medium
   - **Falsification:** If no major platform formalizes allocation rules or risk controls by 2027-06-30, this prediction is wrong because the cross-domain portfolio analogy would not have translated into operating practice.
   - **Supporting observations:** [obs: en-019d9565-55ed-7ed2-b492-3c9405f18d0f], [obs: en-019d9565-98ac-7f42-9209-faf03adc7cc8]

### Decision Points

#### Decision Point 1
- **Decision:** Whether to standardize on a CI-native execution substrate for coding agents now or keep experimentation inside IDE tools such as Cursor and Claude Code.
- **Timing trigger:** When more than 2 teams request production access for agent-generated changes, likely within the next 1-2 quarters [obs: en-019d9565-2236-7340-bf30-81e7ba17e99e].
- **Option A:** Deploy GitHub Actions-based branch-per-variant pipelines with ephemeral preview environments
  — **Tradeoff:** 2-4 engineering-weeks plus moderate platform maintenance overhead, but strong auditability and replay [obs: en-019d9565-5bfb-72f2-89f3-5dd9f803adf4].
- **Option B:** Build a Kubernetes job-runner control plane for agent execution
  — **Tradeoff:** 4-8 engineering-weeks and requires a dedicated platform team, but gives the highest environment fidelity and scale [obs: en-019d9565-7650-7812-859b-ccac41bc1205].
- **Option C:** Keep agent use in IDE-only workflows with manual PR handoff
  — **Tradeoff:** under 1 engineering-week to enable, but weak reproducibility and poor scaling beyond enthusiasts [obs: en-019d9565-1dbb-75e2-b46e-b68a335fb96f].
- **Recommended:** Option A, because it captures most of the control-plane benefit quickly without the heavier operating burden of a fully custom runner stack.

#### Decision Point 2
- **Decision:** What deployment gate should be required before agent-generated code can reach staging or production.
- **Timing trigger:** As soon as the organization sees its first rollback, flaky integration failure, or security-review escalation from agent-generated code, likely within 3-6 months [obs: en-019d9565-83d4-7d40-906d-8d97fcc95c68].
- **Option A:** Require a staged harness using static analysis, unit tests, and historical-ticket replay before merge
  — **Tradeoff:** 2-5 engineering-weeks to build and curate fixtures, but creates measurable promotion thresholds [obs: en-019d9565-42b5-7e03-8289-3d9bd183e683].
- **Option B:** Add OPA or Cedar policy gates to CI for risk scoring, approval routing, and rollback limits
  — **Tradeoff:** 3-6 engineering-weeks plus policy upkeep, but materially improves auditability and governance posture [obs: en-019d9565-98ac-7f42-9209-faf03adc7cc8].
- **Option C:** Continue with benchmark-based vendor selection plus human code review
  — **Tradeoff:** lowest short-term effort, but high false-confidence risk because local architectural constraints remain untested [obs: en-019d9565-42b5-7e03-8289-3d9bd183e683].
- **Recommended:** Option A first, then layer Option B, because repo-specific evidence is the missing reliability substrate and policy gates work best once evaluation outputs exist.

#### Decision Point 3
- **Decision:** How to frame coding-agent adoption organizationally: cost-cutting tool, platform capability, or controlled experimentation program.
- **Timing trigger:** Before annual planning or the next headcount review cycle, when workforce anxiety and budget scrutiny become visible, likely within 6 months [obs: en-019d9565-7a81-79c0-89f1-5a1d81469526].
- **Option A:** Create a cross-functional platform program spanning engineering, QA, security, and finance with explicit ownership of evals and rollout policy
  — **Tradeoff:** requires dedicated leadership attention and 1-2 program-manager equivalents, but best supports durable adoption [obs: en-019d9565-5930-7530-b4ec-d6a35403486e].
- **Option B:** Let each team adopt its preferred toolset independently
  — **Tradeoff:** near-zero central cost initially, but creates duplicated spend, fragmented controls, and likely niche dead ends [obs: en-019d9565-83cf-7022-8e74-6057ccec67ba].
- **Option C:** Position agents primarily as headcount-reduction levers in the next budgeting cycle
  — **Tradeoff:** may appear to save budget in the short term, but sharply raises trust, data-sharing, and adoption resistance risk [obs: en-019d9565-7f6d-7911-bf68-815c6c2c9b5e].
- **Recommended:** Option A, because organizational legitimacy is a prerequisite for scale and cannot be repaired cheaply after workforce trust is lost.

### Assumptions & Limitations

1. **Assumption:** Frontier model quality continues improving, but not fast enough to eliminate the need for scaffolds and environment control [obs: en-019d9565-7650-7812-859b-ccac41bc1205].
   - **If wrong:** Pure model improvements could compress the value of orchestration and shorten the time to broad deployment.
   - **Confidence:** medium

2. **Assumption:** Enterprise buyers will keep applying tighter ROI and governance discipline to coding-agent budgets over the next year [obs: en-019d9565-1fd8-7343-abe2-7d4429a5ea9d] [obs: en-019d9565-98ac-7f42-9209-faf03adc7cc8].
   - **If wrong:** The market could grow faster and stay more fragmented if strategic positioning outweighs near-term financial scrutiny.
   - **Confidence:** high

3. **Assumption:** Organizational politics and incentive redesign matter as much as technical readiness for rollout [obs: en-019d9565-5930-7530-b4ec-d6a35403486e] [obs: en-019d9565-7f6d-7911-bf68-815c6c2c9b5e].
   - **If wrong:** Adoption may scale more like a conventional developer tool rollout, with less friction from management structure and workforce signaling.
   - **Confidence:** medium

### Methodology
- 6 independent probes across two time steps synthesized into one projection.
- 24 total observations and 5 active directions were reviewed through the API.
- Claims were grounded in probe observations, with deliberate inclusion of practitioner, critic, and adjacent-domain perspectives.
- The synthesis emphasized theme diversity across technical architecture, economics/market, organizational/adoption, evaluation/testing, governance/policy, and cross-domain reasoning.

