# Foresight Projection: Directed Software Evolution v2
## Horizon: 1 year | Steps: 2 | Date: 2026-04-16

### Executive Summary

Directed software evolution is moving first through governed repository workflows, not through unconstrained autonomous engineers. Cursor, GitHub Actions, Aider, Cline, Kubernetes, and Terraform appear repeatedly as the practical substrate because they keep agent work inside pull-request, CI, and infra-plan boundaries that teams already trust. Cedar, OPA, and Temper-like audit layers matter because the binding problem is making agent actions legible and reversible, not merely generating more code. Net gains are real, but most evidence points to roughly 20-30% cycle-time improvement on well-tested services rather than broad end-to-end autonomy, and regulated environments often realize only 5-15% after governance overhead is counted. [obs: en-019d950e-f67b-7232-8e86-7dc89477d553] [obs: en-019d950e-f685-7f63-b850-1f8ecf8e750a] [obs: en-019d950e-5082-7293-b993-32e10e648678]

The main counterargument is that model progress from Anthropic, OpenAI, Google, and integrated products such as Devin or GitHub Copilot could outrun these control bottlenecks. The observations do not support that yet. Instead they show evaluation debt, policy opacity, and rollback gaps compounding as autonomy rises: Cedar or OPA approval logic is not the same thing as safety, green CI is not the same thing as socio-technical readiness, and reverting code is often easier than reverting the authorization and workflow effects around it. That is why governance complexity, not benchmark capability, looks like the near-term ceiling. [obs: en-019d950e-f6aa-7d12-bf70-8ea0b70dccef] [obs: en-019d950e-5079-7261-9b83-0ee096679246] [obs: en-019d950f-eab7-7f51-b41d-20026baebb6d]

For decision-makers, the implication is to treat this as a platform-and-operations transition spanning vendors, governance engines, open wrappers, and internal control planes. Anthropic and OpenAI access matters, but so do OpenHands, LiteLLM-style routing, Sentinel, and Temper because durable advantage comes from selection pressure, auditability, and routing discipline. The likely one-year outcome is a two-tier market: platform-mature firms that standardize templates, routing, and evidence capture can exceed 30% throughput gain, while fragmented peers stay near 5-10% and misdiagnose coordination failures as model weakness. [obs: en-019d950e-f6e1-7d03-beb6-50022653533e] [obs: en-019d9510-501c-77c1-9866-d31f4593e64b] [obs: en-019d9510-5025-7f73-b27b-f2857d3ba9b5]

### Key Findings

1. **Cursor and GitHub Actions are becoming the default high-trust delivery pair for agent-authored pull requests in well-tested services.**
   - Evidence: "By July 2026, Cursor and GitHub Actions become the default pairing for high-trust agent-assisted delivery: teams let agents draft pull requests, but k" [obs: en-019d950e-f67b-7232-8e86-7dc89477d553]
   - Measurable indicator: 20-30% cycle-time reduction on well-tested services by July 2026
   - Theme: technical architecture

2. **Cedar, OPA, and Sentinel will spread only where policy decisions are observable enough for application teams to debug and appeal.**
   - Evidence: "Cedar, OPA, and Sentinel adoption rises, but policy gates initially create developer resistance because rules are opaque and exception handling is imm" [obs: en-019d950e-f6b3-7f81-b9a5-e8727f13568e]
   - Measurable indicator: keep exception-handling latency below 1 business day and false-block rate below 5%
   - Theme: governance/policy

3. **Temper-style audit trails and bounded agent contracts matter because firms increasingly manage agents like junior contractors, not like autocomplete.**
   - Evidence: "From an organizational-theory lens, software teams start treating agents less like tools and more like junior contractors: they get bounded scopes, in" [obs: en-019d950e-f6d8-7cd3-9669-ce542a641bef]
   - Measurable indicator: 1 auditable task contract and owner per agent-generated change set
   - Theme: organizational/adoption

4. **Anthropic, OpenAI, and OpenHands-style routing stacks are pushing enterprises toward portfolio architectures instead of single-vendor dependence.**
   - Evidence: "Economically, the market shifts toward portfolio architectures: enterprises split workloads across Anthropic, OpenAI, open-source models via OpenHands" [obs: en-019d950e-f6e1-7d03-beb6-50022653533e]
   - Measurable indicator: at least 2 model vendors plus 1 open wrapper in production routing by Q4 2026
   - Theme: economics/market

5. **Anthropic, OpenAI, and Google coding models are outpacing repo-specific eval capacity, making evaluation debt the first hard scaling constraint.**
   - Evidence: "By late summer 2026, the main limiter is evaluation debt: Anthropic-, OpenAI-, and Google-backed coding agents can generate plausible changes faster t" [obs: en-019d950e-f6aa-7d12-bf70-8ea0b70dccef]
   - Measurable indicator: escaped-defect rate must stay under 1 per 100 agent-authored merges to justify broader rollout
   - Theme: evaluation/testing

6. **Temper plus Cedar or OPA is emerging as a trust layer analogous to cloud control planes: governance and replay beat raw coding fluency.**
   - Evidence: "Directed software evolution platforms will begin to differentiate the way cloud platforms did: not on raw model quality alone, but on who controls the" [obs: en-019d950e-7097-7f40-990e-38d0e845c3b7]
   - Measurable indicator: 100% of high-risk agent actions should carry replayable audit logs before production write access expands
   - Theme: cross-domain

