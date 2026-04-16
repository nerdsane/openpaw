## Foresight Projection: Directed Software Evolution — April 2026 to April 2027

---

### Executive Summary

Directed Software Evolution over the next 12 months will be shaped by a three-way tension between rapidly maturing agent tooling, structurally immature trust infrastructure, and under-designed organizational coordination. On the tooling front, the landscape has consolidated fast: GitHub shipped an asynchronous coding agent inside Copilot positioned as a configurable, steerable loop embedded in existing developer workflows [obs: en-019d96c8-6c59-77f2-b638-f47eafd6fbee], VS Code and Harness standardized tool access through full MCP specification support [obs: en-019d96c8-6c65-7462-8407-a79a48c4a2a3], and OpenAI released Codex CLI as an open-source local agent installable via npm and Homebrew [obs: en-019d96c8-6c71-7bb3-853e-0b4db03f7bd9]. These moves make it technically feasible to run bounded agent loops where tests, policy, and approvals already exist. But feasibility is not readiness.

The binding constraint is not model capability — it is trust architecture and organizational design. MCP authorization is still normalizing around OAuth client registration, with churn in discovery and dynamic registration paths that leaves machine-to-machine trust boundaries immature [obs: en-019d96c8-926f-72b3-98f1-858a5a5d888f]. OWASP ranks prompt injection as the top LLM risk, and every artifact in a directed-evolution harness loop — specs, tickets, code comments, retrieved docs — becomes a potential hostile control channel [obs: en-019d96c8-927c-78a3-a224-1cb1b9a701a6]. NIST's AI Agent Standards Initiative is still organizing at the governance layer rather than declaring mature operational trust primitives [obs: en-019d96c8-9292-7f73-8d88-743d00768948]. Meanwhile, DORA's 2025 research frames AI as an amplifier of existing organizational strengths and weaknesses, not a universal productivity layer, meaning that firms with weak coordination will see agentic tooling amplify their dysfunction before it helps [obs: en-019d96c9-8513-7fd0-987e-8530f98276c9].

The teams that capture value first will be those that invest in harness quality (machine-checkable acceptance criteria, reproducible sandboxes, repo-specific evals), narrow tool scopes with adversarial testing, and redesign coordination structures — formalizing review gateways, task decomposition rules, and the emerging role of workflow designers who maintain prompt/policy assets and tune the boundary between human approval and agent autonomy [obs: en-019d96c8-6c7d-7862-a56c-210563f8f6e6] [obs: en-019d96c9-8507-7fa0-99a4-418658c68c40]. The next year is not about who deploys the most autonomous agents; it is about who builds the tightest selection environment around them.

---

### Key Findings

**1. Platform owners are embedding agent loops into existing developer control planes, not creating new standalone platforms.**
GitHub's Copilot coding agent attaches selection pressure directly to issues, PRs, code review, Actions, and security controls [obs: en-019d96c8-6c59-77f2-b638-f47eafd6fbee]. This means directed evolution will spread through workflow surfaces developers already trust rather than through net-new autonomous platforms.
*Theme: Platform integration*

**2. MCP is consolidating as the tool-interoperability standard, enabling portable harness-aware agent stacks.**
VS Code shipped full MCP spec support (June 12, 2025), and Harness launched an MCP server for DevOps workflows (May 29, 2025), lowering integration cost for stage-2 and stage-3 directed-evolution behaviors [obs: en-019d96c8-6c65-7462-8407-a79a48c4a2a3]. Teams that ignore this protocol layer will overfit to one vendor and pay expensive reintegration costs later.
*Theme: Standards convergence*

**3. Local-agent packaging is making verification-friendly adoption more likely than pure cloud delegation.**
OpenAI's Codex CLI (April 16, 2025) and similar local agents run inside existing terminals and CI sandboxes, making permissions, diffs, test runs, and rollback easier to constrain [obs: en-019d96c8-6c71-7bb3-853e-0b4db03f7bd9]. This is a concrete step toward harness-first engineering.
*Theme: Verifiability*

**4. Harness quality matters more than model quality once candidate generation is cheap.**
Adding another top-tier model yields smaller gains than adding repo-specific evals, deterministic setup scripts, reproducible fixtures, and machine-readable completion criteria [obs: en-019d96c8-6c7d-7862-a56c-210563f8f6e6]. Teams treating agents as high-throughput hypothesis generators under strong harnesses will outperform those treating them as autonomous engineers.
*Theme: Selection over generation*

**5. MCP authorization infrastructure is running behind protocol interoperability, creating a trust gap.**
Client registration paths are churning (OAuth discovery, Dynamic Client Registration deprecation), and many deployments ship with ad hoc preregistration [obs: en-019d96c8-926f-72b3-98f1-858a5a5d888f]. Tool invocation success is being mistaken for trustworthy authorization.
*Theme: Trust deficit*

