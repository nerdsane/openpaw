# Foresight Projection: Directed Software Evolution v2
## Horizon: 1 year | Steps: 2 | Date: 2026-04-16

### Executive Summary

Anthropic, OpenAI, Cursor, GitHub Actions, Kubernetes, Cedar, OPA, OpenHands, Aider, Cline, ArgoCD, Terraform, and Temper together point to a dominant trajectory in which software organizations stop treating coding agents as isolated assistants and start operating them as governed production systems. In the first 12 months, the durable wins come from teams that bind frontier models to execution substrates, policy gates, evaluation harnesses, and rollback paths, not from teams that merely switch to the latest model or IDE surface. The strongest near-term pattern is the rise of the agent workbench and the agent factory: model vendors capture mindshare, but operational control lives in stacks that combine orchestration, CI, policy, and observability [obs: en-019d9529-7184-78b2-8656-0c3911058cc6], [obs: en-019d9529-719a-7222-86d0-216e04fef4ea], [obs: en-019d9529-7240-7e83-9cf7-47fd3d9f6ef7].

The main counterargument is that autonomy headlines hide stubborn production constraints. Evaluation harnesses still miss repository-specific regressions, merge conflicts and review queues create coordination debt, and regulated environments keep humans in the approval loop far longer than demo narratives suggest. This means many organizations will discover that adding more agents without repo hygiene, policy gates, and acceptance-rate measurement increases cloud spend and exception handling faster than it increases delivered throughput [obs: en-019d9529-71cf-7a42-bc43-c998f0ab9b9a], [obs: en-019d9529-743a-78c1-9334-277f660a8b24], [obs: en-019d9529-7456-7dd1-8423-cbef78925143].

For decision-makers, the practical implication is to spend less time debating one-vendor standardization and more time building the control plane for directed software evolution. A realistic target is a 20-30% reduction in PR cycle time on well-instrumented services, paired with policy coverage on 100% of production deployment paths and repo-specific evaluation pass thresholds above 85% before autonomy is expanded. Organizations that reach those thresholds within 6-12 months can scale agents as a portfolio; those that do not will see adoption stall at local productivity gains rather than platform-level compounding [obs: en-019d9529-7409-70b2-9ead-adce61e847b5], [obs: en-019d9529-744a-7bf2-a18b-76434ac61973], [obs: en-019d9529-748c-71f3-a05a-051e87e501fb].

### Key Findings

1. **Cursor and Anthropic push software work toward governed agent workbenches rather than standalone chat, with GitHub Actions becoming the operational spine for repeatable autonomous tasks.**
   - Evidence: "Cursor, Anthropic Claude Code, and OpenAI Codex-style workflows converge on long-running agent loops built around repo context, planning checkpoints, " [obs: en-019d9529-7184-78b2-8656-0c3911058cc6]
   - Measurable indicator: Target >20% reduction in PR cycle time within 90 days
   - Theme: technical architecture

2. **Cedar, OPA, and Sentinel become purchasing criteria for agent deployment because security teams require policy gates before autonomous merge or deploy rights are granted.**
   - Evidence: "Cedar, OPA, and Sentinel-style policy layers become mandatory not because regulators immediately require them, but because internal audit, security, a" [obs: en-019d9529-71e5-7703-add8-e05408e111e7]
   - Measurable indicator: Policy coverage threshold: 100% of production deploy paths gated by policy-as-code
   - Theme: governance/policy

3. **Cursor, Devin, and OpenHands behave like semi-specialized labor pools, forcing managers to design queueing, review, and exception-handling workflows rather than simply buying more seats.**
   - Evidence: "From an organizational-theory lens, software teams are rediscovering the factory: Cursor, Devin, and OpenHands act less like individual tools and more" [obs: en-019d9529-721a-7d81-beaf-e26f0082ead2]
   - Measurable indicator: Sustain >30% acceptance rate on agent-submitted changes before scaling headcount equivalents
   - Theme: organizational/adoption