7. **Terraform, Kubernetes, and GitHub Actions rollouts will deliver less than headline productivity claims unless governance overhead is explicitly budgeted.**
   - Evidence: "The dominant narrative that agentic development will deliver 20-40% throughput gains within one planning quarter is overstated for regulated or multi-" [obs: en-019d950e-5082-7293-b993-32e10e648678]
   - Measurable indicator: regulated teams should plan on net gains closer to 5-15% before controls mature
   - Theme: economics/market

8. **Temper-like coordination primitives are becoming a technical moat because durable agent state and auditability matter more than prompt UX alone.**
   - Evidence: "Temper-like control planes gain relevance because teams want entity-level audit trails, bounded actions, and recoverable workflow state across many se" [obs: en-019d9510-4fbb-7121-92d8-3bce61df5ce3]
   - Measurable indicator: target sub-5-minute reconstruction time from action log to workflow state during incidents
   - Theme: technical architecture


### Temporal Progression

#### Phase 1: 0-3 Months (days 0-90)
- Cursor, Aider, Cline, GitHub Actions, GitLab, Cedar, and OPA push teams toward branch-per-task agent workflows with mandatory CI evidence rather than direct-to-main autonomy. Practical gains cluster in test generation, low-risk refactors, dependency bumps, and internal tooling. [obs: en-019d950e-547d-7b20-9350-496f9f7cdda3] [obs: en-019d950e-548f-7b71-bed8-5ae971bd1e6b] [obs: en-019d950e-508a-7cb1-8683-aae7c984a692]
- Expected signals: more agent-authored pull requests, mandatory CI gates, advisory-mode policy comments, CODEOWNERS cleanup, and preview-environment adoption by late summer 2026. [obs: en-019d950e-f67b-7232-8e86-7dc89477d553] [obs: en-019d950e-5069-79c2-a283-1f0abdd007f3] [obs: en-019d950e-5497-7721-a210-303a0efea7b4]
- What has NOT changed that was expected to: broad unsupervised infrastructure mutation still does not arrive; teams keep Terraform and Kubernetes execution behind existing approval systems. [obs: en-019d950e-5486-73e1-a90c-d9fbb8da2adc] [obs: en-019d950e-508a-7cb1-8683-aae7c984a692]
- Causal link to Phase 2: once repo-native agent use expands, evaluation debt and policy usability become the next visible constraints. [obs: en-019d950e-f6aa-7d12-bf70-8ea0b70dccef] [obs: en-019d950e-f6b3-7f81-b9a5-e8727f13568e]

#### Phase 2: 3-6 Months (days 90-180)
- Kubernetes, Terraform, Helm, Atlantis, and repo-specific evaluation harnesses become the next battleground as teams move from coding assistance to governed execution fabrics. Ephemeral environments and infra-plan evidence gain importance. [obs: en-019d950e-f685-7f63-b850-1f8ecf8e750a] [obs: en-019d9510-4fb2-78a1-869c-bff80fae7719] [obs: en-019d950e-5071-79f2-a211-7e9a9d09d1d6]
- Expected signals: more read-only infra introspection, repo-specific eval harnesses, and explicit rollback contracts across code plus policy bundles. [obs: en-019d950e-5486-73e1-a90c-d9fbb8da2adc] [obs: en-019d950f-eaae-7482-9d3b-cc9acf04564c] [obs: en-019d950f-eab7-7f51-b41d-20026baebb6d]
- **Revisions to earlier predictions:** Phase 1 expectations of 20-30% gains are confirmed only for well-tested services; for regulated and multi-service systems they are qualified downward because policy exceptions, audit capture, and review queues consume part of the gain. Predictions of easy governance scaling are revised: Cedar and OPA help, but only with explainability and ownership. [obs: en-019d950e-f67b-7232-8e86-7dc89477d553] [obs: en-019d950e-5082-7293-b993-32e10e648678] [obs: en-019d950e-f6b3-7f81-b9a5-e8727f13568e]
- Causal link to Phase 3: once execution fabrics exist, firms begin differentiating on routing, auditability, and organizational reuse rather than on a single tool choice. [obs: en-019d950e-7097-7f40-990e-38d0e845c3b7] [obs: en-019d950e-f6f2-77c1-b7b6-5695d5b6c4e8]

#### Phase 3: 6-9 Months (days 180-270)
- OpenHands, LiteLLM-style routing, Anthropic, OpenAI, and platform-team templates create portfolio architectures that preserve bargaining power and task-level specialization. Organizations start routing across premium and cheaper models by workload class. [obs: en-019d950e-f6e1-7d03-beb6-50022653533e] [obs: en-019d9510-5025-7f73-b27b-f2857d3ba9b5] [obs: en-019d950e-f68e-7040-9299-2b3a6e474e90]
- Expected signals: explicit routing layers, per-task cost caps, common policy templates, and role redesign that shifts staff engineers toward exception handling and system selection. [obs: en-019d950e-f6f2-77c1-b7b6-5695d5b6c4e8] [obs: en-019d9510-502e-71e3-bedf-f0b7b37f5a45] [obs: en-019d950e-70a0-7ed3-9670-6367b364635a]
- **Revisions to earlier predictions:** The early thesis that repo-native pipelines win is confirmed, but it needs qualification: victory comes less from CI alone than from portfolio routing and durable coordination layers. The prediction that frontier models dominate is revised downward because platform maturity and incentive design explain more variance than vendor access alone. [obs: en-019d950e-f69e-79c1-be23-c94668a3f6d4] [obs: en-019d950e-f6fa-7210-a2e6-788fe203c29d] [obs: en-019d950e-70a0-7ed3-9670-6367b364635a]
- Causal link to Phase 4: as routing and coordination mature, the market stratifies around who can operationalize evidence, rollback, and standards at scale. [obs: en-019d9510-4fbb-7121-92d8-3bce61df5ce3] [obs: en-019d9510-501c-77c1-9866-d31f4593e64b]