**6. The harness itself is the primary attack surface for directed-evolution systems.**
OWASP LLM01:2025 frames prompt injection as the top risk; every artifact in the harness loop (specs, tickets, docs, code comments) is a potential hostile control channel [obs: en-019d96c8-927c-78a3-a224-1cb1b9a701a6]. Expect at least one high-visibility incident where an agent follows injected instructions while still passing local tests.
*Theme: Adversarial harness risk*

**7. Safety checks scale faster than proofs of useful progress — creating a verification asymmetry.**
Teams can prove more things an agent must NOT do than that it is reliably improving architecture, incident rates, or user value [obs: en-019d96c8-9292-7f73-8d88-743d00768948]. Selection pressure may optimize for easy-to-measure local metrics (test pass rate, latency) while degrading system comprehensibility and long-horizon maintainability.
*Theme: Safety-liveness asymmetry*

**8. Managerial span-of-control, not model capability, is the limiting factor.**
When agents draft code, tests, and issue updates, the scarce role becomes architectural review, policy setting, and exception handling by senior engineers [obs: en-019d96c9-84e3-7ea3-8a5f-9ddc163ede76]. Serious adopters will add review gateways and task decomposition rules before adding more agents.
*Theme: Coordination bottleneck*

**9. Directed evolution amplifies senior engineers and platform teams faster than it replaces junior headcount.**
Anthropic's Economic Index shows high model exposure in programming work, but exposure ≠ substitution; it increases the premium on people who judge outputs, define interfaces, and maintain evaluation loops [obs: en-019d96c9-84ee-7e63-b849-99c34bb878ab]. Cutting junior pipelines too early weakens the future bench for supervising agentic workflows.
*Theme: Labor market recomposition*

**10. More autonomy may briefly reduce effective throughput in mixed-maturity organizations.**
DORA 2025 emphasizes AI amplifies existing strengths and weaknesses. Unmanaged automation raises coordination load (excess diffs, merge conflicts, review fatigue) faster than it reduces coding time [obs: en-019d96c9-8513-7fd0-987e-8530f98276c9]. Some firms will quietly narrow agent permissions after discovering this.
*Theme: Coordination cost overshoot*

**11. Enterprise adoption is gated by governance and incentive design, not demo impressiveness.**
Many pilots will remain confined to documentation, tests, migrations, and internal tooling because security review, secrets handling, auditability, and approval boundaries are immature [obs: en-019d96c9-84fb-7a31-9cb9-b2248cff4574]. More governed competitors will win procurement cycles.
*Theme: Governance-gated adoption*

**12. The emerging high-leverage role is workflow design, not coding speed.**
Organizations using directed evolution seriously will standardize task templates, test coverage thresholds, and repository policies, rewarding people who define evaluation harnesses, maintain prompt/policy assets, and tune human-agent approval boundaries [obs: en-019d96c9-8507-7fa0-99a4-418658c68c40].
*Theme: Role evolution*

---

### Temporal Progression

#### Phase 1: 0–3 months (April – July 2026)
**What changes:**
- GitHub Copilot's coding agent reaches GA-level availability for enterprise customers; teams begin piloting issue-to-branch and failing-test-to-candidate-fix loops inside existing CI/CD [obs: en-019d96c8-6c59-77f2-b638-f47eafd6fbee].
- MCP adoption crosses a critical threshold: major IDEs (VS Code confirmed), CI platforms (Harness confirmed), and at least 2 additional DevOps vendors ship MCP servers, creating a portable tool-contract layer [obs: en-019d96c8-6c65-7462-8407-a79a48c4a2a3].
- Local agent usage (Codex CLI, Claude Code, similar tools) becomes the default entry point for security-conscious teams because local execution provides inherent sandboxing [obs: en-019d96c8-6c71-7bb3-853e-0b4db03f7bd9].
- Early-adopter teams discover that harness investment (repo-specific evals, deterministic fixtures, machine-readable completion criteria) yields larger gains than model upgrades [obs: en-019d96c8-6c7d-7862-a56c-210563f8f6e6].

**Signals expected:**
- Conference talks and blog posts shift from "AI wrote my code" to "how we evaluate AI-generated PRs."
- At least one prominent open-source project publishes agent-specific contribution guidelines (sandbox requirements, test mandates, PR templates for agent-generated patches).
- MCP server ecosystem grows from ~dozens to ~hundreds of registered servers.

**What has NOT changed:**
- Production deployment authority remains human-gated for most organizations [obs: en-019d96c8-9287-7650-99c6-4b0bcfbf34f0].
- MCP authorization remains fragile; most deployments use preregistered clients rather than robust dynamic registration [obs: en-019d96c8-926f-72b3-98f1-858a5a5d888f].
- Junior developer hiring has not been materially affected yet [obs: en-019d96c9-84ee-7e63-b849-99c34bb878ab].

