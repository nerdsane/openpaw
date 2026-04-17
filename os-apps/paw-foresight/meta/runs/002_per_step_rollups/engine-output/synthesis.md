# Foresight Projection: Directed Software Evolution v2
## Horizon: 1 year | Steps: 2

### Executive Summary
Directed software evolution is moving from demos to governed production cells, but the winning pattern is narrower than the original dark-factory story. Across OpenAI Codex, Anthropic Claude Code, Cursor, GitHub Actions, Kubernetes, Terraform, Cedar, OPA, and Temper-style entity/action systems, the credible near-term architecture is a bounded agent frontend paired with deterministic acceptance machinery and administrative policy gates [obs: en-019d98cc-232c-7610-8005-b29e28078e9c] [obs: en-019d98cc-2335-7ac3-94af-f9c4be261d83] [obs: en-019d98d0-5b48-7ad1-b276-00b32f57a3d3]. The quant signal is not abstract: Codex tasks are framed in 1-30 minute isolated environments, GitHub policy enablement names Anthropic and OpenAI explicitly, and governance now lives in platform defaults as much as in standalone policy engines [obs: en-019d98cf-f875-7d91-9c33-8bad5b518049] [obs: en-019d98d0-5b48-7ad1-b276-00b32f57a3d3].

The core bottleneck also clarified over the year. SWE-bench Verified still relies on a human-validated subset of 500 instances, which means evaluation scarcity remains a structural constraint even as OpenHands advertises scale to 1000s of agents and Cursor monetizes more background work [obs: en-019d98cc-430c-7eb1-b421-3eddf26b0b85] [obs: en-019d98d0-11a7-72a3-baee-8bdb3b18aad0] [obs: en-019d98d0-5b5b-7c62-b2c2-9391585ccaf6]. That matters because Anthropic, OpenAI, Devin, OpenHands, and Cursor are not constrained mainly by patch generation anymore; they are constrained by acceptance surfaces, environment packaging, provenance, and organizational review bandwidth [obs: en-019d98cf-f892-74a3-b9cb-32afbc08e758] [obs: en-019d98d0-11b1-7c72-be6e-e0295998fd71].

Economically, the category is becoming a governed workflow market rather than a single-model market. Cursor's $20/$60/$200 pricing ladder, OpenHands' workflow packaging, GitHub's admin-controlled agent enablement, and OPA/Cedar's mixed-stack role all point to supervisory capacity, policy integration, and rollback legibility as the scarce assets [obs: en-019d98d0-11ba-7a00-b60f-0d7a88940af0] [obs: en-019d98cf-f87f-7cf0-961c-8577b2c5c45d] [obs: en-019d98cf-f889-7930-85b3-576ed62d6562] [obs: en-019d98d0-11c4-78f3-9115-f0300472d2d5]. The most important revision from the projection is that policy does not centralize cleanly into Cedar or OPA alone; instead, mixed control planes become the enterprise default, which changes where durable advantage accrues [obs: en-019d98cf-f889-7930-85b3-576ed62d6562].

### Key Findings
1. **OpenAI Codex is standardizing the bounded cloud workcell as the dominant agent form factor.**
   - Evidence: "each task runs in its own isolated environment... usually takes 1-30 minutes" [obs: en-019d98cf-f875-7d91-9c33-8bad5b518049]
   - Measurable indicator: 1-30 minute task window in isolated environments.
   - Theme: technical architecture
2. **GitHub is turning Anthropic Claude and OpenAI Codex access into an administrator-governed platform setting rather than a developer-local choice.**
   - Evidence: "only when the relevant Anthropic/OpenAI policy is enabled by an administrator" [obs: en-019d98d0-5b48-7ad1-b276-00b32f57a3d3]
   - Measurable indicator: 2 vendor policy toggles named directly: Anthropic and OpenAI.
   - Theme: governance/policy