4. **Anthropic and OpenAI still matter, but the economics tilt toward assemblers that combine model endpoints with Temper-style orchestration, observability, and policy controls.**
   - Evidence: "Market structure shifts toward platform assemblers: firms that combine Anthropic or OpenAI models with Cedar or OPA policies, GitHub Actions, observab" [obs: en-019d9529-7240-7e83-9cf7-47fd3d9f6ef7]
   - Measurable indicator: Budget mix shifts toward 15-25% spend on orchestration/evaluation layers instead of model tokens alone
   - Theme: economics/market

5. **SWE-style evaluation packs and repository-specific regression suites become the real gatekeepers of autonomy, overtaking benchmark theater as the measure that matters for deployment.**
   - Evidence: "Evaluation matures from benchmark theater into deployment gates: SWE-style task suites, repository-specific regression packs, and change-failure-rate " [obs: en-019d9529-7409-70b2-9ead-adce61e847b5]
   - Measurable indicator: Require pass rates above 85% on repo-specific regression packs
   - Theme: evaluation/testing

6. **Biology and finance analogies become operational reality as teams route cheap exploratory work to open-source agents and reserve premium Anthropic or OpenAI capacity for verification-heavy tasks.**
   - Evidence: "Cross-domain analogies harden into practice: organizations adopt portfolio logic from finance and mutation-selection logic from biology, assigning man" [obs: en-019d9529-747c-70b1-9c8d-a3b3ad51522e]
   - Measurable indicator: At least 3:1 ratio of exploratory-agent runs to verifier-agent runs
   - Theme: cross-domain

7. **Kubernetes-era engineering discipline determines who benefits: teams with >70% test coverage and platform support capture gains while brittle monoliths mostly buy expensive noise.**
   - Evidence: "Economic returns polarize: teams with mature repo hygiene, test coverage above roughly 70%, and platform engineering support capture outsized gains, w" [obs: en-019d9529-744a-7bf2-a18b-76434ac61973]
   - Measurable indicator: Repository test coverage threshold: 70%+
   - Theme: organizational/adoption

8. **ERP-like rollout dynamics mean GitHub, GitLab, and internal developer portals only deliver value when paired with manager training, process redesign, and explicit supervision norms.**
   - Evidence: "Enterprise adoption resembles ERP rollouts more than consumer SaaS adoption: value depends on process redesign, internal standards, and training manag" [obs: en-019d9529-748c-71f3-a05a-051e87e501fb]
   - Measurable indicator: Allocate 2-4 engineering-weeks plus manager enablement per business unit rollout
   - Theme: model/vendor

### Temporal Progression

#### Phase 1: 0-3 Months (days 0-90)
- Cursor, Anthropic Claude Code, GitHub Actions, Kubernetes, and Terraform define the early winning stack as teams move from IDE chat to governed agent workbenches with bounded execution [obs: en-019d9529-7184-78b2-8656-0c3911058cc6], [obs: en-019d9529-719a-7222-86d0-216e04fef4ea].
- Expected signals: at least one internal platform team formalizes agent runners, permission scopes, and staging-only deploy rights; ArgoCD pilots appear alongside existing CI paths by late quarter [obs: en-019d9529-719a-7222-86d0-216e04fef4ea], [obs: en-019d9529-71a9-7372-bd3f-60bcab4c3602].
- What has NOT changed that was expected to: fully autonomous production deploys still remain rare because policy gates and evaluation are not mature enough [obs: en-019d9529-71cf-7a42-bc43-c998f0ab9b9a], [obs: en-019d9529-71e5-7703-add8-e05408e111e7].
- Causal link to Phase 2: once governed execution exists, security and platform teams can enforce Cedar, OPA, or Sentinel constraints instead of relying only on manual trust.

