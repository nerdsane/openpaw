## Foresight Projection: Directed Software Evolution — April 2026 to April 2027

---

### Executive Summary

Directed software evolution over the next 12 months will advance through harness-native agent deployment and institutional governance, not through a leap to fully autonomous software factories. The engine's three independent probes — practitioner, critic, and adjacent-domain analyst — converge on a central finding: the binding constraint is not model capability but certified flow through the evaluation harness. Vendors are converging on validation and remediation workflows: Harness embeds AI agents inside pull request and pipeline operations [obs: en-019d970f-07ea-7ca0-9846-844c8520637e], OpenAI's Codex standardizes repeatable agent workflows around MCP and traceable orchestration [obs: en-019d970f-0802-7a93-ab7c-093505b8013a], and products like Aether describe agents that spin up in isolated VMs, reproduce failures, verify fixes, and open PRs with proof [obs: en-019d970f-07f6-7b83-847e-e410c8e3c805].

The adjacent-domain analysis reinforces this: DORA's 2025 report says AI amplifies organizational strengths and weaknesses, borrowing from manufacturing's insight that shorter lead times come from limiting inventory, not from faster machines [obs: en-019d970f-3112-7a60-b9d7-276ac8db1e66]. Anthropic's enterprise Claude Code rollout adds spend controls, analytics, managed policy settings, and a Compliance API, signaling that the market is converging on governance infrastructure as a prerequisite to scale [obs: en-019d970f-311e-78d1-ab76-cce58a36ae1e]. The critic perspective adds that MCP-related security guidance from Microsoft treats Model Context Protocol integrations as a live indirect-prompt-injection surface [obs: en-019d970f-6391-79c1-a767-f8478d5d0a9c], and SWE-Bench Pro shows widely used coding models remain below 25% Pass@1 on realistic enterprise tasks [obs: en-019d970f-639e-7370-9b86-d3932886880b].

The teams that capture value first will invest in repo-specific acceptance harnesses, MCP-backed tool scoping, governance infrastructure (spend controls, policy-as-code, approval boundaries), and supply-chain-style control towers that allocate autonomous effort under WIP, budget, and policy constraints. The next year is not about who deploys the most autonomous agents; it is about who builds the tightest certified flow from candidate generation through selection and governance.

---

### Key Findings

**1. The main implementation pattern shipping now is harness-first agent deployment, not fully autonomous coding.**
Platform teams put agent work behind repo-specific acceptance harnesses: unit and integration tests, replayable CI jobs, policy checks, and PR-based review. The New Stack article "Enterprise dev teams are about to hit a wall. And CI pipelines can't save them" and Harness documentation on AI PR Agents confirm this pattern.
*Theme: Technical architecture* [obs: en-019d970f-07ea-7ca0-9846-844c8520637e]

**2. A near-term architecture standard emerges around isolated execution plus evidence-bearing PRs.**
The Aether product page describes agents that spin up in isolated VMs, reproduce failures, verify fixes, and open PRs with proof. Harness AI PR Agents describe the same control surface inside pipelines. Within 90 days, platform teams copy this pattern even without buying these products.
*Theme: Technical architecture* [obs: en-019d970f-07f6-7b83-847e-e410c8e3c805]

**3. MCP-backed agent scaffolding is the most important technical enabler, not better base models.**
OpenAI's Codex MCP documentation and cookbook on consistent workflows with Codex CLI and the Agents SDK show a concrete mechanism: expose stable tools and let agents compose verified workflows rather than freeform code generation.
*Theme: Standards/protocols* [obs: en-019d970f-0802-7a93-ab7c-093505b8013a]

**4. The dark-factory narrative is more exposed to control-plane compromise than admitted.**
Microsoft's April 2025 post "Protecting against indirect prompt injection attacks in MCP" treats MCP integrations as a live indirect-prompt-injection surface. MCP Security Research documented prompt-injection vectors through MCP servers, meaning untrusted artifacts can steer agent tool use unless tools are strongly isolated.
*Theme: Governance/security* [obs: en-019d970f-6391-79c1-a767-f8478d5d0a9c]