3. **Cursor is exposing the economic ceiling of always-on coding agents faster than benchmark rhetoric suggests.**
   - Evidence: "Cursor's public pricing shows a ladder from Pro at $20 per month to Pro+ at $60 per month and Ultra at $200 per month" [obs: en-019d98d0-11ba-7a00-b60f-0d7a88940af0]
   - Measurable indicator: $20, $60, and $200 monthly tiers.
   - Theme: economics/market
4. **SWE-bench Verified remains the legitimacy bottleneck for Anthropic, OpenAI, Devin, OpenHands, and Cursor-style agents.**
   - Evidence: "human-validated subset of only 500 instances" [obs: en-019d98d0-11a7-72a3-baee-8bdb3b18aad0]
   - Measurable indicator: 500 verified instances.
   - Theme: evaluation/testing
5. **OpenHands is positioning model-agnostic task factories, not raw autonomy, as the enterprise buying unit.**
   - Evidence: "fixed workflows such as vulnerability remediation, PR review, migration, and incident triage" [obs: en-019d98cf-f87f-7cf0-961c-8577b2c5c45d]
   - Measurable indicator: 4 workflow categories named directly.
   - Theme: organizational/adoption
6. **Industrial-automation analogies explain the market better than artisan-programmer analogies.**
   - Evidence: "looks less like autonomous evolution and more like industrial automation cells with guarded machine envelopes" [obs: en-019d98cf-f875-7d91-9c33-8bad5b518049]
   - Measurable indicator: 3 concrete control elements named: isolation, quality inspection, and guarded envelopes.
   - Theme: cross-domain
7. **Cedar and OPA are important, but mixed control stacks are beating single-engine convergence.**
   - Evidence: "mixed stacks are winning because the operational boundary is fragmented" [obs: en-019d98d0-11c4-78f3-9115-f0300472d2d5]
   - Measurable indicator: at least 5 active layers named: repo permissions, CI, Kubernetes, Terraform, and agent-local permissions.
   - Theme: governance/policy
8. **Managerial decomposition and approval bandwidth are becoming a harder limit than model quality alone.**
   - Evidence: "the organizational bottleneck after one year is no longer just benchmark verification; it is managerial decomposition and approval bandwidth" [obs: en-019d98cf-f892-74a3-b9cb-32afbc08e758]
   - Measurable indicator: Cursor timeline cites multi-agent collaboration in 2024 and semantic search in 2026, showing a multi-year shift toward supervised fleets.
   - Theme: organizational/adoption


### Temporal Progression
Temporal Progression below is assembled from the step rollups. The most consequential revision occurred when Step 1 overturned the implicit Step 0 expectation that Cedar or OPA would sit at the center of governance; Step 1 showed that GitHub-native controls, repo policy, CI, Kubernetes, Terraform, and agent-local permission systems are forming a mixed control plane instead [obs: en-019d98cf-f889-7930-85b3-576ed62d6562] [obs: en-019d98d0-11c4-78f3-9115-f0300472d2d5].

#### 0-3mo
The opening phase is defined by Step 0 rollup, claim 1: "OpenAI Codex, Claude Code, and Cursor are being adopted first as harness-governed repo operators rather than free-form autonomous coders." Step 0 rollup, claim 2 added that GitHub Actions, Kubernetes admission controls, and Terraform gates would remain the promotion surface. In practical terms, Anthropic Claude Code, OpenAI Codex, and Cursor become acceptable first where deterministic harnesses already exist and where PRs, CI checks, and branch protections make evidence legible [obs: en-019d98cc-232c-7610-8005-b29e28078e9c] [obs: en-019d98cc-2335-7ac3-94af-f9c4be261d83].

#### 3-6mo
Step 0 rollup, claim 4 predicted that Cedar and OPA were closer to production-scale governance than unconstrained self-modifying agents. By mid-horizon, GitHub enters the picture as a new platform actor and starts absorbing part of that function through repo-native control surfaces, while OPA still spans Kubernetes and CI [obs: en-019d98d0-5b48-7ad1-b276-00b32f57a3d3] [obs: en-019d98d0-11c4-78f3-9115-f0300472d2d5].