#### Phase 2: 3-6 Months (days 90-180)
- Cedar, OPA, Sentinel, and OpenHands enter the core conversation as organizations realize that authorization, policy distribution, and evaluation, not raw generation alone, control safe scaling [obs: en-019d9529-71e5-7703-add8-e05408e111e7], [obs: en-019d9529-71f6-7fe0-af93-25bfed8822f2].
- Expected signals: policy-as-code reviews become part of CI templates, and at least one business unit requires explicit policy coverage before an agent can merge into protected branches [obs: en-019d9529-71e5-7703-add8-e05408e111e7].
- **Revisions to earlier predictions:** The Phase 1 expectation of quick autonomous rollout is qualified: agent workbenches do spread, but release authority remains bounded by policy and evaluation; the prediction of rapid organization-wide standardization is revised downward for regulated environments [obs: en-019d9529-71cf-7a42-bc43-c998f0ab9b9a], [obs: en-019d9529-721a-7d81-beaf-e26f0082ead2].
- Causal link to Phase 3: once policies are encoded, organizations can begin separating cheap exploratory runs from expensive verification runs.

#### Phase 3: 6-9 Months (days 180-270)
- Datadog-style observability, SWE-bench-like evaluation suites, and internal developer portals become critical because teams need to measure acceptance rate, change-failure rate, and rollback cost by agent tier [obs: en-019d9529-7409-70b2-9ead-adce61e847b5], [obs: en-019d9529-749e-7580-8938-1b5154c54515].
- Expected signals: repository-specific evaluation packs are linked to merge checks, and model routing rules separate exploratory, refinement, and verifier workflows by cost/risk class [obs: en-019d9529-73f8-7b90-a55e-ed397c08a89f], [obs: en-019d9529-747c-70b1-9c8d-a3b3ad51522e].
- **Revisions to earlier predictions:** Phase 1 hopes for simple model standardization are revised: the field moves toward layered brokerage. Phase 2 optimism on policy alone is also qualified because coordination debt and poor repo hygiene limit gains even when guardrails exist [obs: en-019d9529-73f8-7b90-a55e-ed397c08a89f], [obs: en-019d9529-743a-78c1-9334-277f660a8b24], [obs: en-019d9529-744a-7bf2-a18b-76434ac61973].
- Causal link to Phase 4: once measurement intermediaries mature, buyers can price trust and optimize portfolios instead of debating anecdotes.

#### Phase 4: 9-12 Months (days 270-365)
- GitLab, ArgoCD, Cline, Aider, and internal orchestration layers such as Temper coexist in a more stratified market where model vendors, workflow orchestrators, and measurement intermediaries each own distinct layers of value [obs: en-019d9529-741b-7e91-b885-1fe69b64db8d], [obs: en-019d9529-748c-71f3-a05a-051e87e501fb], [obs: en-019d9529-749e-7580-8938-1b5154c54515].
- Expected signals: buyers compare agent acceptance-rate dashboards, policy exception volume, and time-to-rollback as standard procurement artifacts, not just benchmark scores [obs: en-019d9529-749e-7580-8938-1b5154c54515].
- **Revisions to earlier predictions:** Early claims of near-linear agent scaling are falsified for low-discipline teams; predictions about layered execution governance are confirmed; predictions of immediate full autonomy are revised to hybrid human-plus-policy supervision for most critical services [obs: en-019d9529-743a-78c1-9334-277f660a8b24], [obs: en-019d9529-744a-7bf2-a18b-76434ac61973], [obs: en-019d9529-7456-7dd1-8423-cbef78925143].
- **Final state assessment:** At day 365, directed software evolution is real but uneven: high-discipline organizations operate agent portfolios with clear trust metrics, while others remain stuck at pilot scale because governance, evaluation, and coordination debt outpace enthusiasm [obs: en-019d9529-747c-70b1-9c8d-a3b3ad51522e], [obs: en-019d9529-748c-71f3-a05a-051e87e501fb].

### Active Directions

#### Directed software evolution consolidates around governed execution substrates, not standalone coding copilots
**Direction ID:** en-019d9529-71b9-7551-9e32-2b0c77281988

Near-term adoption clusters around tools that let organizations decompose work into bounded steps, run those steps in controlled environments, and preserve an audit trail across planning, code generation, testing, and deployment. Cursor and Anthropic may capture the attention layer, but Kubernetes-backed runners, GitHub Actions pipelines, Terraform plans, and orchestration systems like Temper define whether agents graduate from individual productivity tools into organization-level delivery systems. Supporting evidence points toward teams valuing recoverability, review checkpoints, and environment isolation at least as much as raw code generation throughput.