**Causal links to Phase 2:**
- The harness-quality gap between leading and lagging teams becomes visible in measurable PR merge rates and defect rates, creating organizational pressure to invest in evaluation infrastructure.
- At least one high-visibility prompt-injection incident in an agent-assisted repository catalyzes demand for adversarial harness testing [obs: en-019d96c8-927c-78a3-a224-1cb1b9a701a6].

---

#### Phase 2: 3–6 months (July – October 2026)
**What changes:**
- Trust architecture becomes the dominant enterprise procurement criterion for agentic coding tools. Vendors that ship least-privilege tool scoping, memory isolation, human-in-the-loop approval for sensitive actions, and monitoring dashboards gain market share [obs: en-019d96c8-9287-7650-99c6-4b0bcfbf34f0].
- MCP authorization stabilizes: the community converges on a default OAuth flow with Client ID Metadata Documents, reducing ad hoc preregistration [obs: en-019d96c8-926f-72b3-98f1-858a5a5d888f].
- Organizations begin formalizing the "workflow designer" role — the person who defines evaluation harnesses, maintains prompt/policy assets, and tunes the human-agent approval boundary [obs: en-019d96c9-8507-7fa0-99a4-418658c68c40].
- Some firms that aggressively expanded agent autonomy in Phase 1 quietly narrow permissions after experiencing coordination-cost overshoot (excess diffs, merge conflicts, review fatigue) [obs: en-019d96c9-8513-7fd0-987e-8530f98276c9].

**Signals expected:**
- At least 2 major enterprises publish post-mortems or case studies on agent-coordination failures.
- NIST or ISO publishes a draft framework for AI agent operational trust, moving from organizing to specifying [obs: en-019d96c8-9292-7f73-8d88-743d00768948].
- Job postings for "AI workflow engineer," "agent evaluation lead," or similar roles appear on major platforms.
- Platform vendors begin bundling adversarial testing (red-teaming for prompt injection in repo context) as a feature.

**What has NOT changed:**
- Fully autonomous deployment (agent writes, tests, deploys, monitors without human approval) remains rare outside low-risk internal tooling.
- The safety-liveness asymmetry persists: teams can constrain agents effectively but still struggle to prove agents are improving system-level quality [obs: en-019d96c8-9292-7f73-8d88-743d00768948].

**Revisions to earlier predictions:**
- MCP adoption may be faster than expected if GitHub and GitLab ship first-party MCP servers, reducing the integration burden below Phase 1 projections.
- The prompt-injection incident predicted for Phase 1 may be delayed if early adopters keep agent scopes narrow enough to prevent high-impact failures.

**Causal links to Phase 3:**
- Stabilized authorization and adversarial-testing tooling enable a new class of semi-autonomous workflows: agents with broader tool access but within policy-enforced guardrails.
- The formalized workflow-designer role creates demand for training, certification, and tooling specific to evaluation-harness engineering.

---

#### Phase 3: 6–9 months (October 2026 – January 2027)
**What changes:**
- Stage-2 "dark factory" patterns become operational at leading firms: agents handle end-to-end loops (issue triage → code generation → test → review request → deployment staging) for well-defined task types, with human approval only at deployment gates [obs: en-019d96c8-6c59-77f2-b638-f47eafd6fbee] [obs: en-019d96c8-6c7d-7862-a56c-210563f8f6e6].
- The labor-market segmentation effect becomes measurable: senior engineers, platform teams, and DevEx specialists command higher premiums; some organizations begin restructuring junior onboarding to emphasize agent-supervision skills [obs: en-019d96c9-84ee-7e63-b849-99c34bb878ab].
- Harness-as-product emerges: open-source and commercial products specifically focused on agent evaluation, selection, and policy enforcement gain traction, separate from the agents themselves.
- Enterprise governance maturity separates leaders from laggards: firms with robust audit, approval, and secrets-handling workflows expand agent scope to production-adjacent systems; firms without these stay confined to internal tooling and documentation [obs: en-019d96c9-84fb-7a31-9cb9-b2248cff4574].

**Signals expected:**
- At least one major tech company reports measurable developer productivity improvements attributed to harness-bound agent workflows (not raw model capability) in earnings calls or engineering blog posts.
- Universities and bootcamps begin offering coursework on agent evaluation and workflow design.
- Open-source harness frameworks (analogous to testing frameworks but for agent-generated code evaluation) reach 1,000+ GitHub stars.

**What has NOT changed:**
- Stage-4 evolutionary behavior (agents improving their own selection harnesses) remains experimental and confined to research settings.
- The verification asymmetry persists: proving agents are NOT doing harm remains easier than proving they are doing good [obs: en-019d96c8-9292-7f73-8d88-743d00768948].

**Revisions to earlier predictions:**
- If the Phase 2 authorization stabilization happens faster, semi-autonomous workflows may reach Phase 3 maturity earlier, compressing the timeline.
- If DORA's "AI amplifies dysfunction" finding proves stronger than expected, some organizations may be in active agent rollback rather than expansion [obs: en-019d96c9-8513-7fd0-987e-8530f98276c9].