**Revisions to earlier predictions**
- Step 1 rollup revised the step-0 claim that "Cedar and OPA are closer to production-scale agent governance than unconstrained self-modifying software agents" into a mixed-stack claim because GitHub-native controls plus OPA/Cedar coexist instead of converging [obs: en-019d98cf-f889-7930-85b3-576ed62d6562].

#### 6-9mo
This phase introduces OpenHands as the clearest new company/tool. Step 1 rollup, claim 4 and claim 5 imply that managed agent fleets and packaged workflows start to outcompete generic autonomy stories. OpenHands' workflow catalog and deployment choices, combined with Cursor's queueing model and price ladder, move the market toward supervised task factories with explicit throughput and cost management [obs: en-019d98cf-f87f-7cf0-961c-8577b2c5c45d] [obs: en-019d98d0-11ba-7a00-b60f-0d7a88940af0] [obs: en-019d98cf-f892-74a3-b9cb-32afbc08e758].

**Revisions to earlier predictions**
- Step 1 rollup strengthened the step-0 claim that procurement and security would favor policy-wrapped autonomy by showing GitHub administrator policies for Claude and Codex agents; the mechanism was default platform enablement rather than optional sidecar governance [obs: en-019d98d0-5b48-7ad1-b276-00b32f57a3d3].

#### 9-12mo
The last phase introduces SWE-bench Verified as the decisive testing institution and fixes the boundary of progress. Step 1 rollup, claim 3 says the benchmark is still a 500-instance human-validated subset a year later, so evaluation remains scarce even while Codex, Cursor, OpenHands, Anthropic, and GitHub all broaden the product surface. The result is a governed agent-factory market: more cloud agents, more platform controls, more mixed-stack policy, but not a self-sustaining dark factory [obs: en-019d98d0-11a7-72a3-baee-8bdb3b18aad0] [obs: en-019d98d0-11b1-7c72-be6e-e0295998fd71] [obs: en-019d98d0-5b5b-7c62-b2c2-9391585ccaf6].

**Revisions to earlier predictions**
- Step 1 rollup strengthened the step-0 evaluation bottleneck claim because SWE-bench Verified remained fixed at 500 human-validated instances, showing that evaluation scarcity persisted instead of dissolving [obs: en-019d98d0-11a7-72a3-baee-8bdb3b18aad0].
- Step 1 rollup strengthened the step-0 harness-governed repo operator claim by showing Codex in isolated 1-30 minute cloud environments with explicit evidence traces, making the bounded-workcell pattern more concrete than the original projection [obs: en-019d98d0-5b3d-74d1-ae93-6922d15f517e].


### Active Directions
#### Near-term winners will operationalize directed software evolution as harness-governed agent pipelines, not fully autonomous dark factories.
**Direction ID:** en-019d98cc-234d-78a2-9333-3f14556c69be

The strongest near-term thesis is that directed software evolution will advance first through harness-governed agent pipelines, not through unconstrained autonomous coding. The current state already argues that rigor and autonomy are the same investment, and the most credible external signals reinforce that exact mechanism. OpenAI's Codex positioning centers on parallel task execution plus safe, trustworthy agents; Anthropic's Claude Code guidance centers on sub-agents, deployment integration, and controlled operational use; OpenHands and Cursor are both commercializing artifact-producing agent workflows rather than claiming generalized unattended mutation of production systems. Across these products, the common denominator is not model novelty but the execution envelope: repos, CI, branch protections, review queues, and bounded tool permissions.

From a practitioner standpoint, this means the highest-leverage build sequence over the next 12 months is clear. First, strengthen harnesses: deterministic tests, eval suites, reproducible local environments, and machine-readable acceptance gates. Second, move governance into code: OPA, Cedar-style authorization, or Temper-like typed entity/action state machines for high-risk operations such as deploy, infra mutation, secret access, and remediation. Third, allow agents to operate aggressively inside those bounds: parallel patch generation, vulnerability remediation, migration drafting, runbook execution, and PR preparation. The technical feasibility is already here for dark-factory islands inside a broader human-governed system. The mistake would be waiting for a single fully autonomous agent breakthrough; the winning teams will instead compose agent toolchains with stronger CI verification cascades and explicit policy layers, then widen autonomy as evidence accumulates.