This means the practical moat shifts away from prompt UX alone and toward the operating model for directed execution. Vendors or internal platforms that can bind agent loops to repo permissions, policy evaluation, rollout controls, and post-change observability will see more durable adoption than tools optimized only for synchronous IDE chat.

Supporting observations: [obs: en-019d9529-7184-78b2-8656-0c3911058cc6], [obs: en-019d9529-719a-7222-86d0-216e04fef4ea], [obs: en-019d9529-71a9-7372-bd3f-60bcab4c3602]

**Counterfactual:** If this thesis is wrong, the market will remain fragmented around local IDE productivity gains and platform teams will delay deeper orchestration investments.

#### Governance and evaluation bottlenecks will cap agent autonomy before model quality does
**Direction ID:** en-019d9529-7208-7280-942b-b151ea3cb7d7

The fastest-moving vendors can demonstrate impressive autonomous coding sessions, but production software delivery is constrained by what organizations can safely verify, authorize, and roll back. Cedar, OPA, and Sentinel are not decorative compliance layers; they become the mechanism that determines where autonomous execution can occur and where humans must remain in the loop. The more powerful the model, the more valuable these boundary systems become because blast radius increases with capability.

As a result, the near-term ceiling on autonomy is not only benchmark score improvements from Anthropic, OpenAI, or open-source frontier models. It is the maturity of evaluation harnesses, release policies, and exception-management workflows. Teams that fail to build these controls will be forced back into manual review after the first notable failure.

Supporting observations: [obs: en-019d9529-71cf-7a42-bc43-c998f0ab9b9a], [obs: en-019d9529-71e5-7703-add8-e05408e111e7], [obs: en-019d9529-71f6-7fe0-af93-25bfed8822f2]

**Counterfactual:** If this thesis is wrong, model reliability will improve so quickly that policy friction becomes secondary and teams will accept much looser controls.

#### The strategic unit is the agent factory, not the individual model
**Direction ID:** en-019d9529-7250-77f2-8895-0352883c6330

Looking from adjacent domains such as manufacturing systems and evolutionary biology, the salient change is not the emergence of one superior model but the emergence of a production architecture that can generate many candidate changes, test them cheaply, and permit only safe variants to propagate. In software, that architecture combines model vendors like Anthropic and OpenAI with workflow systems, policy engines, CI pipelines, and evaluation harnesses. This is why market power increasingly accrues to teams that can compose these pieces into a repeatable agent factory rather than to those who merely expose another chat endpoint.

The organizational consequence is equally important: management attention shifts from selecting a single best assistant to designing queues, acceptance criteria, feedback loops, and escalation paths. That is a deeper operating-model change than swapping one IDE plugin for another.

Supporting observations: [obs: en-019d9529-721a-7d81-beaf-e26f0082ead2], [obs: en-019d9529-722e-7ca2-9834-7297c5e2fc0f], [obs: en-019d9529-7240-7e83-9cf7-47fd3d9f6ef7]

**Counterfactual:** If this thesis is wrong, individual model UX and pricing will dominate, and internal platform investments will appear overbuilt.

#### The durable enterprise stack separates model selection from execution governance
**Direction ID:** en-019d9529-742a-7303-86e9-761f278c6427

Over a one-year horizon, organizations converge on a layered design. Frontier models from Anthropic and OpenAI, plus lower-cost specialist or open-weight options, are routed according to task complexity and risk. But that routing sits above a more stable execution layer composed of Kubernetes sandboxes, CI systems, secrets boundaries, approval workflows, and observability hooks. This separation lets teams improve model performance without rebuilding operational controls every quarter.

The strategic payoff is procurement and operational flexibility. Enterprises can swap or mix models while preserving evaluation gates, audit trails, and environment controls. The stack starts to look more like cloud infrastructure brokerage than a single monolithic AI coding product.

Supporting observations: [obs: en-019d9529-73f8-7b90-a55e-ed397c08a89f], [obs: en-019d9529-7409-70b2-9ead-adce61e847b5], [obs: en-019d9529-741b-7e91-b885-1fe69b64db8d]

**Counterfactual:** If this thesis is wrong, one end-to-end vendor will capture enough trust to collapse the stack into a vertically integrated platform.