**Causal links to Phase 4:**
- Accumulated harness quality and operational trust from Phases 1–3 create the preconditions for selective stage-3 machine-tool construction patterns.
- The formalized workflow-designer role and harness-as-product ecosystem lower the barrier for new entrants to adopt directed evolution without repeating early-adopter mistakes.

---

#### Phase 4: 9–12 months (January – April 2027)
**What changes:**
- Directed software evolution enters mainstream enterprise planning as a recognized operational capability with known maturity stages, not a speculative technology category.
- Stage-3 construction patterns — agents composing multi-service changes across repositories using standardized MCP tool contracts and policy-enforced authorization — become operational at 5+ publicly visible organizations [obs: en-019d96c8-6c65-7462-8407-a79a48c4a2a3].
- The trust gap narrows but does not close: NIST or equivalent bodies publish v1 operational trust standards for AI agents, and at least 2 major cloud providers offer compliance-certified agent execution environments [obs: en-019d96c8-9292-7f73-8d88-743d00768948].
- Organizational design for agentic workflows becomes a recognized management discipline: span-of-control calculations, review-capacity planning, and task-decomposition frameworks are codified in engineering management literature [obs: en-019d96c9-84e3-7ea3-8a5f-9ddc163ede76].

**Signals expected:**
- Industry analysts (Gartner, Forrester) publish maturity models specifically for directed software evolution.
- At least one acquisition in the agent-evaluation/harness space signals market validation.
- Measurable divergence in engineering productivity between harness-mature and harness-immature organizations becomes a cited competitive factor.

**What has NOT changed:**
- Fully autonomous, self-improving software systems (stage-4 evolutionary behavior) remain aspirational. No organization has demonstrated reliable self-modification of selection harnesses by agents.
- Prompt injection and context poisoning remain open problems, though mitigated by narrower tool scopes and adversarial testing [obs: en-019d96c8-927c-78a3-a224-1cb1b9a701a6].

**Revisions to earlier predictions:**
- If the labor-market recomposition effect from Phase 3 proves more disruptive than expected, regulatory or political responses may introduce new constraints on agent autonomy.
- If trust architecture matures faster than projected (driven by competitive pressure from governed competitors winning procurement), stage-3 patterns could be more widespread than the 5+ organizations projected here.

---

### Active Directions

**Direction 1: The next 90 days will reward teams that treat coding agents as harness-bound workflow components, not autonomous replacements for engineers.**

*Full reasoning:* The most likely 90-day trajectory is a rapid shift from standalone coding demos toward harness-centered agent workflows embedded in existing developer control planes. The external evidence points the same way: GitHub is shipping an asynchronous coding agent inside Copilot and pairing it with workflow surfaces developers already trust, while VS Code and vendors such as Harness are standardizing the tool boundary through MCP. OpenAI's open-source Codex CLI further strengthens the local, inspectable execution path. Together these moves make it technically feasible to run agents where tests, policy, telemetry, and approvals already exist. That is the operational substrate directed software evolution needs.

What practitioners will actually adopt first is not open-ended self-improving software, but bounded loops: issue triage to draft PR, failing test to candidate fix, dependency bump to verification run, deployment anomaly to proposed rollback or config patch. The winning teams will invest in machine-checkable acceptance criteria, reproducible sandboxes, and repo-specific evals so multiple candidate solutions can be generated and filtered automatically. In other words, the next 90 days favor stage-1 and stage-2 maturation with selective stage-3 construction patterns. Directed evolution advances when the harness becomes a first-class product artifact.

The main risk is category error. If leaders interpret current product launches as proof that autonomy has already been solved, they will give agents tasks without adequate state models, approval boundaries, or regression harnesses. That will produce visible failures and stall adoption. If instead they narrow task scopes and standardize tool/context interfaces, they can accumulate the exact feedback loops required for later stage-4 evolutionary behavior.

*Counterfactual:* If the market instead jumps directly to broad autonomous engineering without stronger harnesses, adoption will outrun governance and trigger a trust backlash that slows the whole category.

*Supporting observations:* en-019d96c8-6c59-77f2-b638-f47eafd6fbee, en-019d96c8-6c65-7462-8407-a79a48c4a2a3, en-019d96c8-6c71-7bb3-853e-0b4db03f7bd9, en-019d96c8-6c7d-7862-a56c-210563f8f6e6

---

**Direction 2: Directed software evolution will be constrained by trust architecture, not model capability, over the next 90 days.**

*Full reasoning:* My strongest thesis is that directed software evolution will hit a trust bottleneck before it hits a capability bottleneck. The field already has enough model quality and tooling to generate patches, wire workflows, and operate inside narrow harnesses, but the external evidence points to unfinished security and governance foundations. MCP authorization is still stabilizing around client registration and discovery conventions, which indicates that machine-to-machine trust boundaries are not yet boring infrastructure. OWASP continues to treat prompt injection, tool abuse, memory poisoning, and approval routing as first-order agent risks, which means the attack surface is not incidental to autonomous engineering systems; it is the central design problem.