**5. Long-horizon coding-agent evals argue against near-term general autonomy.**
SWE-Bench Pro reports widely used coding models remain below 25% Pass@1 on realistic enterprise tasks, with GPT-5 highest at 23.3%. Even scaffolded leaderboard runs top out at 43.72% resolved for SWE-Agent + Claude-4.5-Sonnet with 250-turn limits.
*Theme: Evaluation/testing* [obs: en-019d970f-639e-7370-9b86-d3932886880b]

**6. Task structure and evaluation quality matter more than model branding.**
A 2026 MSR paper "Comparing AI Coding Agents: A Task-Stratified Analysis of Pull Request Acceptance" finds large variation by task type: documentation PRs reach 82.1% acceptance while new features are 66.1%, and no single agent leads every category.
*Theme: Evaluation/testing* [obs: en-019d970f-63ab-7391-853e-806adcd09e9a]

**7. Directed software evolution is moving toward just-in-time manufacturing with certified flow through the harness.**
DORA's 2025 report borrows from manufacturing: shorter lead times come from limiting inventory, not faster machines. Agents that run ahead of the verification pipeline create risk-inventory. The near-term winners limit agent WIP to what the harness can certify per cycle.
*Theme: Cross-domain/organizational* [obs: en-019d970f-3112-7a60-b9d7-276ac8db1e66]

**8. The bottleneck is shifting from model intelligence to budgeted organizational permissioning.**
Anthropic's enterprise Claude Code rollout adds spend controls, usage analytics, managed policy settings, and a Compliance API. Firms are treating coding agents like governed production equipment with quotas, telemetry, and procurement controls.
*Theme: Economics/market* [obs: en-019d970f-311e-78d1-ab76-cce58a36ae1e]

**9. The harness is becoming the environment, and benchmark design is becoming ecological engineering.**
SWE-bench Verified is a human-validated subset of 500 instances specifically created to ensure tasks are clear and solvable. In biological terms, fitness is highly dependent on the niche definition. Over the next 90 days, the most impactful action is improving the niche, not improving the organism.
*Theme: Cross-domain/biology* [obs: en-019d970f-312b-7982-ab8b-5695f4ddc9c4]

**10. Safety constraints mature faster than liveness proofs, stalling autonomy in governance-heavy environments.**
Policy engines, approval gates, sandboxing, and audit trails will improve, but systems still fail on ambiguous objectives, cross-repo coordination, and economic tradeoffs. The likely near-term operating point is: proven safety gates plus experimental liveness probes.
*Theme: Governance/policy* [obs: en-019d970f-63b9-7870-be30-d1cd6a6bbefa]

**11. A new adoption split appears between application teams and platform teams.**
Application teams use coding agents in IDEs, but platform teams become the chokepoint because they own CI, secrets, runners, policy, and observability. Products like Agent CI position evaluation gates as CI-native concerns.
*Theme: Organizational/adoption* [obs: en-019d970f-081d-7d03-82cf-516737ae590f]

**12. The "dark factory" is overstated for the near term; semi-autonomous maintenance under fixed guardrails is what actually expands.**
Vendor messaging reinforces this: Harness AI PR Agents target code review, CI remediation, and test-writing tasks. Codex materials emphasize repeatability, traceability, and scoped context rather than unrestricted self-modification.
*Theme: Market/adoption* [obs: en-019d970f-080f-79d0-b88f-2bcc1f58b8a1]

**13. The next 90 days produce supply-chain-style control towers, not dark factories.**
Manufacturing analogs emphasize resilient orchestration, weekly stability, daily feasibility, exception handling, and compliance visibility rather than full lights-out autonomy. Enterprises will integrate coding agents into exception-routing pipelines with explicit approval thresholds.
*Theme: Cross-domain/manufacturing* [obs: en-019d970f-3139-7350-a020-5fca7cd98766]

---

### Temporal Progression

#### Phase 1: 0-3 months (April - July 2026)
**What changes:**
- Platform teams adopt harness-first agent deployment: scoped tools via MCP, isolated execution environments, repo-specific evals, and PR-based delivery with machine-generated evidence [obs: en-019d970f-07ea-7ca0-9846-844c8520637e].
- Vendors converge on isolated execution + evidence-bearing PR architecture: Aether, Harness AI PR Agents, OpenAI Codex all ship this pattern [obs: en-019d970f-07f6-7b83-847e-e410c8e3c805].
- MCP-backed agent scaffolding becomes the primary enabler for repeatable, auditable agent workflows [obs: en-019d970f-0802-7a93-ab7c-093505b8013a].