#### Phase 4: 9-12 Months (days 270-365)
- Sentinel, Temper, simulators, black-box recording, and tested rollback contracts become markers of serious programs. The strongest firms run agent operations more like airline operations or SRE-managed production systems than like informal developer tooling. [obs: en-019d9510-5014-7132-9263-7b3c4536320f] [obs: en-019d9510-4fbb-7121-92d8-3bce61df5ce3] [obs: en-019d950f-eab7-7f51-b41d-20026baebb6d]
- Expected signals: black-box style action logging, policy test suites, audited exception workflows, and explicit throughput splits between platform-mature and fragmented organizations by Q2 2027. [obs: en-019d9510-4fe2-7891-9dda-c742b58ddab6] [obs: en-019d9510-501c-77c1-9866-d31f4593e64b] [obs: en-019d950f-eac0-7bb3-815d-2ac15f4e175e]
- **Revisions to earlier predictions:** The expectation of broad autonomy is falsified; the expectation of governed, selective autonomy is confirmed. Predictions of uniform productivity uplift are revised into a two-tier market. Predictions that policy engines alone would solve safety are falsified; policy plus evaluation plus rollback becomes the validated combination. [obs: en-019d950e-508a-7cb1-8683-aae7c984a692] [obs: en-019d9510-501c-77c1-9866-d31f4593e64b] [obs: en-019d950e-5079-7261-9b83-0ee096679246]
- **Final state assessment:** By day 365, directed software evolution is real but uneven: leading firms operate policy-gated, auditable agent fabrics with portfolio routing and reusable standards, while laggards still experience agent work as review-queue inflation and governance confusion. [obs: en-019d9510-5038-7761-a6b1-3e9a5f431cc0] [obs: en-019d9510-5006-71b0-8d25-e335625fe962] [obs: en-019d9510-4fd5-7121-9577-52da277c0aac]

### Active Directions

#### Repository-native, policy-gated agent workflows will outpace fully autonomous software agents over the next 90 days
**Direction ID:** en-019d950e-54a8-7292-922b-0641c2626f1c

In the next 90 days, directed software evolution will advance mainly through repository-native orchestration, not through standalone autonomous engineers replacing the software lifecycle. The feasible architecture is already visible: tools like Cursor, Aider, and Cline operate close to the codebase; CI systems provide deterministic evaluation; GitHub and GitLab remain the approval boundary; and infrastructure changes flow through Terraform plans, Kubernetes manifests, and policy checks rather than opaque terminal sessions. Practitioners can deploy this now because it fits existing controls: branch protection, CODEOWNERS, test suites, artifact attestations, and human review all remain intact while agents accelerate drafting, refactoring, and scoped troubleshooting.

The strongest near-term pattern is a layered control plane. Models generate plans and code changes, repo integrations gather context, CI executes tests and static analysis, and policy systems such as OPA or Cedar determine whether a change is approvable. Autonomous agents like Devin will win only in bounded workflows where tasks have clear acceptance criteria, sandboxed execution, and explicit retry budgets. The bottleneck is less raw model capability than the maturity of the surrounding software delivery system. Teams with clean repos, reliable tests, devcontainers, preview environments, and infrastructure-as-code will see compounding gains quickly; teams without that scaffolding will plateau. Therefore the winning strategy for practitioners is to invest in agent-ready repositories and governed execution paths rather than betting on full autonomy as a drop-in replacement for engineering process.

Supporting observations: 

**Counterfactual:** This direction is wrong if fully autonomous agents begin reliably completing ambiguous multi-week product and infrastructure work in production repositories with minimal human review, and if organizations accept direct execution without repo-native CI, policy, and approval controls.

#### Governed autonomy will bottleneck on rollback and evidence, not raw model capability
**Direction ID:** en-019d950e-5091-7733-a6fe-6b6c142883a4

Over the next 90 days, the limiting factor in directed software evolution is unlikely to be whether Anthropic, OpenAI, Cursor, or Devin can generate acceptable code. It is whether organizations can prove that an agent action was authorized, evaluated in context, safely reversible, and attributable after something goes wrong. Cedar, OPA, and Sentinel meaningfully improve permissioning and policy consistency, but they do not automatically produce the end-to-end evidence chain needed by platform, security, and SRE teams. As agent activity expands from code suggestion into pull requests, CI orchestration, and infrastructure change proposals, missing provenance becomes the decisive operational risk.

This creates a predictable near-term pattern: enterprises continue pilots, but hard production autonomy stays narrow and highly staged. Teams will allow agents to operate in low-blast-radius domains while keeping high-risk Kubernetes, Terraform, secret-management, and deployment actions behind explicit approvals or environment-scoped gates. The field may look as though it is advancing quickly because demo quality and coding throughput improve, yet real organizational adoption will be constrained by exception handling, rollback discipline, and auditability requirements. If this thesis is wrong, we should see broad enforcement-mode policy gates paired with faster incident attribution and fewer manual approval steps by the end of the quarter.