In the next 90 days, the winning teams will not be the ones that maximize autonomous patch volume. They will be the ones that aggressively narrow tool scopes, isolate memory, require approval for boundary-crossing actions, and treat every retrieved artifact as hostile until proven otherwise. This directly challenges the dominant narrative that better harnesses alone unlock safe dark factories. Harnesses are necessary, but not sufficient: the harness must itself be policy-aware, identity-aware, and adversarially tested. Without that, directed evolution selects for whatever can satisfy local checks, including exploitably brittle behaviors.

The practical implication is that progress will look slower and more bureaucratic than enthusiasts expect. Enterprises will keep agentic systems in semi-autonomous modes, especially around deployment, secrets, production data, and cross-system writes. That slowdown is not failure; it is the price of converting impressive code generation into operationally trustworthy software evolution.

*Counterfactual:* If this thesis is wrong, autonomy will scale primarily on the strength of generation and test harnesses, and organizations will rapidly relax approval and policy controls without suffering trust-destroying incidents.

*Supporting observations:* en-019d96c8-926f-72b3-98f1-858a5a5d888f, en-019d96c8-927c-78a3-a224-1cb1b9a701a6, en-019d96c8-9287-7650-99c6-4b0bcfbf34f0, en-019d96c8-9292-7f73-8d88-743d00768948

---

**Direction 3: Directed software evolution will advance fastest where firms redesign coordination, not where they simply deploy more autonomous coding tools.**

*Full reasoning:* Directed software evolution will advance fastest where firms redesign coordination rather than merely adding autonomous coding tools. In the next 90 days, the key competitive advantage will come from decomposing work into evaluable units, formalizing approval paths, and building durable interfaces between humans, agents, and repositories. Adjacent fields give a clear analogy: when the cost of production drops, bottlenecks move to selection, governance, and allocation. In biology, selection environments matter as much as raw generative capacity. In economics and organizational design, lower transaction cost exposes hidden coordination cost. That is the shape of the next quarter.

The external signals reinforce this interpretation. Stack Overflow data shows high AI use but weaker trust, which means verification remains central. Anthropic's economic evidence shows software work is highly exposed to AI assistance, but that should be read as job recomposition, not instant full replacement. DORA emphasizes that AI amplifies existing system quality rather than rescuing weak systems by itself. So the strongest thesis is that social and institutional redesign will determine adoption speed: the winners will improve governance, incentives, review capacity, and workflow architecture before they maximize autonomy.

*Counterfactual:* If this thesis is wrong, the next 90 days will show broad durable productivity gains from agentic coding even in weakly governed teams, with little need for new review mechanisms, incentive changes, or approval boundaries.

*Supporting observations:* en-019d96c9-84e3-7ea3-8a5f-9ddc163ede76, en-019d96c9-84ee-7e63-b849-99c34bb878ab, en-019d96c9-84fb-7a31-9cb9-b2248cff4574, en-019d96c9-8507-7fa0-99a4-418658c68c40, en-019d96c9-8513-7fd0-987e-8530f98276c9

---

### What Surprised Us

**1. The harness is the attack surface, not just the safety mechanism.**
The dominant narrative positions harnesses (tests, evals, policy checks) as the solution to agent risk. But OWASP's LLM01:2025 guidance reveals that every artifact in the harness loop — specs, tickets, code comments, retrieved docs — is a potential prompt-injection channel [obs: en-019d96c8-927c-78a3-a224-1cb1b9a701a6]. This inverts the usual framing: the more artifacts you feed into a directed-evolution loop, the larger the adversarial surface. Passing tests does not prove trustworthy behavior when the test context itself can be poisoned. This is a structural challenge that harness-first advocates have not adequately addressed.

**2. More autonomy can reduce throughput in the near term.**
The intuition that "agents = more output" is wrong in organizations with mixed maturity. DORA 2025 explicitly frames AI as an amplifier of existing weaknesses, and the practical consequence is that unmanaged agent autonomy generates excess diffs, parallel work streams, and merge conflicts that increase review fatigue faster than they reduce coding time [obs: en-019d96c9-8513-7fd0-987e-8530f98276c9]. This is a coordination-theory result, not a technical one, and it challenges the assumption that directed evolution can proceed without organizational redesign.

**3. Safety scales; liveness does not.**
Teams can prove more things an agent must NOT do (type errors, test failures, lint violations, policy breaches) than that it IS reliably moving a system toward better architecture or higher user value [obs: en-019d96c8-9292-7f73-8d88-743d00768948]. NIST's AI Agent Standards Initiative confirms this: governance and measurement are still being organized, not codified. The implication is that directed evolution may optimize for local, easy-to-measure metrics while quietly degrading system comprehensibility — a form of evolutionary fitness that does not correspond to human-valued progress.