**Signals expected:**
- More product announcements centered on evals, repo memory, approval workflows, and audit logs.
- Agent CI products (agent-ci.com) and similar evaluation gates become CI-native.
- Anthropic, OpenAI, Cursor, Harness, and similar vendors emphasize governance UX over model capability.

**What has NOT changed:**
- Agents still do not reliably own complex cross-service production changes end to end [obs: en-019d970f-639e-7370-9b86-d3932886880b].
- MCP authorization remains a live prompt-injection surface [obs: en-019d970f-6391-79c1-a767-f8478d5d0a9c].

**Causal links to Phase 2:**
Once teams have evals, traces, and approval logic, they can begin comparing multiple generated variants rather than asking whether a single agent output is "good enough."

---

#### Phase 2: 3-6 months (July - October 2026)
**What changes:**
- Advanced teams begin routine multi-candidate generation for bounded tasks evaluated against internal harnesses.
- Enterprise Claude Code spend controls, analytics, and Compliance API become standard procurement requirements [obs: en-019d970f-311e-78d1-ab76-cce58a36ae1e].
- Platform teams formalize control-tower architectures: WIP caps, budget-aware scheduling, explicit exception routing [obs: en-019d970f-3139-7350-a020-5fca7cd98766].

**Signals expected:**
- Public or private benchmarks shift from pass@1 to pass@k or best-of-n outcome metrics.
- More teams store operational context as machine-readable assets: incident traces, policy histories, action graphs.
- NIST or ISO publishes a draft framework for AI agent operational trust.

**What has NOT changed:**
- Diversity preservation remains immature; systems generate many variants but select on narrow metrics.
- Multi-agent coordination overhead persists.