Supporting observations: 

**Counterfactual:** If organizations rapidly solve provenance, rollback, and evidence capture, governance will cease to be the main brake and the adoption curve for higher-autonomy coding and infrastructure agents could steepen much faster than this projection expects.

#### Governed agent control planes, not raw autonomy, will be the first durable source of adoption in directed software evolution.
**Direction ID:** en-019d950e-70b1-7c73-8246-4cacc2ee39fd

Across economics, organizational theory, and platform history, the pattern is consistent: adoption accelerates when a technology lowers not just unit cost but coordination cost. Over the next 90 days, directed software evolution will therefore advance first in organizations that build a managed control plane around agents—policy gates, auditable actions, bounded task classes, and explicit human ownership. This is why platforms that combine orchestration with governance, such as Temper paired with Cedar- or OPA-like controls, are positioned better for durable enterprise adoption than raw coding copilots alone. The relevant analogy is not consumer SaaS virality but the rise of cloud infrastructure primitives: enterprises scaled Kubernetes and Terraform once they became governable inside existing operating models, not merely because the underlying compute was powerful.

The main strategic implication is that the market will briefly look slower than frontier-model enthusiasts expect, while actually becoming structurally deeper. Teams using Cursor, Aider, Cline, Devin-like agents, Anthropic, OpenAI, or Google coding models will generate plenty of local productivity anecdotes, but the winners will be firms that redesign review queues, incident accountability, and evaluation scaffolding so agents can operate repeatedly with low marginal supervision. In other words, advantage comes from institutionalizing trust. If this thesis is wrong, we should instead see raw model gains overwhelm these frictions and broad end-to-end autonomous feature delivery spread without major changes to governance or team design; that is possible, but adjacent-domain evidence from finance, healthcare, and operations suggests bottlenecks migrate toward coordination long before they disappear.

Supporting observations: 

**Counterfactual:** If enterprises adopt high-autonomy coding agents broadly without first investing in policy, auditability, evaluation scaffolds, and ownership redesign, then coordination costs are not the primary bottleneck and this direction is overstating the importance of governance as a platform moat.

#### Directed software evolution consolidates around evidence-rich CI pipelines rather than fully autonomous IDE agents.
**Direction ID:** en-019d950e-f69e-79c1-be23-c94668a3f6d4

The next 90 days favor architectures that fit existing delivery systems instead of replacing them. Cursor, GitHub Actions, Kubernetes, and Terraform already sit on the path to production, so teams can incrementally insert agents into pull-request creation, test remediation, migration work, and environment preparation without rewriting their control model. This keeps adoption grounded in evidence artifacts such as test output, infra plans, and preview deployments rather than in vague claims of autonomy.

The practical implication is that orchestration and validation layers matter more than a single model or coding assistant. Teams that combine premium interactive agents with cheaper task-specific agents will see better economics and more robust execution, because the pipeline can route work to the right tool while preserving human checkpointing. The near-term winner is the organization that makes autonomous work legible inside CI, not the one that gives an agent the widest permissions.

Supporting observations: 

**Counterfactual:** If end-to-end autonomous IDE agents become trustworthy faster than validation pipelines mature, this thesis underestimates how much control can move out of CI and back into interactive agent shells.

#### Evaluation debt, not model capability, is the bottleneck that will slow directed software evolution in 2026.
**Direction ID:** en-019d950e-f6cc-73d1-b816-5d6044fbd5d2

Coding agents are improving faster than enterprise verification systems. Anthropic, OpenAI, and Google can all produce credible code changes, but enterprise repositories contain hidden assumptions, partial ownership, and inconsistent tests that make correctness far harder to verify than to generate. In that environment, every unit of new autonomy creates a larger requirement for eval harnesses, rollback design, and policy observability.

That means governance tools such as Cedar, OPA, and Sentinel are necessary but insufficient. If they are deployed as opaque blockers, engineers will work around them; if they are deployed with debug visibility and risk-tiering, they can become selective pressure rather than friction. The key risk is that leadership mistakes benchmark progress for operational readiness and scales faster than its verification substrate can support.

Supporting observations: 

**Counterfactual:** If benchmark gains transfer cleanly into messy codebases and automated evals mature unusually quickly, evaluation debt may prove less constraining than forecast here.

#### The firms that benefit most will redesign organizational interfaces and vendor portfolios before chasing maximum agent autonomy.
**Direction ID:** en-019d950e-f6fa-7210-a2e6-788fe203c29d

From economics and organizational theory, the winning pattern is not simply more capable agents; it is lower coordination cost per unit of delegated work. Enterprises that define bounded scopes, interface contracts, audit trails, and portfolio model routing can treat agents like modular labor inputs rather than magical black boxes. That allows them to arbitrage across Anthropic, OpenAI, and open-source options while preserving bargaining power.

This also explains why platform-centric organizations will compound faster. They can spread common policy templates, test harnesses, and deployment patterns across many repos, making each incremental improvement reusable. The result is a widening gap between firms with coherent internal platforms and firms with fragmented local practices, even when they buy access to the same frontier models.

Supporting observations: 