**4. Protocol interoperability is running ahead of trust infrastructure.**
MCP has won the tool-contract standard battle remarkably fast (VS Code, Harness, and dozens of servers within months), but the authorization layer underneath it is churning: Client ID Metadata Documents, dynamic registration deprecation, inconsistent auth server assumptions [obs: en-019d96c8-926f-72b3-98f1-858a5a5d888f]. Teams are succeeding at tool invocation while failing at trustworthy authorization. This means the capability to compose multi-tool agent stacks exists today, but the governance to make that composition safe does not. Successful tool calls are being mistaken for secure integrations.

---

### Top 5 Predictions with Falsification Criteria

**Prediction 1: By July 2026, GitHub Copilot's coding agent will be the most-used bounded-loop agent in production, measured by PR volume.**

- *Measurable indicator:* GitHub reports agent-generated PR statistics in its next Octoverse report or equivalent public data release, showing Copilot agent PRs exceeding those from any competing platform-integrated agent.
- *Confidence:* High (75%)
- *Falsification:* If GitHub has not shipped GA enterprise availability for the Copilot coding agent by July 2026, or if a competing platform (e.g., GitLab, Cursor, Windsurf) reports higher agent-generated PR volume, this prediction is wrong because it overestimated GitHub's first-mover advantage in the platform-integrated agent space.
- *Supporting observations:* [obs: en-019d96c8-6c59-77f2-b638-f47eafd6fbee], [obs: en-019d96c8-6c7d-7862-a56c-210563f8f6e6]

**Prediction 2: By October 2026, at least one high-visibility security incident will involve an agent executing hostile instructions embedded in repository context (code comments, issue descriptions, or retrieved documentation) while passing all local tests.**

- *Measurable indicator:* A CVE, public post-mortem, or security advisory explicitly attributing a code-level incident to prompt injection via repository artifacts processed by a coding agent.
- *Confidence:* High (70%)
- *Falsification:* If no such incident is publicly reported by October 2026, this prediction is wrong because either (a) early adopters kept agent scopes narrow enough to prevent high-impact failures, or (b) prompt injection in repository context is harder to weaponize than OWASP's risk ranking suggests.
- *Supporting observations:* [obs: en-019d96c8-927c-78a3-a224-1cb1b9a701a6], [obs: en-019d96c8-926f-72b3-98f1-858a5a5d888f]

**Prediction 3: By January 2027, at least 3 Fortune 500 companies will have created a dedicated "AI workflow engineer" or "agent evaluation lead" role with explicit responsibility for harness design and agent-human boundary management.**

- *Measurable indicator:* Public job postings, LinkedIn role announcements, or earnings-call mentions of dedicated agent-workflow roles at named Fortune 500 companies.
- *Confidence:* Medium (60%)
- *Falsification:* If no such roles are publicly visible by January 2027, this prediction is wrong because organizations absorbed agent coordination into existing DevOps or platform-engineering roles rather than creating new dedicated positions, suggesting the coordination bottleneck was less acute than projected.
- *Supporting observations:* [obs: en-019d96c9-8507-7fa0-99a4-418658c68c40], [obs: en-019d96c9-84e3-7ea3-8a5f-9ddc163ede76], [obs: en-019d96c9-84ee-7e63-b849-99c34bb878ab]

**Prediction 4: By October 2026, at least 2 organizations will publicly report narrowing agent permissions or rolling back autonomy scope after experiencing coordination-cost overshoot (increased merge conflicts, review fatigue, or defect rates from unmanaged parallel agent work).**

- *Measurable indicator:* Published blog posts, conference talks, or DORA-affiliated case studies documenting permission narrowing or autonomy rollback attributed to coordination costs.
- *Confidence:* Medium-High (65%)
- *Falsification:* If no such rollback reports surface by October 2026, this prediction is wrong because either (a) firms that expanded autonomy also invested adequately in coordination infrastructure, or (b) the coordination-cost-overshoot effect is weaker than DORA's amplification thesis implies.
- *Supporting observations:* [obs: en-019d96c9-8513-7fd0-987e-8530f98276c9], [obs: en-019d96c9-84e3-7ea3-8a5f-9ddc163ede76]

**Prediction 5: By April 2027, MCP will be the dominant tool-interoperability protocol for coding agents, with 500+ registered MCP servers across CI/CD, project management, observability, and security tooling vendors.**