#### Coordination debt becomes the hidden tax on multi-agent software organizations
**Direction ID:** en-019d9529-7468-7d43-a322-fa95b8194bf8

A year in, the organizations that merely scale agent counts discover a new bottleneck: not code generation, but coordination. More agents generate more partial solutions, more branch contention, more environment contention, and more review exceptions. Unless the underlying repo architecture, ownership boundaries, and evaluation gates are disciplined, the organization experiences the software equivalent of adding workers to a congested factory floor.

This creates a bifurcation in outcomes. High-discipline teams with strong tests, modular services, and platform controls continue compounding value; low-discipline teams see cloud costs and governance overhead rise faster than useful output. That hidden tax will shape adoption narratives more than benchmark headlines do.

Supporting observations: [obs: en-019d9529-743a-78c1-9334-277f660a8b24], [obs: en-019d9529-744a-7bf2-a18b-76434ac61973], [obs: en-019d9529-7456-7dd1-8423-cbef78925143]

**Counterfactual:** If this thesis is wrong, coordination tooling and model planning improve enough that throughput scales close to linearly with additional agents.

#### Software organizations will manage agents like an investment portfolio with explicit risk tranching
**Direction ID:** en-019d9529-74b3-7c82-a068-7a2913254203

By the end of the year, the best operators treat agent capacity as capital allocation. Cheap exploratory models and open-source agents search for options; mid-tier systems refine the promising branches; high-trust verifier agents, policy engines, and human reviewers approve only the changes that satisfy explicit acceptance criteria. This mirrors portfolio management and biological selection more than traditional seat-based software purchasing.

That framing matters because it changes what leaders measure. Instead of counting seats or prompts, they track acceptance rate, rollback rate, policy exception volume, and throughput adjusted for rework. The organizations that learn this measurement discipline become better at compounding agent value than those that chase model novelty alone.

Supporting observations: [obs: en-019d9529-747c-70b1-9c8d-a3b3ad51522e], [obs: en-019d9529-748c-71f3-a05a-051e87e501fb], [obs: en-019d9529-749e-7580-8938-1b5154c54515]

**Counterfactual:** If this thesis is wrong, seat-based pricing and one-model standardization remain the dominant operating logic.

### What Surprised Us

- **The dominant narrative overestimates fully autonomous software delivery: evaluation harnesses still miss cross-service r** [obs: en-019d9529-71cf-7a42-bc43-c998f0ab9b9a]
  Why surprising: This challenges the assumption that better models automatically make production delivery safe.
- **Open-source agent stacks like OpenHands, Aider, and Cline accelerate experimentation but also compress differentiation; ** [obs: en-019d9529-71f6-7fe0-af93-25bfed8822f2]
  Why surprising: This challenges the assumption that open-source experimentation alone creates durable differentiation.
- **A visible failure mode emerges around hidden coordination costs: as organizations add more agents, they create more merg** [obs: en-019d9529-743a-78c1-9334-277f660a8b24]
  Why surprising: This challenges the assumption that adding more agents scales output linearly.
- **Regulated organizations adopt autonomous change slowly enough that human sign-off remains attached to production deploym** [obs: en-019d9529-7456-7dd1-8423-cbef78925143]
  Why surprising: This challenges the assumption that regulated industries will quickly remove human sign-off.
- **The market rewards measurement intermediaries: vendors that track change-failure rate, time-to-rollback, acceptance-rate** [obs: en-019d9529-749e-7580-8938-1b5154c54515]
  Why surprising: This challenges the assumption that model vendors alone will define market power.

### Top 5 Predictions with Falsification Criteria

1. **Prediction:** By Q4 2026, at least one-third of large platform engineering groups standardize a Cedar, OPA, or Sentinel gate before agents can deploy to production.
   - **Measurable indicator:** >=33% of surveyed large platform teams report policy-gated agent deploy workflows
   - **Confidence:** high
   - **Falsification:** If named platform teams have not published or deployed policy-gated agent deploy workflows by 2026-12-31, this prediction is wrong because governance integration would not have become a practical deployment requirement.
   - **Supporting observations:** [obs: en-019d9529-7184-78b2-8656-0c3911058cc6], [obs: en-019d9529-719a-7222-86d0-216e04fef4ea], [obs: en-019d9529-71a9-7372-bd3f-60bcab4c3602]