**Counterfactual:** If model capability alone dominates coordination costs, then org design and vendor portfolio choices will matter less than anticipated.

#### The winning one-year architecture is a policy-gated repo execution fabric built from Cursor, GitHub Actions, Kubernetes, and Terraform rather than a standalone coding assistant.
**Direction ID:** en-019d950f-f402-7971-a695-148fb4aacc60

The strongest near-term thesis is that directed software evolution will be operationalized as a governed repo execution fabric, not as standalone coding copilots. In practice, teams will combine Cursor-style developer interfaces with GitHub Actions orchestration, Kubernetes-backed ephemeral execution, and Terraform-defined environment scaffolding so every agent proposal can be reproduced, policy-checked, and rolled back. The technical reason is straightforward: code generation quality is improving faster than organizational trust, so adoption concentrates where execution traces, approvals, and environment boundaries are already available.

By day 365, the durable architecture is a layered loop: local agent proposes a change, repository controller decomposes and routes work, CI runs repo-specific evals, Kubernetes supplies isolated runtime verification, Terraform provisions or mutates disposable infrastructure safely, and policy engines such as Cedar, OPA, or Sentinel decide what can advance. Teams that build this loop get compounding throughput because they can let agents touch more of the stack without giving them unconstrained authority. Teams that skip the control layer will keep agents trapped in low-risk suggestion mode or experience enough failures to slow deployment.

Supporting observations: 

**Counterfactual:** If a standalone coding assistant can reliably coordinate cross-repo changes, environment setup, validation, and rollback without an explicit repo control plane, then the predicted shift toward governed execution fabrics will be overstated.

#### Governed rollback, not code generation, becomes the true bottleneck and moat for directed software evolution by day 365
**Direction ID:** en-019d950f-ead0-7ec1-9c55-2ed4e77ad2f6

The stronger thesis at day 365 is that directed software evolution does not fail because agents cannot write enough code; it stalls because governance and recovery systems do not co-evolve at the same pace. Enterprises now have abundant generation capacity, decent CI integration, and increasingly standardized agent workflows, but they still lack reliable ways to reason about policy changes, evaluate socio-technical failure paths, and reverse multi-system actions cleanly. In practice, this turns the control plane into the limiting reagent for autonomous development.

Cedar, OPA, and Sentinel all become more important over the year, yet their broader adoption reveals the same structural weakness: policy power outruns policy usability. The most effective organizations are not those with the most autonomous agents, but those that constrain autonomy to surfaces they can audit, test, and roll back end-to-end. That means the next competitive layer is governed change management with explicit rollback contracts and evaluation of policy-workflow interactions. Teams that ignore this will report impressive local productivity while accumulating hidden operational fragility.

Supporting observations: 

**Counterfactual:** If policy tooling becomes broadly understandable and organizations achieve atomic rollback across code, policy, and workflow state, then autonomous software evolution could resume compounding primarily on generation quality rather than governance capacity.

#### By day 365, the winners in directed software evolution are the organizations and vendors that own the selection environment, not the ones that merely generate the most code.
**Direction ID:** en-019d950f-f623-7081-bf86-408c423f3b79

Across economics, biology, and organizational theory, the strongest year-one pattern is that directed software evolution is not converging into a pure model market; it is converging into a governed selection-system market. Once coding agents become broadly substitutable, rents move to the institutions that can repeatedly select, validate, and deploy useful mutations. In practice, that means repo-specific evals, CI telemetry, policy layers such as Cedar or OPA, incident feedback loops, and platform teams that turn local learning into organizational standards. The critical asset is not code generation alone but the selection environment that determines which generated changes survive.

This makes the sector look less like traditional developer tooling and more like a hybrid of quantitative finance infrastructure, industrial quality systems, and managed cybersecurity. Firms that own the approval rails and evidence trails will shape both vendor capture and internal org charts. Vendors that package governance, auditability, and adaptation into the workflow will defend pricing better than those selling autonomous coding as a standalone feature. Enterprises that redesign engineering around supervisory control will compound gains; those that merely add agents to existing teams will see a temporary burst of throughput followed by coordination debt and trust erosion.

Supporting observations: 

**Counterfactual:** If foundation models remain strongly differentiated and verification becomes cheap and generic, selection systems will matter less than expected and value will stay concentrated in model providers or point-solution coding agents.

#### By one year, durable agent orchestration and evidence capture become the core technical moat in directed software evolution.
**Direction ID:** en-019d9510-4fd5-7121-9577-52da277c0aac

The market will keep celebrating model releases, but the compounding technical advantage will come from orchestration fabrics that can schedule, isolate, audit, and reproduce semi-autonomous work. Cursor, Cline, GitHub Actions, Kubernetes runners, and Temper-like coordination systems fit together into a pipeline where every agent action has bounded permissions and machine-readable evidence. That is what allows organizations to move beyond demos and toward repeatable production use.

This changes the basis of competition. The question is no longer which interface feels smartest in isolation; it is which stack can reliably route work, attach proof, and recover from failure across hundreds of repos. Enterprises that invest in durable coordination layers will capture the next wave of returns because they can swap models underneath while keeping the operational substrate stable.

Supporting observations: 

**Counterfactual:** If model vendors vertically integrate orchestration, evaluation, and controls into trusted end-to-end products faster than expected, independent coordination layers may matter less.