Supporting observations: [obs: en-019d98cc-232c-7610-8005-b29e28078e9c], [obs: en-019d98cc-2335-7ac3-94af-f9c4be261d83], [obs: en-019d98cc-233d-7d63-9958-13ddd956341e], [obs: en-019d98cc-2345-7213-b0d2-71995169b1ff]

**Counterfactual:** If the market suddenly proves reliable fully autonomous production mutation without stronger harnesses and policy gates, then this thesis is too conservative and underestimates how quickly dark-factory operation can outrun today's governance stack.

#### Dark factories will stall on evaluation and governance debt, not raw model capability
**Direction ID:** en-019d98cc-4330-7d20-b7a5-883a252f22f5

Thesis: over the next day, directed software evolution will hit a governance-and-evaluation wall before it reaches dark-factory scale. The current state argues that rigor and autonomy are the same investment, but the external evidence suggests the investment is still mostly missing outside narrow demos. SWE-bench Verified exists because reliable software-agent evaluation still requires costly human validation on a constrained subset of tasks. That means the selection function for directed evolution is not yet rich enough, broad enough, or cheap enough to govern continuous autonomous variation across real production repositories.

At the same time, the live tooling market is signaling caution rather than full autonomy. Anthropic documents permission modes and approval boundaries; OpenHands documents task-fit limits and SDLC integration work; OPA and Cedar show that policy-as-code is real but fragmented. Put together, the likely near-term outcome is not a seamless dark factory but a brittle semi-automated workshop: agents generate proposals, humans curate permissions, and teams spend significant hidden labor on harness upkeep, eval maintenance, exception handling, and policy reconciliation. Organizations that treat this as primarily a model-capability problem will overinvest in more agent throughput and underinvest in the control surfaces that actually determine whether autonomous engineering is safe, cheap, and legible.

Supporting observations: [obs: en-019d98cc-430c-7eb1-b421-3eddf26b0b85], [obs: en-019d98cc-4315-7b11-bedf-5054ec460443], [obs: en-019d98cc-431e-7942-9f02-effae5f252db], [obs: en-019d98cc-4327-7522-9407-e9735713d3d6]

**Counterfactual:** If this thesis is wrong, then organizations will demonstrate unattended agent loops that reliably handle repo-specific tasks, pass broad adversarial evaluations, and operate under unified policy controls without large human review overhead.

#### Near-term advantage will accrue to policy-wrapped, harness-first agent platforms rather than maximally autonomous coding agents.
**Direction ID:** en-019d98cc-5f1b-7a43-ae57-478435816369

Directed Software Evolution is about to be selected less like a better programming assistant and more like a better industrial control architecture. Across the external evidence, the common pattern is not unconstrained agency but bounded, inspectable loops: Anthropic is shipping long-running agent workflows together with permissioning and explicit best practices; GitHub Actions remains a dominant automation substrate because it makes work triggerable, reviewable, and repeatable; OPA shows that policy engines are already accepted as a way to enforce behavior across the stack. In adjacent-domain terms, this looks like the shift from artisanal work to programmable manufacturing cells with interlocks, telemetry, and supervisor overrides.

That matters because economic selection will favor systems that reduce organizational friction, not just systems that maximize local code-generation quality. Platform teams, security teams, and procurement functions are the ecological environment in which these agents must survive. A Temper-like governed action model, or Cedar/OPA-style policy boundary around mutations, turns autonomy into something organizations can insure, audit, and gradually widen. The next day of movement in this domain therefore points toward harness-first dark-factory slices in CI, operations, and typed control planes, with freeform repository mutation expanding only where eval diversity, rollback, and recovery behavior are already instrumented.