**Revisions to earlier predictions:**
- If MCP authorization stabilizes faster than expected (driven by Microsoft's security guidance), semi-autonomous workflows may reach Phase 3 maturity earlier.
- If task-stratified evaluation data [obs: en-019d970f-63ab-7391-853e-806adcd09e9a] shows wider acceptance gaps than expected, firms may narrow agent scope further.

**Causal links to Phase 3:**
Organizations with mature harnesses and governance can begin limited evolutionary retention — keeping successful patterns rather than treating each task independently.

---

#### Phase 3: 6-9 months (October 2026 - January 2027)
**What changes:**
- Leading teams move from one-off agent assistance to compounding loops: generate variants, evaluate under harnesses, retain successful patterns.
- The adoption split between application teams and platform teams [obs: en-019d970f-081d-7d03-82cf-516737ae590f] resolves toward platform-team ownership of agent governance.
- Harness-as-product emerges: open-source and commercial products focused on agent evaluation, selection, and policy enforcement gain traction.

**Signals expected:**
- At least one major tech company reports measurable productivity improvements attributed to harness-bound agent workflows.
- Universities and bootcamps begin offering coursework on agent evaluation and workflow design.
- Open-source harness frameworks reach 1,000+ GitHub stars.

**What has NOT changed:**
- Safety-liveness asymmetry persists: proving agents are NOT doing harm remains easier than proving they are doing good [obs: en-019d970f-63b9-7870-be30-d1cd6a6bbefa].
- True dark-factory operation remains confined to low-blast-radius internal workflows.

**Revisions to earlier predictions:**
- If DORA's "AI amplifies dysfunction" finding [obs: en-019d970f-3112-7a60-b9d7-276ac8db1e66] proves stronger than expected, some organizations may be in active agent rollback rather than expansion.

**Causal links to Phase 4:**
Accumulated harness quality and operational trust create preconditions for selective machine-tool construction patterns.

---

#### Phase 4: 9-12 months (January - April 2027)
**What changes:**
- Directed software evolution enters mainstream enterprise planning as a recognized operational capability with known maturity stages.
- Control-tower architectures [obs: en-019d970f-3146-7dc1-a4cb-0a898347d7dd] become a named pattern in engineering management literature.
- The trust gap narrows but does not close: NIST or equivalent publishes v1 operational trust standards for AI agents.

**Signals expected:**
- Industry analysts publish maturity models specifically for directed software evolution.
- At least one acquisition in the agent-evaluation/harness space signals market validation.
- Measurable divergence between harness-mature and harness-immature organizations becomes a cited competitive factor.

**What has NOT changed:**
- Fully autonomous, self-improving software systems remain aspirational. No organization has demonstrated reliable self-modification of selection harnesses by agents.
- Prompt injection and context poisoning remain open problems [obs: en-019d970f-6391-79c1-a767-f8478d5d0a9c].

**Revisions to earlier predictions:**
- If the labor-market recomposition effect proves more disruptive than expected, regulatory responses may introduce new constraints on agent autonomy.

---

### Active Directions

**Direction 1: The next 90 days belong to harness-native coding agents: platform teams will adopt PR-producing, evidence-backed automation before they trust autonomous repo evolution.**

Over the next 90 days, directed software evolution advances through platformized harnesses rather than through a jump to fully autonomous software factories. The implementation pattern that crosses from novelty to routine practice is: scoped tools via MCP or equivalent interfaces, isolated execution environments, repo-specific evals, and PR-based delivery with machine-generated evidence. This is feasible now because it aligns with the assets organizations already trust: CI pipelines, branch protection, audit logs, and human approval points.

The strongest evidence from current market signals is that vendors are converging on validation and remediation workflows. Harness is embedding AI agents inside pull request and pipeline operations, and Codex materials are standardizing repeatable agent workflows around MCP and traceable orchestration. That means the real competitive advantage in this period is not having the most powerful model, but having the best acceptance harness: deterministic repro, good test coverage, policy enforcement, environment simulation, and clean rollback paths. Teams that invest there will accumulate a compounding advantage because each successful agent task strengthens the next selection loop; teams that chase open-ended autonomy without that substrate will generate more patches but fewer trusted changes.

*Supporting observations:* [obs: en-019d970f-07ea-7ca0-9846-844c8520637e], [obs: en-019d970f-07f6-7b83-847e-e410c8e3c805], [obs: en-019d970f-0802-7a93-ab7c-093505b8013a], [obs: en-019d970f-080f-79d0-b88f-2bcc1f58b8a1], [obs: en-019d970f-081d-7d03-82cf-516737ae590f]

*Counterfactual:* If this thesis is wrong, model capability will outrun harness constraints quickly and teams will skip pipeline-centric guardrails in favor of broader unattended codebase modification. In that world, CI and policy systems become secondary rather than primary control surfaces.

---

**Direction 2: The next breakthrough is not a smarter coding agent but a control-tower architecture that allocates autonomous effort under WIP, budget, and policy constraints.**

My strongest thesis is that directed software evolution will advance in the next 90 days primarily through better economic and cybernetic governance, not through a sudden leap in autonomous coding intelligence. Across manufacturing, supply chains, and organizational economics, systems scale when they convert invisible work into managed flow: WIP limits, queue visibility, spend ceilings, and exception-routing all matter because they keep the system near an operating point where local optimizers do not destabilize the whole. The current knowledge graph correctly emphasizes harness-first engineering, but the adjacent-field lesson is that the harness is only one layer of the control loop. The other layer is institutional: who is allowed to spend compute, which repos get autonomous effort, what policies bind tool use, and how exceptions escalate.

Recent external signals reinforce this. DORA's 2025 AI-assisted development framing says AI amplifies organizational strengths and weaknesses, which means firms with poor flow discipline will simply automate disorder. Anthropic's enterprise Claude Code rollout adds spend controls, analytics, managed policy settings, and a Compliance API, signaling that the market is converging on governance infrastructure as a prerequisite to scale. SWE-bench Verified likewise shows that outcomes depend heavily on the evaluation niche and scaffold, not only the base model. So the near-term winning architecture for directed software evolution is a control tower over a bounded set of agents: repo-specific fitness functions, explicit WIP caps, budget-aware scheduling, and human authority over rare but high-cost exceptions.

*Supporting observations:* [obs: en-019d970f-3112-7a60-b9d7-276ac8db1e66], [obs: en-019d970f-311e-78d1-ab76-cce58a36ae1e], [obs: en-019d970f-312b-7982-ab8b-5695f4ddc9c4], [obs: en-019d970f-3139-7350-a020-5fca7cd98766]

*Counterfactual:* If governance and flow control are not the binding constraints, then raw model quality should dominate outcomes in the next 90 days and organizations should see reliable autonomy scale without major new budgeting, policy, or orchestration layers.

---

**Direction 3: The next 90 days favor constrained evolutionary loops, not true dark factories.**

My strongest thesis is that directed software evolution will make visible progress in the next 90 days, but mostly by tightening harnesses and narrowing action scopes rather than by delivering true dark-factory autonomy. External signals point in the same direction: MCP-related security guidance from Microsoft and the Kilo Code prompt-injection vulnerability both show that the control plane around coding agents is still fragile. When untrusted text can influence tool choice or permission boundaries, scaling autonomy increases blast radius faster than it increases useful output.

The performance data is equally constraining. SWE-Bench Pro shows long-horizon software tasks remain difficult even for strong models and scaffolds, while task-stratified PR acceptance data suggests that outcome quality is driven heavily by task type and evaluation design. So the strategically correct move is not to assume general autonomous software factories are imminent; it is to build governed, repo-specific evolutionary loops where agents generate candidates, run strong tests, and hand off irreversible decisions. If this thesis is wrong and broad autonomy is actually ready, organizations that over-constrain agents may leave productivity on the table. But if the thesis is right, then the winners are the teams that treat harness quality, permission design, and rollbackability as the main product surface.

*Supporting observations:* [obs: en-019d970f-6391-79c1-a767-f8478d5d0a9c], [obs: en-019d970f-639e-7370-9b86-d3932886880b], [obs: en-019d970f-63ab-7391-853e-806adcd09e9a], [obs: en-019d970f-63b9-7870-be30-d1cd6a6bbefa]

*Counterfactual:* If broad autonomous software delivery becomes reliable faster than these signals suggest, organizations that keep tight human gates may underexploit a real window for compounding speed gains.

---

### What Surprised Us

**1. The dark-factory narrative is more exposed to control-plane compromise than the base model narrative admits.** Microsoft's MCP security guidance and MCP Security Research documented prompt-injection vectors through MCP servers, meaning every tool-use surface is a potential hostile control channel — not just the code itself.
[obs: en-019d970f-6391-79c1-a767-f8478d5d0a9c]

**2. Task structure matters more than model branding.** The MSR task-stratified analysis of PR acceptance shows documentation PRs reach 82.1% acceptance while new features are 66.1%, and no single agent leads every category. This challenges the narrative that any single "best" agent dominates.
[obs: en-019d970f-63ab-7391-853e-806adcd09e9a]

**3. The dark factory is not imminent even by vendor messaging standards.** Harness AI PR Agents target code review, CI remediation, and test-writing — explicitly scoped maintenance, not autonomous repo evolution.
[obs: en-019d970f-080f-79d0-b88f-2bcc1f58b8a1]

**4. Application teams vs. platform teams is becoming the key adoption split.** Platform teams own CI, secrets, runners, and observability — making them the real chokepoint for agent autonomy expansion.
[obs: en-019d970f-081d-7d03-82cf-516737ae590f]

---

### Top 5 Predictions with Falsification Criteria

**Prediction 1: By July 2026, the dominant deployment pattern for coding agents in enterprises will be harness-native PR workflows (isolated execution, evidence-bearing PRs, human merge gates), not autonomous commit-to-deploy pipelines.**

- *Measurable indicator:* >70% of coding agent vendor landing pages emphasize PR review, evidence, and human approval rather than autonomous deployment.
- *Confidence:* High (75%)
- *Falsification:* If by July 2026 more than 3 major enterprises publicly permit fully autonomous merge-to-deploy coding agents on core production systems without human review, this prediction is wrong because harness constraints were less binding than expected.
- *Supporting observations:* [obs: en-019d970f-07ea-7ca0-9846-844c8520637e], [obs: en-019d970f-07f6-7b83-847e-e410c8e3c805]

**Prediction 2: By October 2026, at least one high-visibility security incident will involve an agent executing hostile instructions embedded via MCP tool integrations while passing local tests.**

- *Measurable indicator:* A CVE, public post-mortem, or security advisory explicitly attributing a code-level incident to indirect prompt injection via MCP server context.
- *Confidence:* High (70%)
- *Falsification:* If no such incident is publicly reported by October 2026, this prediction is wrong because either early adopters kept tool scopes narrow enough or MCP prompt-injection is harder to weaponize than Microsoft's security guidance suggests.
- *Supporting observations:* [obs: en-019d970f-6391-79c1-a767-f8478d5d0a9c]

**Prediction 3: By January 2027, enterprise procurement for coding agents will weight governance features (spend controls, audit logs, policy-as-code, approval workflows) more heavily than benchmark scores.**

- *Measurable indicator:* At least 3 Fortune 500 RFPs or public vendor evaluations list governance as a top-3 criterion above model performance.
- *Confidence:* Medium-High (65%)
- *Falsification:* If by January 2027 benchmark scores remain the primary procurement criterion and governance features are optional add-ons, this prediction is wrong because enterprises absorbed governance into existing security teams rather than demanding it from vendors.
- *Supporting observations:* [obs: en-019d970f-311e-78d1-ab76-cce58a36ae1e], [obs: en-019d970f-3139-7350-a020-5fca7cd98766]

**Prediction 4: By October 2026, SWE-Bench Pro Pass@1 for frontier models will remain below 35%, keeping human approval structurally necessary for production-path changes.**

- *Measurable indicator:* SWE-Bench Pro leaderboard or published results showing frontier model Pass@1 scores.
- *Confidence:* Medium (60%)
- *Falsification:* If frontier model Pass@1 exceeds 50% on SWE-Bench Pro by October 2026, this prediction is wrong because model capability advanced faster than the current trajectory implies.
- *Supporting observations:* [obs: en-019d970f-639e-7370-9b86-d3932886880b]

**Prediction 5: By April 2027, the "workflow designer" or "agent evaluation lead" role will exist at 5+ publicly visible organizations as a recognized position distinct from DevOps or platform engineering.**

- *Measurable indicator:* Public job postings, LinkedIn role announcements, or org chart mentions at named companies.
- *Confidence:* Medium (55%)
- *Falsification:* If no such roles are publicly visible by April 2027, this prediction is wrong because agent coordination was absorbed into existing platform-engineering roles rather than creating new dedicated positions.
- *Supporting observations:* [obs: en-019d970f-081d-7d03-82cf-516737ae590f], [obs: en-019d970f-3112-7a60-b9d7-276ac8db1e66]

---

### Decision Points

#### Decision Point 1
- **Decision:** When to invest in repo-specific evaluation harnesses vs. relying on vendor-provided defaults
- **Timing trigger:** When agent-generated PR acceptance rate drops below 60%, or when agents pass tests but reviewers consistently reject for architectural reasons [obs: en-019d970f-07ea-7ca0-9846-844c8520637e]
- **Option A:** Build custom repo-specific evals (deterministic setup scripts, reproducible fixtures, machine-readable completion criteria, architecture-level property tests). — **Tradeoff:** 2-4 engineering-weeks upfront, ~5 hours/week ongoing maintenance.
- **Option B:** Adopt vendor-bundled evaluation (GitHub Copilot review, Cursor inline verification). — **Tradeoff:** 1-2 days setup, minimal maintenance, but misses repo-specific failure modes.
- **Option C:** Hybrid — use vendor defaults for low-risk tasks (docs, tests, dependency bumps) and custom harnesses for production-path code. — **Tradeoff:** 1-2 engineering-weeks upfront, ~3 hours/week ongoing.
- **Recommended:** Option C. Evidence strongly suggests harness quality matters more than model quality [obs: en-019d970f-0802-7a93-ab7c-093505b8013a], but full custom harnesses for all work is premature.

#### Decision Point 2
- **Decision:** When to adopt MCP-based tool scoping vs. waiting for authorization-layer maturity
- **Timing trigger:** When agent workflows need 3+ external system integrations (CI, project management, observability) and custom integrations are accumulating [obs: en-019d970f-0802-7a93-ab7c-093505b8013a]
- **Option A:** Adopt MCP now with hardened OAuth preregistration and aggressive scope-narrowing. — **Tradeoff:** 1-2 engineering-weeks for initial setup + ongoing auth maintenance; accepts prompt-injection risk [obs: en-019d970f-6391-79c1-a767-f8478d5d0a9c].
- **Option B:** Wait for MCP authorization to stabilize (estimated Q3-Q4 2026). Continue vendor-specific integrations. — **Tradeoff:** Higher per-integration cost now, lower migration risk later.
- **Option C:** Adopt MCP for read-only tool access; restrict write operations to vendor APIs with established auth. — **Tradeoff:** 1 engineering-week setup, captures portability benefit while containing trust risk.
- **Recommended:** Option C. MCP's tool-contract value is real but the authorization gap is also real. Read-only MCP captures portability while containing risk.

#### Decision Point 3
- **Decision:** When to formalize governance infrastructure (spend controls, policy-as-code, exception routing) vs. ad hoc agent management
- **Timing trigger:** When 3+ engineers spend >20% of their time maintaining agent configurations, prompt templates, or approval policies [obs: en-019d970f-311e-78d1-ab76-cce58a36ae1e]
- **Option A:** Deploy Cedar/OPA policy-as-code gates on the CI pipeline with spend controls and compliance API integration. — **Tradeoff:** 3-6 engineering-weeks for initial deployment, requires dedicated platform engineer. Creates the control-tower foundation [obs: en-019d970f-3139-7350-a020-5fca7cd98766].
- **Option B:** Expand existing platform-engineering roles to include agent governance. — **Tradeoff:** Role redefinition + 1-2 weeks training per engineer. Lower organizational disruption but weaker enforcement.
- **Option C:** Defer formalization; let agent coordination emerge organically. — **Tradeoff:** Near-zero upfront cost. Risk: DORA's amplification effect causes coordination-cost overshoot [obs: en-019d970f-3112-7a60-b9d7-276ac8db1e66].
- **Recommended:** Option A for organizations with 50+ engineers using agent workflows; Option B for smaller teams. The governance bottleneck is real and compounding.

---

### Assumptions & Limitations

**Assumption 1: Model capability is sufficient and no longer the primary bottleneck for directed software evolution.**
- *If wrong:* A major model regression or pricing shock could re-establish model capability as the binding constraint, invalidating the governance-first thesis.
- *Confidence:* High (80%). SWE-Bench Pro and vendor launches confirm current models are "good enough" for bounded coding tasks [obs: en-019d970f-639e-7370-9b86-d3932886880b].

**Assumption 2: Enterprise security teams will gate agent autonomy expansion around deployment, secrets, and production data.**
- *If wrong:* If competitive pressure causes enterprises to relax gates faster than trust infrastructure matures, adoption could accelerate with higher incident risk.
- *Confidence:* High (75%). Microsoft's MCP security guidance and the control-plane compromise evidence [obs: en-019d970f-6391-79c1-a767-f8478d5d0a9c] indicate the security community is actively constraining agent autonomy.

**Assumption 3: The safety-liveness asymmetry is a persistent structural feature, not a temporary gap.**
- *If wrong:* Novel evaluation methods (formal verification of agent intent, causal impact analysis) could close the gap, enabling faster autonomy expansion.
- *Confidence:* Medium-High (65%). The asymmetry is rooted in fundamental measurement difficulty [obs: en-019d970f-63b9-7870-be30-d1cd6a6bbefa].

---

### Methodology

- **3 independent probes** per step (practitioner, critic, adjacent-domain), each with web search
- **1 projection step** at 365 days, with temporal progression derived from analysis
- **13 total observations**, 3 active directions
- **Orchestrator status:** The orchestrator session failed before reaching convergence/synthesis (session timeout 900s). Synthesis assembled from engine-produced observations and directions following the SKILL.md template structure. Observation deduplication was added to SKILL.md but never executed due to the orchestrator failure.
- **External evidence sources cited:** The New Stack (AI agent validation bottleneck), Aether (isolated VM agent execution), Harness AI PR Agents, OpenAI Codex MCP documentation, Microsoft MCP security guidance, MCP Security Research, SWE-Bench Pro (Scale AI), MSR "Comparing AI Coding Agents" (task-stratified analysis), DORA 2025 AI-assisted development report, Anthropic Claude Code Enterprise, Agent CI (agent-ci.com)