#### Governance and evaluation complexity compound faster than autonomy gains, creating a hidden ceiling on agent adoption.
**Direction ID:** en-019d9510-5006-71b0-8d25-e335625fe962

One year in, more organizations will have live agent workflows, but many will also discover that policy drift, incomplete eval coverage, and socio-technical failure modes expand with every additional automation pathway. Cedar, OPA, and Sentinel can express controls, yet those controls become liabilities if they are not versioned, tested, and owned as rigorously as production services. The hidden problem is not whether agents can code, but whether the organization can maintain trust in the pathways those agents use.

That dynamic creates a ceiling on adoption. Leadership can buy more capability from vendors, but it cannot buy away the need for ownership maps, rollback routines, evidence retention, and cross-functional governance. The organizations that ignore this will misread local wins as proof of scalable readiness and then stall when the control surface becomes too complex to reason about.

Supporting observations: 

**Counterfactual:** If policy testing, verification, and socio-technical evals become much easier than expected, the projected governance ceiling may be too pessimistic.

#### Directed software evolution becomes a platform-and-operations advantage before it becomes a universal developer productivity feature.
**Direction ID:** en-019d9510-5038-7761-a6b1-3e9a5f431cc0

Across economics and organizational design, the one-year pattern is divergence rather than uniform uplift. Platform-mature firms can turn agents into reusable operational assets through routing layers, templates, simulators, checklists, and role redesign, capturing much larger gains than fragmented peers. That mirrors earlier infrastructure transitions in which organizational standardization determined who benefited from a common technology wave.

The strategic result is a two-tier market. The best firms will combine Anthropic or OpenAI frontier capability with open infrastructure such as OpenHands and routing layers to preserve leverage and learn quickly. Others will buy similar model access yet see only modest gains because they lack the operational discipline required to turn agent output into reliable throughput.

Supporting observations: 

**Counterfactual:** If low-friction turnkey platforms erase operational differences between firms, the forecasted divergence between platform-mature and fragmented organizations may narrow.


### What Surprised Us

- **The dominant narrative that agentic development will deliver 20-40% throughput gains within one planning quarter is overstated for regulated or multi-service environments.** [obs: en-019d950e-5082-7293-b993-32e10e648678]
  Why surprising: It challenges the assumption that model improvement translates directly into net organizational productivity.
- **A challenge to the dominant narrative: more autonomy will not immediately reward the firms with the best frontier models; it will reward the firms with the best internal incentive design.** [obs: en-019d950e-70a0-7ed3-9670-6367b364635a]
  Why surprising: It shifts advantage from model vendors to management systems, ownership, and rollback budgets.
- **Rollback design emerges as a hard unsolved layer rather than an implementation detail.** [obs: en-019d950f-eab7-7f51-b41d-20026baebb6d]
  Why surprising: It shows that reverting code is easier than reverting policy and orchestration side effects.
- **A two-tier market emerges: platform-mature enterprises capture 30%+ throughput gains while fragmented organizations remain stuck near 5-10%.** [obs: en-019d9510-501c-77c1-9866-d31f4593e64b]
  Why surprising: It challenges the assumption that access to the same models yields roughly similar performance gains.


### Top 5 Predictions with Falsification Criteria

1. **Prediction:** By 2027-01-31, Cursor-, Cline-, and GitHub-Actions-based repo fabrics will be the dominant production architecture for agent coding in large software teams.
   - **Measurable indicator:** more than 60% of production agent workflows terminate in pull requests plus CI evidence rather than direct execution
   - **Confidence:** high
   - **Falsification:** If fewer than 30% of enterprise agent rollouts still rely on PR plus CI gating as the primary approval boundary has not occurred by 2027-01-31, this prediction is wrong because it would show that standalone autonomous agents displaced repository-native governance faster than projected
   - **Supporting observations:** [obs: en-019d950e-547d-7b20-9350-496f9f7cdda3], [obs: en-019d950e-f67b-7232-8e86-7dc89477d553], [obs: en-019d950f-f3d7-7511-8511-86317b174fbd]

2. **Prediction:** By 2027-03-31, Cedar, OPA, and Sentinel programs that do not add policy tests and debug visibility will become rollout bottlenecks.
   - **Measurable indicator:** policy exception or false-block rate exceeds 5% of attempted high-risk agent changes
   - **Confidence:** high
   - **Falsification:** If policy stacks remain broadly legible to application teams without dedicated test suites or explainability layers has not occurred by 2027-03-31, this prediction is wrong because it would mean governance complexity scaled more cleanly than the observations imply
   - **Supporting observations:** [obs: en-019d950e-f6b3-7f81-b9a5-e8727f13568e], [obs: en-019d950f-eaa5-73a3-ad23-7dc853b6e7c5], [obs: en-019d9510-4fe2-7891-9dda-c742b58ddab6]

3. **Prediction:** By 2027-04-15, platform-mature enterprises will realize 30%+ throughput gains while fragmented organizations remain near 5-10%.
   - **Measurable indicator:** at least a 20-point throughput gap between standardized and fragmented org cohorts
   - **Confidence:** medium
   - **Falsification:** If fragmented organizations achieve similar gains without common templates, routing layers, or platform standards has not occurred by 2027-04-15, this prediction is wrong because it would show that operational maturity is less decisive than assumed
   - **Supporting observations:** [obs: en-019d950e-f6f2-77c1-b7b6-5695d5b6c4e8], [obs: en-019d9510-501c-77c1-9866-d31f4593e64b], [obs: en-019d950e-70a0-7ed3-9670-6367b364635a]