Supporting observations: [obs: en-019d98cc-5ef4-7002-b5d0-a880813d0518], [obs: en-019d98cc-5eff-79d2-8fc9-16aab785e0de], [obs: en-019d98cc-5f09-7121-8362-5591566dea56], [obs: en-019d98cc-5f12-75b1-85d0-ba2dc9edeefd]

**Counterfactual:** If raw autonomy rather than governed control proves to be the dominant buyer preference, enterprise platforms will lose ground to IDE-native agents and benchmark-driven generalist products until governance is rebuilt after incidents.

#### Directed software evolution matures into a governed agent-factory market, with supervisory design and control-stack integration becoming the main selection pressure.
**Direction ID:** en-019d98cf-f89c-7403-a148-85e549f83ff9

Step 0 correctly identified the strongest initial thesis: agent adoption would concentrate in harness-governed repo operators rather than unconstrained autonomous coders. One year later, that thesis still holds, but the surrounding mechanism has become clearer and materially different in two ways. First, OpenAI Codex and OpenHands both present the agent as a bounded production cell: isolated runtime, explicit task envelope, reviewable PR or artifact, and evidence-bearing execution trace. This is classic industrial organization under uncertainty. Firms do not buy maximum theoretical autonomy; they buy predictable throughput inside a governed container. The relevant analogy is not biological self-direction alone but a factory line whose stations are increasingly software-defined.

Second, the governance layer did not simplify into a single policy-engine winner. Instead, the last year points toward a mixed control stack: GitHub-native controls, identity-linked OIDC pathways, CI gates, Kubernetes or Docker isolation, and selective use of policy engines such as OPA. That revises the step-0 emphasis on Cedar/OPA as the likely center of gravity. The real center is the operating system of the firm: task routing, approval queues, audit logs, and deployment permissions across many tools. Cursor's interface signals the same change from individual coding assistance to managerial oversight of multiple concurrent agent jobs. In economic terms, the scarce factor is shifting from code generation to supervisory attention and integration capacity. Over the last year, what changed is not that autonomy disappeared; it became domesticated into governed workcells, and the winning vendors are the ones that package that domestication into enterprise workflow rather than promise unrestricted self-evolution.

Supporting observations: [obs: en-019d98cf-f875-7d91-9c33-8bad5b518049], [obs: en-019d98cf-f87f-7cf0-961c-8577b2c5c45d], [obs: en-019d98cf-f889-7930-85b3-576ed62d6562], [obs: en-019d98cf-f892-74a3-b9cb-32afbc08e758]

**Counterfactual:** If this direction is wrong and raw autonomy becomes the decisive factor, then tightly governed multi-tool stacks and supervisory workflows will be outcompeted by end-to-end autonomous systems with minimal human review.

#### By day 365, directed software evolution advances as a governed cost-sensitive workflow layer, not as a self-sustaining dark factory.
**Direction ID:** en-019d98d0-11cd-7a31-ba50-6f3cd1334e15

Step 0 correctly argued that harness-governed repo operators would grow faster than free-form dark factories, but the last year changes the reason this thesis matters. The strongest new evidence is not that governance has become simpler; it is that capability has expanded faster than verification and cost control. OpenAI positioned Codex on May 16, 2025 as a parallel cloud software engineering agent and by June 3, 2025 added broader availability and internet access during execution. Cursor's pricing stack now monetizes the jump from assisted coding to cloud-agent execution, with visible tiers at $20, $60, and $200 per month. These are strong signals that the market is successfully productizing agentic coding, but they also reveal the hidden liabilities: more parallel work, more retrieval, more billable execution, and more places where provenance becomes hard to reconstruct after the fact.