2. **Prediction:** By Q1 2027, enterprises running coding agents at scale route at least three task classes across different models or agent tiers instead of relying on a single frontier model.
   - **Measurable indicator:** >=3 production task classes mapped to separate cost/risk tiers
   - **Confidence:** medium
   - **Falsification:** If most enterprise rollouts still use a single model for planning, coding, and verification by 2027-03-31, this prediction is wrong because layered model brokerage would not have delivered enough operational value.
   - **Supporting observations:** [obs: en-019d9529-71cf-7a42-bc43-c998f0ab9b9a], [obs: en-019d9529-71e5-7703-add8-e05408e111e7], [obs: en-019d9529-71f6-7fe0-af93-25bfed8822f2]

3. **Prediction:** By Q1 2027, repository-specific evaluation harnesses become a release prerequisite for agent-authored changes in high-change services.
   - **Measurable indicator:** >=85% pass threshold on repo-specific suites before merge
   - **Confidence:** high
   - **Falsification:** If agent-authored changes are still merged primarily on human judgment without repo-linked evaluation thresholds by 2027-03-31, this prediction is wrong because evaluation would have remained theater rather than an operational control.
   - **Supporting observations:** [obs: en-019d9529-721a-7d81-beaf-e26f0082ead2], [obs: en-019d9529-722e-7ca2-9834-7297c5e2fc0f], [obs: en-019d9529-7240-7e83-9cf7-47fd3d9f6ef7]

4. **Prediction:** By Q1 2027, organizations that scale agent counts without repo hygiene or modular boundaries will report flat or negative throughput gains after cloud and review costs are included.
   - **Measurable indicator:** <10% net throughput gain at equal or higher cloud spend
   - **Confidence:** medium
   - **Falsification:** If poorly instrumented, low-coverage teams still show strong net throughput gains by 2027-03-31, this prediction is wrong because coordination debt would not have emerged as the dominant tax.
   - **Supporting observations:** [obs: en-019d9529-73f8-7b90-a55e-ed397c08a89f], [obs: en-019d9529-7409-70b2-9ead-adce61e847b5], [obs: en-019d9529-741b-7e91-b885-1fe69b64db8d]

5. **Prediction:** By Q2 2027, leading software organizations manage agents as a portfolio with explicit exploratory, refinement, and verification tiers.
   - **Measurable indicator:** At least 3 named agent tiers with acceptance-rate and rollback metrics
   - **Confidence:** medium
   - **Falsification:** If leading adopters still buy and measure agents as undifferentiated seats by 2027-06-30, this prediction is wrong because portfolio-style operating models would not have proven superior.
   - **Supporting observations:** [obs: en-019d9529-743a-78c1-9334-277f660a8b24], [obs: en-019d9529-744a-7bf2-a18b-76434ac61973], [obs: en-019d9529-7456-7dd1-8423-cbef78925143]

### Decision Points

#### Decision Point 1
- **Decision:** Decide whether to deploy Cedar or OPA policy gates on CI/CD paths that agents can touch.
- **Timing trigger:** First autonomous code path is proposed for staging or production use, likely within 1-2 quarters.
- **Option A:** Deploy Cedar policy gates on CI pipelines and runner permissions by Q3 2026 — **Tradeoff:** 3-5 engineering-weeks plus policy authoring overhead for platform and security teams.
- **Option B:** Deploy OPA admission and pipeline policies across Kubernetes and GitHub Actions — **Tradeoff:** 4-6 engineering-weeks and requires policy distribution plus rego expertise.
- **Option C:** Keep human-only release approvals with no machine-enforced policy layer — **Tradeoff:** lowest near-term effort, but adds ongoing manual review cost and higher compliance risk.
- **Recommended:** Option A, because Cedar gives fine-grained authorization semantics that fit agent action boundaries and scales cleanly into a broader governed-execution model.