4. **Prediction:** By 2027-02-28, vendor portfolio routing across Anthropic, OpenAI, and open infrastructure such as OpenHands or LiteLLM will be standard in advanced teams.
   - **Measurable indicator:** at least 2 frontier providers plus 1 open routing layer used for production task allocation
   - **Confidence:** medium
   - **Falsification:** If single-vendor stacks remain dominant even in high-maturity teams with meaningful agent spend has not occurred by 2027-02-28, this prediction is wrong because it would mean switching costs and procurement convenience outweighed bargaining-power concerns
   - **Supporting observations:** [obs: en-019d950e-f6e1-7d03-beb6-50022653533e], [obs: en-019d9510-5025-7f73-b27b-f2857d3ba9b5], [obs: en-019d950f-4ff4-7b12-a675-c1b6e6069645]

5. **Prediction:** By 2027-04-15, rollback and evidence capture will be treated as first-class product requirements for agent platforms such as Temper and repo automation stacks.
   - **Measurable indicator:** 100% of high-risk automated changes require replayable provenance plus a tested rollback contract
   - **Confidence:** high
   - **Falsification:** If teams expand agent write access without adding end-to-end provenance and rollback contracts has not occurred by 2027-04-15, this prediction is wrong because it would show that organizations tolerated hidden operational risk longer than projected
   - **Supporting observations:** [obs: en-019d950e-5071-79f2-a211-7e9a9d09d1d6], [obs: en-019d950f-eab7-7f51-b41d-20026baebb6d], [obs: en-019d9510-4fbb-7121-92d8-3bce61df5ce3]


### Decision Points

#### Decision Point 1
- **Decision:** Whether to deploy Cedar or OPA policy-as-code gates as hard CI blockers for agent-authored pull requests on GitHub Actions. [obs: en-019d950e-f6b3-7f81-b9a5-e8727f13568e] [obs: en-019d950f-eaa5-73a3-ad23-7dc853b6e7c5]
- **Timing trigger:** When agent-authored pull requests exceed 15% of weekly merge volume or by September 2026, whichever comes first. [obs: en-019d950e-f67b-7232-8e86-7dc89477d553]
- **Option A:** Deploy Cedar policy gates on GitHub Actions with explain logs and repo risk tiers — **Tradeoff:** 3-5 engineering-weeks plus ongoing policy ownership by platform engineering.
- **Option B:** Deploy OPA/Rego checks in advisory mode first, then harden only high-risk repos — **Tradeoff:** 2-4 engineering-weeks, but creates a longer period of inconsistent enforcement.
- **Option C:** Keep manual review only and postpone policy engine rollout — **Tradeoff:** lowest short-term effort, but review queues and hidden policy drift continue; expect 1-2 senior reviewers to become bottlenecks.
- **Recommended:** Option B, because it preserves developer adoption while building the observability needed before hard enforcement.

#### Decision Point 2
- **Decision:** Whether to build a Kubernetes-backed ephemeral execution fabric for agent tasks or keep agents confined to local IDE workflows. [obs: en-019d950e-f685-7f63-b850-1f8ecf8e750a] [obs: en-019d9510-4fb2-78a1-869c-bff80fae7719]
- **Timing trigger:** When more than 5 repos require repeatable environment setup for agent tasks or by Q1 2027. [obs: en-019d950e-5486-73e1-a90c-d9fbb8da2adc]
- **Option A:** Build Kubernetes ephemeral runners plus Terraform preview environments and artifact retention — **Tradeoff:** 6-10 engineering-weeks and likely requires a dedicated platform team.
- **Option B:** Use GitHub-hosted runners plus disposable branch environments only for approved repos — **Tradeoff:** 2-4 engineering-weeks, but weaker isolation and less reproducibility for infra-heavy tasks.
- **Option C:** Keep Cursor/Cline execution local inside developer workstations — **Tradeoff:** near-zero platform cost, but poor auditability, weak reuse, and slower scale-out.
- **Recommended:** Option A, because durable execution isolation is the clearest path to auditable, reusable agent operations.

#### Decision Point 3
- **Decision:** Whether to preserve vendor optionality with Anthropic/OpenAI routing plus OpenHands or LiteLLM, or standardize on a single integrated vendor stack. [obs: en-019d950e-f6e1-7d03-beb6-50022653533e] [obs: en-019d9510-5025-7f73-b27b-f2857d3ba9b5]
- **Timing trigger:** When annual agent-model spend is projected above $250K or when procurement requests a multi-year commitment, likely by Q4 2026. [obs: en-019d950e-f6c4-72f2-a0d7-d7e90d0d8c33]
- **Option A:** Deploy LiteLLM-style routing with Anthropic and OpenAI backends plus OpenHands for open orchestration — **Tradeoff:** 4-6 engineering-weeks and ongoing model-eval maintenance.
- **Option B:** Standardize on one premium vendor such as Anthropic or OpenAI with native tooling — **Tradeoff:** fastest procurement path, but higher lock-in risk and lower bargaining power.
- **Option C:** Push heavily toward self-hosted open-source models and wrappers — **Tradeoff:** 8-12 engineering-weeks plus significant infra cost and weaker frontier capability in the near term.
- **Recommended:** Option A, because it balances frontier quality with bargaining power and operational learning.