At the same time, the evaluation and governance bottlenecks from step 0 did not resolve. SWE-bench Verified still advertises a human-validated subset of 500 instances, which means public legitimacy still depends on narrow, expensive evaluation rather than cheap organization-specific proof. Governance is also not converging into one clean control plane. OPA remains credible in Kubernetes and audit-heavy settings, while Claude Code surfaces permission modes, administration, and CI/CD integration as operational primitives. The consequence is a fragmented but durable equilibrium: agent use expands, yet fully autonomous software factories stall because every increase in capability multiplies review, policy, and spend obligations across different layers. Over the last year, the strongest step-0 thesis survives, but it should be revised from harnesses as an adoption accelerator to harnesses as a hard ceiling on unconstrained autonomy.

Supporting observations: [obs: en-019d98d0-11a7-72a3-baee-8bdb3b18aad0], [obs: en-019d98d0-11b1-7c72-be6e-e0295998fd71], [obs: en-019d98d0-11ba-7a00-b60f-0d7a88940af0], [obs: en-019d98d0-11c4-78f3-9115-f0300472d2d5]

**Counterfactual:** If this direction is wrong, then benchmark progress, policy tooling, and falling execution costs will combine quickly enough that organizations begin trusting largely autonomous repo-to-production loops with only sparse human checkpoints.

#### Directed software evolution will operationalize as repo-instruction-driven cloud agents behind mixed policy gates, not as a single autonomous coding stack.
**Direction ID:** en-019d98d0-5b65-7162-a3da-be7df2005b53

The strongest surviving step-0 thesis was that adoption would center on harness-governed repo operators rather than unconstrained autonomous coders, and the last year strengthened that claim while changing where the bottleneck sits. What changed is that the major vendors and open-source challengers now expose more of the operational substrate directly: Codex made isolated cloud sandboxes, test execution, AGENTS.md repository instructions, and evidence-bearing logs into first-class workflow primitives; GitHub turned agent enablement and model selection into administrator-controlled product settings; OpenHands packaged the same basic architecture across CLI, local GUI, cloud, and Kubernetes enterprise surfaces. That means the architecture pattern is no longer speculative. It is a shipping control plane pattern: agent frontend for code generation and exploration, deterministic backend for tests, policy, and promotion.

The main revision from step 0 is that governance and evaluation are fragmenting by layer rather than converging into a single universal control stack. Cedar is credible for embedded application authorization, but OPA remains the practical choice across Kubernetes, CI/CD, and cross-cutting infrastructure policy. Likewise, SWE-bench-style validation still matters, but day-365 adoption decisions are increasingly driven by whether a team can encode repo-local commands, environment setup, and secret boundaries so the agent can operate repeatably. Over the next year, the winning implementations in directed software evolution will therefore be harness-first, policy-layered, and deployment-surface aware: organizations will standardize on repo instruction files, ephemeral task sandboxes, CI verification cascades, and mixed policy engines, then let agents operate aggressively inside those rails.

Supporting observations: [obs: en-019d98d0-5b3d-74d1-ae93-6922d15f517e], [obs: en-019d98d0-5b48-7ad1-b276-00b32f57a3d3], [obs: en-019d98d0-5b52-7bb2-a766-cdda83a9860a], [obs: en-019d98d0-5b5b-7c62-b2c2-9391585ccaf6]

**Counterfactual:** If a single end-to-end autonomous coding stack becomes trusted without repo-specific harnesses, policy layering, or admin controls, then the harness-first and mixed-governance forecast is wrong.


### What Surprised Us
- The market did not centralize around Cedar or OPA alone; mixed control stacks won faster than many governance narratives implied [obs: en-019d98cf-f889-7930-85b3-576ed62d6562].
- Benchmark legitimacy remained pinned to 500 human-validated SWE-bench Verified instances even after a year of rapid product launches [obs: en-019d98d0-11a7-72a3-baee-8bdb3b18aad0].
- Cursor's product story points toward managerial queue supervision, not merely better autocomplete, which shifts the bottleneck from coding to decomposition and review [obs: en-019d98cf-f892-74a3-b9cb-32afbc08e758].
- OpenHands' packaging of incident triage, remediation, and migration as fixed workflows suggests buyers want contract-manufacturing-like cells rather than general AI engineers [obs: en-019d98cf-f87f-7cf0-961c-8577b2c5c45d].