- *Measurable indicator:* MCP server registry or ecosystem tracker shows 500+ servers; at least 3 of the top 5 CI/CD platforms (GitHub Actions, GitLab CI, Jenkins, CircleCI, Harness) support MCP natively.
- *Confidence:* Medium (55%)
- *Falsification:* If MCP server count is below 200 by April 2027, or if a competing protocol (e.g., OpenAI's tool-call spec, a Google-led alternative) captures more vendor integrations, this prediction is wrong because MCP's early-mover advantage did not translate to ecosystem lock-in, possibly due to the authorization-layer instability identified in the observations.
- *Supporting observations:* [obs: en-019d96c8-6c65-7462-8407-a79a48c4a2a3], [obs: en-019d96c8-926f-72b3-98f1-858a5a5d888f]

---

### Decision Points

**Decision Point 1: When to invest in repo-specific evaluation harnesses vs. relying on vendor-provided defaults**

- *Timing trigger:* When your team's agent-generated PR acceptance rate drops below 60%, or when you observe agents passing tests but producing code that reviewers consistently reject for architectural or maintainability reasons [obs: en-019d96c8-6c7d-7862-a56c-210563f8f6e6] [obs: en-019d96c8-9292-7f73-8d88-743d00768948].
- *Option A:* Build custom repo-specific evals (deterministic setup scripts, reproducible fixtures, machine-readable completion criteria, architecture-level property tests). **Effort:** 2–4 engineering-weeks upfront, ongoing maintenance of ~5 hours/week. **Tools:** Custom CI stages, repo-specific eval scripts, harness frameworks (emerging OSS options).
- *Option B:* Adopt vendor-bundled evaluation (GitHub Copilot's built-in review suggestions, Cursor's inline verification). **Effort:** 1–2 days setup, minimal ongoing maintenance. **Tools:** Vendor IDE extensions and platform features.
- *Option C:* Hybrid — use vendor defaults for low-risk tasks (docs, tests, dependency bumps) and custom harnesses for production-path code. **Effort:** 1–2 engineering-weeks upfront, ~3 hours/week ongoing. **Tools:** Vendor tools + targeted custom evals for high-risk paths.
- *Recommended:* **Option C.** The evidence strongly suggests that harness quality matters more than model quality [obs: en-019d96c8-6c7d-7862-a56c-210563f8f6e6], but building full custom harnesses for all agent work is premature until task scopes stabilize. Start with vendor defaults for bounded tasks and invest custom harness effort where the safety-liveness asymmetry is most acute (production deployments, security-sensitive code, architectural decisions).

**Decision Point 2: When to formalize the "workflow designer" role vs. distributing agent-coordination responsibilities across existing roles**

- *Timing trigger:* When your organization has 3+ engineers spending >20% of their time maintaining agent configurations, prompt templates, evaluation criteria, or approval-boundary policies [obs: en-019d96c9-8507-7fa0-99a4-418658c68c40] [obs: en-019d96c9-84e3-7ea3-8a5f-9ddc163ede76].
- *Option A:* Create a dedicated "AI workflow engineer" role with ownership of harness design, agent policy, and human-agent boundary management. **Effort:** 1 FTE, recruiting or internal transfer. **Tools:** Agent configuration systems, evaluation-harness frameworks, policy-as-code tools (Cedar, OPA).
- *Option B:* Expand existing platform-engineering or DevEx roles to include agent-coordination responsibilities. **Effort:** Role redefinition + 1–2 weeks of training per engineer. **Tools:** Existing platform-eng tooling extended with agent-specific configurations.
- *Option C:* Defer formalization; let agent coordination emerge organically from individual team practices. **Effort:** Near-zero upfront. **Risk:** Inconsistent practices, duplication of effort, coordination-cost overshoot [obs: en-019d96c9-8513-7fd0-987e-8530f98276c9].
- *Recommended:* **Option B for most organizations, Option A for organizations with 50+ engineers using agent workflows.** The evidence suggests the coordination bottleneck is real [obs: en-019d96c9-84e3-7ea3-8a5f-9ddc163ede76], but creating a new role too early risks over-specialization before the discipline is well-defined. Expanding platform-eng roles provides coordination benefits with lower organizational disruption.

**Decision Point 3: When to adopt MCP as the tool-interoperability standard vs. waiting for authorization-layer maturity**

- *Timing trigger:* When your agent workflows need to access 3+ external systems (CI, project management, observability, documentation) and you are building custom integrations for each [obs: en-019d96c8-6c65-7462-8407-a79a48c4a2a3] [obs: en-019d96c8-926f-72b3-98f1-858a5a5d888f].
- *Option A:* Adopt MCP now with hardened OAuth preregistration for all tool servers and aggressive scope-narrowing. Accept the auth-layer churn cost. **Effort:** 1–2 engineering-weeks for initial MCP server setup + OAuth hardening per integration. **Tools:** MCP SDK, OAuth server configuration, scope-narrowing policies.
- *Option B:* Wait for MCP authorization to stabilize (estimated: Q3–Q4 2026 based on spec evolution pace). Continue building vendor-specific integrations in the interim. **Effort:** Higher per-integration cost now, lower migration risk later. **Tools:** Vendor-specific APIs and SDKs.
- *Option C:* Adopt MCP for read-only tool access (querying CI status, reading docs, browsing tickets) but restrict write operations to vendor-specific APIs with established auth. **Effort:** 1 engineering-week for read-only MCP setup. **Tools:** MCP for reads, vendor APIs for writes, policy enforcement at the boundary.
- *Recommended:* **Option C.** MCP's tool-contract value is real and growing fast [obs: en-019d96c8-6c65-7462-8407-a79a48c4a2a3], but the authorization gap is also real [obs: en-019d96c8-926f-72b3-98f1-858a5a5d888f]. Adopting MCP for reads captures the portability benefit while containing the trust risk. Expand to writes when auth stabilizes or when your OAuth hardening is independently audited.

---

### Assumptions & Limitations

**Assumption 1: Model capability is sufficient and no longer the primary bottleneck for directed software evolution.**
- *If wrong:* A major model regression (quality drop, API instability, pricing shock) or a failure by frontier labs to maintain current code-generation quality could re-establish model capability as the binding constraint, invalidating the trust-and-coordination-first thesis.
- *Confidence:* High (80%). Multiple independent signals (GitHub Copilot GA, Codex CLI, Anthropic's economic data) confirm that current models are "good enough" for bounded coding tasks [obs: en-019d96c8-6c7d-7862-a56c-210563f8f6e6] [obs: en-019d96c9-84ee-7e63-b849-99c34bb878ab].

**Assumption 2: Enterprise security teams will gate agent autonomy expansion around deployment, secrets, and production data access.**
- *If wrong:* If competitive pressure causes enterprises to relax security gates faster than trust infrastructure matures, adoption could accelerate dramatically but with higher incident risk. The trust-backlash scenario from Direction 2 would become more likely.
- *Confidence:* High (75%). OWASP's agent security cheat sheet and the NIST standards initiative both indicate the security community is actively working to constrain agent autonomy, and enterprise security teams historically prioritize caution [obs: en-019d96c8-9287-7650-99c6-4b0bcfbf34f0] [obs: en-019d96c8-9292-7f73-8d88-743d00768948].

**Assumption 3: MCP will consolidate as the dominant tool-interoperability protocol rather than fragmenting into competing standards.**
- *If wrong:* If Google, Amazon, or a consortium launches a competing protocol with stronger authorization defaults, the ecosystem could fragment, increasing integration costs and slowing the assembly of harness-aware agent stacks. Teams that invested early in MCP would face migration costs.
- *Confidence:* Medium (55%). MCP has strong early momentum (VS Code, Harness, Anthropic backing), but the authorization instability [obs: en-019d96c8-926f-72b3-98f1-858a5a5d888f] and the possibility of vendor-specific alternatives create meaningful fragmentation risk [obs: en-019d96c8-6c65-7462-8407-a79a48c4a2a3].

**Assumption 4: The safety-liveness asymmetry is a persistent structural feature, not a temporary gap that will close with better tooling.**
- *If wrong:* If novel evaluation methods (formal verification of agent intent, causal impact analysis, system-level property testing) mature faster than expected, teams could prove that agents are improving long-horizon system quality, not just satisfying local metrics. This would accelerate the transition to higher-autonomy operating modes.
- *Confidence:* Medium-High (65%). The asymmetry is rooted in fundamental measurement difficulty (proving system improvement is harder than proving constraint satisfaction), which suggests it will persist even as tooling improves [obs: en-019d96c8-9292-7f73-8d88-743d00768948].

---

### Methodology

- **3 independent probes** with web search, each covering a distinct analytical lens:
  - **Probe 1** (aj-019d96c7-27a6-7c13-8741-0a66157bb3b4): Practitioner/tooling perspective — examined platform launches, protocol standards, and local-agent packaging.
  - **Probe 2** (aj-019d96c7-27b8-75c2-8665-e3398741ea9f): Security/trust critic perspective — examined MCP authorization maturity, prompt injection risks, operational trust gaps, and verification asymmetry.
  - **Probe 3** (aj-019d96c7-27ca-7411-932c-a3431cc534df): Adjacent-domain perspective — applied organizational theory, labor economics, and coordination science to agentic coding adoption.
- **1 projection step** at 90 days, extended to 12 months through temporal progression analysis.
- **13 total observations**, 3 active directions.
- **External evidence sources cited:** GitHub newsroom (May 19, 2025), VS Code blog (June 12, 2025), Harness blog (May 29, 2025), OpenAI Codex CLI README, Den Delimarsky's MCP auth spec review (November 2025), OWASP LLM01:2025, OWASP AI Agent Security Cheat Sheet, NIST AI Agent Standards Initiative, Stack Overflow Developer Survey 2024, Anthropic Economic Index, DORA State of AI-assisted Software Development 2025.
- **Analytical biases acknowledged:** The probe structure emphasizes trust and coordination constraints, which may underweight the speed at which competitive pressure drives organizations to accept higher risk. The 90-day projection horizon was extended to 12 months through causal reasoning rather than additional web-sourced evidence, which increases uncertainty in later phases.