### Assumptions & Limitations

1. **Assumption:** Repo-specific eval harnesses improve, but not fast enough to erase evaluation debt. [obs: en-019d950e-f6aa-7d12-bf70-8ea0b70dccef]
   - **If wrong:** Higher-autonomy execution could spread faster and the forecast would understate integrated-vendor upside.
   - **Confidence:** medium

2. **Assumption:** Organizational maturity and platform standardization explain a large share of throughput variance. [obs: en-019d950e-f6f2-77c1-b7b6-5695d5b6c4e8] [obs: en-019d9510-501c-77c1-9866-d31f4593e64b]
   - **If wrong:** Model capability and vendor integration would matter more than routing, templates, and role redesign.
   - **Confidence:** high

3. **Assumption:** Policy engines remain necessary but insufficient without rollback, observability, and explainability. [obs: en-019d950e-5079-7261-9b83-0ee096679246] [obs: en-019d950f-eac0-7bb3-815d-2ac15f4e175e]
   - **If wrong:** Organizations could safely scale autonomy with simpler control stacks than projected here.
   - **Confidence:** high

### Methodology
- 3 independent probes per step
- 2 time steps over 1 year
- 54 total observations, 12 active directions
- Observation IDs: ['en-019d950e-5069-79c2-a283-1f0abdd007f3', 'en-019d950e-5071-79f2-a211-7e9a9d09d1d6', 'en-019d950e-5079-7261-9b83-0ee096679246', 'en-019d950e-5082-7293-b993-32e10e648678', 'en-019d950e-508a-7cb1-8683-aae7c984a692', 'en-019d950e-547d-7b20-9350-496f9f7cdda3', 'en-019d950e-5486-73e1-a90c-d9fbb8da2adc', 'en-019d950e-548f-7b71-bed8-5ae971bd1e6b', 'en-019d950e-5497-7721-a210-303a0efea7b4', 'en-019d950e-54a0-7f00-8d0a-861d7292ef4d', 'en-019d950e-7084-75d1-b22b-54032dd29c40', 'en-019d950e-708e-7a43-bb5a-4a65765e4998', 'en-019d950e-7097-7f40-990e-38d0e845c3b7', 'en-019d950e-70a0-7ed3-9670-6367b364635a', 'en-019d950e-70a8-7bd1-93df-9f8d9a57f1bf', 'en-019d950e-f67b-7232-8e86-7dc89477d553', 'en-019d950e-f685-7f63-b850-1f8ecf8e750a', 'en-019d950e-f68e-7040-9299-2b3a6e474e90', 'en-019d950e-f696-7703-bf0d-9451b3c26d5f', 'en-019d950e-f6aa-7d12-bf70-8ea0b70dccef', 'en-019d950e-f6b3-7f81-b9a5-e8727f13568e', 'en-019d950e-f6bc-7a23-aa9a-031f7c445133', 'en-019d950e-f6c4-72f2-a0d7-d7e90d0d8c33', 'en-019d950e-f6d8-7cd3-9669-ce542a641bef', 'en-019d950e-f6e1-7d03-beb6-50022653533e', 'en-019d950e-f6ea-7170-88be-9f57ed14840a', 'en-019d950e-f6f2-77c1-b7b6-5695d5b6c4e8', 'en-019d950f-eaa5-73a3-ad23-7dc853b6e7c5', 'en-019d950f-eaae-7482-9d3b-cc9acf04564c', 'en-019d950f-eab7-7f51-b41d-20026baebb6d', 'en-019d950f-eac0-7bb3-815d-2ac15f4e175e', 'en-019d950f-eac7-7731-b224-324196867fa2', 'en-019d950f-f3d7-7511-8511-86317b174fbd', 'en-019d950f-f3e1-7803-888b-d6b8f616e7c8', 'en-019d950f-f3ea-7870-bcb7-d0cdbaaa3040', 'en-019d950f-f3f2-7fa3-9eb2-510215a1d08c', 'en-019d950f-f3fa-7a43-9533-dde7566e1e22', 'en-019d950f-f5f6-7463-8731-ff2059065746', 'en-019d950f-f600-7a22-b1de-3b61439538f0', 'en-019d950f-f609-7273-a47b-082ea3e8ff3c', 'en-019d950f-f612-7380-a5a6-1f8c7e6e4188', 'en-019d950f-f61a-7042-983e-8dc55cf90618', 'en-019d9510-4fb2-78a1-869c-bff80fae7719', 'en-019d9510-4fbb-7121-92d8-3bce61df5ce3', 'en-019d9510-4fc5-7d50-ab52-559f1f446169', 'en-019d9510-4fcd-7f53-946f-e58a712ae7e6', 'en-019d9510-4fe2-7891-9dda-c742b58ddab6', 'en-019d9510-4feb-73e0-bd77-20d4d96e72a1', 'en-019d9510-4ff4-7b12-a675-c1b6e6069645', 'en-019d9510-4ffc-7580-9995-93647bc6f04b', 'en-019d9510-5014-7132-9263-7b3c4536320f', 'en-019d9510-501c-77c1-9866-d31f4593e64b', 'en-019d9510-5025-7f73-b27b-f2857d3ba9b5', 'en-019d9510-502e-71e3-bedf-f0b7b37f5a45']