### Top 5 Predictions with Falsification Criteria
1. **Prediction:** By 2026-06-30, at least one major source-control platform will offer administrator-gated model selection for both Anthropic and OpenAI coding agents as a standard enterprise control.
   - **Measurable indicator:** platform UI or changelog names at least 2 vendor policies and repo-level enablement controls.
   - **Confidence:** high
   - **Falsification:** If no major source-control platform has shipped named Anthropic/OpenAI admin gating by 2026-06-30, this prediction is wrong because governance will have remained an add-on rather than a platform default.
   - **Supporting observations:** [obs: en-019d98d0-5b48-7ad1-b276-00b32f57a3d3], [obs: en-019d98cc-5f09-7121-8362-5591566dea56]
2. **Prediction:** By 2026-09-30, mixed governance stacks combining repo policy, CI policy, and infrastructure admission control will be more common in enterprise agent deployments than single-engine Cedar-only or OPA-only stacks.
   - **Measurable indicator:** at least 3 control layers are documented together in enterprise reference architectures.
   - **Confidence:** high
   - **Falsification:** If by 2026-09-30 most enterprise case studies show a single universal policy runtime replacing repo, CI, and infra-specific controls, this prediction is wrong because fragmentation would have resolved faster than expected.
   - **Supporting observations:** [obs: en-019d98cf-f889-7930-85b3-576ed62d6562], [obs: en-019d98d0-11c4-78f3-9115-f0300472d2d5]
3. **Prediction:** By 2026-12-31, enterprise spending on coding agents will concentrate in supervised heavy users and background-job pools rather than broad seat-level autonomy.
   - **Measurable indicator:** price ladders similar to $20/$60/$200 or usage-based cloud-agent tiers remain visible across top vendors.
   - **Confidence:** medium
   - **Falsification:** If by 2026-12-31 top vendors flatten pricing into near-universal low-cost seats without premium agent-execution tiers, this prediction is wrong because cloud-agent economics will have commoditized faster than expected.
   - **Supporting observations:** [obs: en-019d98d0-11ba-7a00-b60f-0d7a88940af0], [obs: en-019d98cf-f892-74a3-b9cb-32afbc08e758]
4. **Prediction:** By 2026-12-31, benchmark leadership alone will still not be sufficient for procurement; major buyers will require repo-specific harness evidence beyond public SWE-bench scores.
   - **Measurable indicator:** vendor evaluations or enterprise pilots include custom harnesses, rollback drills, or environment-packaging tests in addition to public benchmarks.
   - **Confidence:** high
   - **Falsification:** If by 2026-12-31 large buyers rely primarily on public benchmark standings without custom acceptance harnesses, this prediction is wrong because verification scarcity would not be a practical bottleneck.
   - **Supporting observations:** [obs: en-019d98d0-11a7-72a3-baee-8bdb3b18aad0], [obs: en-019d98d0-5b5b-7c62-b2c2-9391585ccaf6]
5. **Prediction:** By 2026-12-31, the most credible agent deployments will be packaged as bounded workcells with isolated execution and evidence traces rather than unrestricted shell autonomy.
   - **Measurable indicator:** vendors advertise isolated runtimes, task envelopes, and terminal or test evidence in product docs.
   - **Confidence:** high
   - **Falsification:** If by 2026-12-31 leading vendors stop emphasizing isolation, task boundaries, and evidence traces in favor of unrestricted autonomous production mutation, this prediction is wrong because safety and governance asymmetry would have collapsed.
   - **Supporting observations:** [obs: en-019d98cf-f875-7d91-9c33-8bad5b518049], [obs: en-019d98d0-5b3d-74d1-ae93-6922d15f517e]