#### Decision Point 2
- **Decision:** Decide whether to standardize agent execution on Kubernetes sandboxes with GitHub Actions/ArgoCD or leave agents inside IDE-local tools only.
- **Timing trigger:** When more than 2 teams request autonomous workflow support or incident-response automation, likely by late 2026.
- **Option A:** Build Kubernetes-run agent sandboxes with GitHub Actions and ArgoCD rollout controls — **Tradeoff:** 6-10 engineering-weeks and requires dedicated platform team ownership.
- **Option B:** Use vendor-managed execution inside Cursor/Anthropic tooling plus limited webhook integrations — **Tradeoff:** 2-3 engineering-weeks but increases vendor dependence and weakens cross-workflow governance.
- **Option C:** Restrict agents to IDE assistance with no shared execution substrate — **Tradeoff:** near-zero platform cost, but caps organizational ROI and prevents reusable audit trails.
- **Recommended:** Option A, because governed shared execution compounds across code, infra, and incident workflows instead of trapping value inside individual seats.

#### Decision Point 3
- **Decision:** Decide whether to fund repo-specific evaluation harnesses and acceptance-rate dashboards before scaling agent seats.
- **Timing trigger:** Agent-authored pull requests exceed roughly 10% of weekly PR volume, likely by Q4 2026.
- **Option A:** Build SWE-style repo evaluation packs and change-failure dashboards in Datadog/GitHub — **Tradeoff:** 4-8 engineering-weeks plus ongoing maintenance of task suites.
- **Option B:** Buy vendor evaluation tooling and integrate with existing CI checks — **Tradeoff:** $50K-100K annual spend and less control over task realism.
- **Option C:** Scale based on anecdotal developer satisfaction and spot review only — **Tradeoff:** minimal setup effort, but high risk of false confidence and hidden rework.
- **Recommended:** Option A, because internally grounded evaluation is the clearest way to price trust and detect where autonomy should stop.

### Assumptions & Limitations

1. **Assumption:** Frontier model quality from Anthropic, OpenAI, and peers continues improving enough to make orchestration and policy the next bottlenecks.
   - **If wrong:** More value remains in prompt UX and local IDE assistance than in governed execution systems.
   - **Confidence:** medium

2. **Assumption:** Enterprises are willing to fund platform engineering work for policy, evaluation, and sandboxing rather than expecting vendor tools to solve everything.
   - **If wrong:** Adoption fragments into vendor-managed silos with weaker portability and lower organization-wide leverage.
   - **Confidence:** high

3. **Assumption:** Acceptance-rate, rollback-rate, and policy-exception metrics become legible enough to guide procurement and operating decisions.
   - **If wrong:** The market remains benchmark- and demo-driven, increasing volatility and slowing trust formation.
   - **Confidence:** medium

### Methodology
- 3 independent probes per step
- 2 time steps over 1 year
- 18 total observations, 6 active directions
- Observation IDs: [en-019d9529-7184-78b2-8656-0c3911058cc6, en-019d9529-719a-7222-86d0-216e04fef4ea, en-019d9529-71a9-7372-bd3f-60bcab4c3602, en-019d9529-71cf-7a42-bc43-c998f0ab9b9a, en-019d9529-71e5-7703-add8-e05408e111e7, en-019d9529-71f6-7fe0-af93-25bfed8822f2, en-019d9529-721a-7d81-beaf-e26f0082ead2, en-019d9529-722e-7ca2-9834-7297c5e2fc0f, en-019d9529-7240-7e83-9cf7-47fd3d9f6ef7, en-019d9529-73f8-7b90-a55e-ed397c08a89f, en-019d9529-7409-70b2-9ead-adce61e847b5, en-019d9529-741b-7e91-b885-1fe69b64db8d, en-019d9529-743a-78c1-9334-277f660a8b24, en-019d9529-744a-7bf2-a18b-76434ac61973, en-019d9529-7456-7dd1-8423-cbef78925143, en-019d9529-747c-70b1-9c8d-a3b3ad51522e, en-019d9529-748c-71f3-a05a-051e87e501fb, en-019d9529-749e-7580-8938-1b5154c54515]