### Decision Points
#### Decision Point 1
- **Decision:** Whether to deploy Cedar for application-level agent authorization while keeping OPA/Gatekeeper on Kubernetes and CI.
- **Timing trigger:** When the organization enables more than 2 cloud coding agents or starts granting deploy permissions, likely by 2026-07.
- **Option A:** deploy Cedar policy gates on product actions and OPA/Gatekeeper on CI/Kubernetes
  — **Tradeoff:** 3-5 engineering-weeks plus ongoing dual-stack policy maintenance.
- **Option B:** standardize on OPA/Rego for CI, admission, and agent action brokering
  — **Tradeoff:** 4-6 engineering-weeks and higher app-layer integration friction.
- **Option C:** rely on GitHub-native rules plus vendor permission modes only
  — **Tradeoff:** 1-2 engineering-weeks but weaker cross-surface audit consistency and vendor lock-in risk.
- **Recommended:** Option A because the projection shows mixed stacks are winning and fine-grained product authorization plus infra policy are diverging operationally.

#### Decision Point 2
- **Decision:** Whether to fund repo-specific evaluation harnesses for Codex, Claude Code, Cursor, or OpenHands before scaling background agents.
- **Timing trigger:** When public benchmark scores become part of vendor selection or after the first failed autonomous PR pilot, likely by 2026-08.
- **Option A:** build custom harnesses in GitHub Actions with rollback drills, flaky-test detection, and repo-specific acceptance suites
  — **Tradeoff:** 4-8 engineering-weeks and ongoing test maintenance.
- **Option B:** use only vendor benchmarks such as SWE-bench Verified plus manual review
  — **Tradeoff:** 1-2 engineering-weeks but high overfitting risk and weaker production validity.
- **Option C:** outsource evaluation to a platform team or specialist vendor with managed harness infrastructure
  — **Tradeoff:** $50K-150K annual and dependence on external instrumentation priorities.
- **Recommended:** Option A because the projection repeatedly shows evaluation scarcity, not candidate generation, is the harder ceiling on safe scaling.

#### Decision Point 3
- **Decision:** Whether to expand from supervised IDE assistance into queued cloud-agent execution on Cursor, Codex, or OpenHands.
- **Timing trigger:** When monthly spend on agent seats exceeds pilot budget or when teams request parallel background task execution, likely by 2026-09.
- **Option A:** keep agents in supervised IDE/CLI mode only with no background execution
  — **Tradeoff:** 0-1 engineering-weeks, lower risk, but limited throughput gains.
- **Option B:** enable bounded cloud agents for low-risk workflows such as dependency bumps, vulnerability fixes, and PR preparation
  — **Tradeoff:** 2-4 engineering-weeks plus cloud runtime and review-queue costs.
- **Option C:** pursue broad autonomous pipeline execution including deploy-affecting tasks
  — **Tradeoff:** 6-10 engineering-weeks, requires dedicated platform team, and carries high incident/blast-radius risk.
- **Recommended:** Option B because bounded workcells are the most credible pattern in the data and preserve a path to scale without overcommitting to dark-factory assumptions.


### Assumptions & Limitations
1. **Assumption:** Enterprise buyers continue to prefer legible, policy-wrapped autonomy over raw unrestricted autonomy.
   - **If-wrong:** Raw-throughput vendors could outrun governance-first platforms and compress the control-plane advantage.
   - **Confidence:** medium-high
2. **Assumption:** Public benchmark legitimacy remains scarce and expensive relative to custom repo-specific verification.
   - **If-wrong:** Cheap, broad, trustworthy evaluation could unlock more aggressive autonomy sooner than projected.
   - **Confidence:** high
3. **Assumption:** Mixed control stacks remain operationally easier than single-engine convergence across application, CI, and infrastructure surfaces.
   - **If-wrong:** A dominant universal control plane could centralize governance and alter vendor power quickly.
   - **Confidence:** medium


### Methodology
- 3 independent probes per step
- 2 time steps over 1 year
- 24 total observations, 6 total active directions
- External evidence included vendor docs, changelogs, benchmark pages, pricing pages, and policy-engine documentation
- Temporal Progression assembled from 2 durable step rollup files written during the loop
