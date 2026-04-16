# Foresight Projection: Directed Software Evolution v2

## Executive Summary

Directed Software Evolution did not converge on a single magical autonomous IDE. It converged on a governed stack in which **OpenAI Codex**, **Anthropic Claude Code**, and **GitHub Copilot coding agent** are used inside bounded repositories, explicit test harnesses, and pull-request workflows rather than as free-form replacements for engineering organizations [obs: en-019d9885-6e93-7642-a0e8-20c5f1459282] [obs: en-019d9885-6ea9-7eb0-967a-47e0857e5834] [obs: en-019d9888-9c12-7be1-a788-b343926a0290] [obs: en-019d9888-9c4b-7e70-935f-008938f500c5]. The strongest quantitative signal is that maintainability and mergeability remain materially below benchmark theater: **roughly half** of SWE-bench-passing PRs in METR's cited critique would still not be merged, so passing tests and passing governance are diverging realities [obs: en-019d9885-c7c9-71d0-acf8-7171b627b77f]. Over the year, the category's center of gravity shifted from "can models generate code?" to "can organizations metabolize generated change safely and cheaply?" [obs: en-019d9885-6e9f-7820-9aee-6c49e4815340] [obs: en-019d9885-abbf-79c0-b788-dcf23f16429e] [obs: en-019d9888-9c26-7893-9b2a-a67efaddc00c].

The projection also shows that **SWE-bench**, **OWASP GenAI Top 10**, and **METR** become strategic reference points for opposite reasons: SWE-bench remains a useful but narrow language for capability claims; OWASP makes prompt injection impossible to dismiss as an edge case; and METR's long-task framing makes it harder to claim that rising benchmark curves imply dark-factory software production [obs: en-019d9885-c7c9-71d0-acf8-7171b627b77f] [obs: en-019d9889-2829-7921-917a-2ba1fa5a649a] [obs: en-019d9889-2846-7a52-be63-e9461853926b] [obs: en-019d9889-2837-7152-873e-ef61442f34ef]. The practical outcome is a two-lane operating model: local terminal agents handle exploration and patch shaping while cloud agents process bounded backlog items in isolated sandboxes with tests, logs, and review artifacts attached [obs: en-019d9888-9c12-7be1-a788-b343926a0290]. Quantitatively, adoption moves where organizations can bound work into **low-to-medium complexity tasks**, keep reviewable evidence, and narrow permissions; it stalls where tasks exceed current long-horizon reliability or where untrusted text can steer privileged tools [obs: en-019d9888-9c4b-7e70-935f-008938f500c5] [obs: en-019d9889-2829-7921-917a-2ba1fa5a649a] [obs: en-019d9889-2846-7a52-be63-e9461853926b].

A third conclusion is organizational: the best analogy by year one is not an autonomous swarm but a layered institution. **OrgAgent**, Deloitte's orchestration framing, and the adjacent-domain ecology metaphor all point to the same structure: routing, review, and compliance become distinct layers; verification capacity becomes the carrying capacity of the system; and scarce human labor shifts toward decomposition, acceptance criteria, and exception handling rather than raw typing [obs: en-019d9885-aba0-7072-9dec-7f306a3605cc] [obs: en-019d9885-abaa-7ca1-aa46-b71f3d07d678] [obs: en-019d9885-abbf-79c0-b788-dcf23f16429e] [obs: en-019d9889-183a-72d3-b9cc-ef4e147da5d1] [obs: en-019d9889-1844-7723-97fc-6d016733c5f8] [obs: en-019d9889-184e-7690-a92b-9831bdc035eb]. The result is not zero-human software production; it is governed acceleration. Firms that deploy policy gates, provenance, and deterministic verification packages can convert abundant code generation into compounding throughput, while firms that chase autonomy headlines without those layers absorb security risk, reviewer fatigue, and budget backlash [obs: en-019d9885-c7d2-7823-ab5b-6356248c1438] [obs: en-019d9889-2853-74b2-b1e4-ba14b25046eb] [obs: en-019d9889-2863-7ea3-8cdc-9206022bb936].

## Key Findings

1. **OpenAI Codex and GitHub Copilot coding agent normalize a two-lane architecture instead of one autonomous IDE monopoly.**
   - Evidence: "By day 365, the default architecture for directed software evolution in serious teams is not a monolithic autonomous IDE agent but a two-lane stack: local terminal agents for interactive exploration plus cloud task agents for queued implementation." [obs: en-019d9888-9c12-7be1-a788-b343926a0290]
   - Measurable indicator: 2 distinct execution lanes become standard in platform design reviews by Q2 2026.
   - Theme: technical architecture

2. **Anthropic Claude Code makes verification packaging the highest-leverage investment, not prompt polish.**
   - Evidence: "The highest-leverage technical investment over the year becomes verification packaging, not raw agent prompting." [obs: en-019d9888-9c26-7893-9b2a-a67efaddc00c]
   - Measurable indicator: every agent-enabled repo should ship at least 4 packaged verification assets by year-end: instructions, deterministic setup, smoke tests, and fixture-backed evals.
   - Theme: evaluation/testing

3. **OWASP GenAI Top 10 keeps prompt injection on the critical path for coding-agent rollout.**
   - Evidence: "A year in, the field still has not neutralized prompt-injection as a routine failure mode for coding agents with tool access." [obs: en-019d9889-2829-7921-917a-2ba1fa5a649a]
   - Measurable indicator: 0 high-privilege write or fetch tools should execute without provenance checks and narrowed permissions in regulated deployments.
   - Theme: governance/policy

4. **SWE-bench remains useful, but METR's finding that roughly half of benchmark-passing PRs would not merge breaks single-number vendor narratives.**
   - Evidence: "METR's March 10, 2026 note ... reports that roughly half of test-passing SWE-bench Verified PRs from mid-2024 to mid/late-2025 agents would not be merged by maintainers." [obs: en-019d9885-c7c9-71d0-acf8-7171b627b77f]
   - Measurable indicator: ~50% non-merge rate for benchmark-passing PRs is the threshold that forces procurement teams to demand repo-specific acceptance metrics.
   - Theme: model/vendor

5. **Deloitte's orchestration lens becomes operational reality: routing and approval latency matter as much as model quality.**
   - Evidence: "Economic selection pressure is shifting Directed Software Evolution from a tooling story to a coordination-cost story." [obs: en-019d9885-aba0-7072-9dec-7f306a3605cc]
   - Measurable indicator: organizations track at least 3 coordination metrics—routing latency, approval latency, and verification queue depth—alongside model evals.
   - Theme: economics/market

6. **OrgAgent-style hierarchy predicts the year-one operating model better than flat-agent rhetoric.**
   - Evidence: "the most credible near-term architecture is not a flat swarm but a layered firm: governance, execution, and compliance will separate as distinct functions." [obs: en-019d9885-abaa-7ca1-aa46-b71f3d07d678]
   - Measurable indicator: 3 distinct control layers—allocation, execution, compliance—appear in successful platform designs.
   - Theme: cross-domain

7. **Verification becomes the ecosystem carrying capacity, not a downstream hygiene task.**
   - Evidence: "A new ecological constraint is becoming legible: verification, not generation, is the carrying capacity of the ecosystem." [obs: en-019d9885-abbf-79c0-b788-dcf23f16429e]
   - Measurable indicator: candidate-change throughput must stay below verification throughput; a sustained queue growth ratio above 1.0 signals instability.
   - Theme: cross-domain

8. **Developer resistance to opaque agent diffs makes inspectability a year-one adoption gate.**
   - Evidence: "engineering organizations increasingly resist opaque agent decision-making even when aggregate throughput improves." [obs: en-019d9889-2863-7ea3-8cdc-9206022bb936]
   - Measurable indicator: if review time per agent PR exceeds human-authored PR review time by 25% for two quarters, adoption stalls.
   - Theme: organizational/adoption

## Temporal Progression

### 0-3 months
**OpenAI Codex CLI** and other terminal-native agents become the fastest path into production because they fit existing least-privilege workflows, repository boundaries, and local test commands [obs: en-019d9885-6e93-7642-a0e8-20c5f1459282]. In the same window, teams discover that the real shipping pattern is a verification cascade—schema checks, static analysis, tests, sandbox runs, then human approval—rather than a single autonomous loop [obs: en-019d9885-6e9f-7820-9aee-6c49e4815340]. Early adopters also learn that liveness instrumentation is weaker than safety instrumentation, so agents can be well-contained without being reliably useful on long trajectories [obs: en-019d9885-6eb3-7ff3-a129-6101ae4e0d8c].

### 3-6 months
**Anthropic Claude Code** normalizes repo-resident instruction files, reusable skills, and governed machine-tool primitives, pushing teams to check operational context into version control instead of hiding it in prompts or tribal memory [obs: en-019d9885-6ea9-7eb0-967a-47e0857e5834] [obs: en-019d9888-9c39-7e80-9481-be0653b4c1d4]. At the same time, economic and organizational pressures make routing and review architecture visible as a first-order design variable rather than support plumbing [obs: en-019d9885-aba0-7072-9dec-7f306a3605cc] [obs: en-019d9885-abaa-7ca1-aa46-b71f3d07d678].

#### Revisions to earlier predictions
The early belief that more orchestration automatically means more progress is revised downward. Shared artifacts—eval traces, issue state, failing tests, and policy marks—start to outperform thick manager-agent layers in some contexts, so teams simplify central planning and invest more in environmental feedback [obs: en-019d9885-abb4-7fe0-be9d-1cf487b20875].

### 6-9 months
**GitHub Copilot coding agent** helps institutionalize remote execution for bounded backlog items in secure cloud environments with tests and linters attached, reinforcing the split between interactive local agents and queued background workers [obs: en-019d9888-9c12-7be1-a788-b343926a0290]. During this phase, benchmark backlash becomes visible: teams realize that SWE-bench and similar scores cannot stand in for multi-repo coordination, ambiguous requirements, or maintainability review, so internal acceptance metrics start to replace leaderboard talk in serious buying decisions [obs: en-019d9889-2837-7152-873e-ef61442f34ef] [obs: en-019d9885-c7c9-71d0-acf8-7171b627b77f].

#### Revisions to earlier predictions
Predictions of flat autonomous swarms are revised toward layered institutions. The ecology and public-health analogies become more predictive than swarm rhetoric: verification, provenance, and policy gates cap real throughput, and platform owners gain leverage through control of trust boundaries and acceptance systems [obs: en-019d9889-182f-7830-8b67-a02de6be767c] [obs: en-019d9889-183a-72d3-b9cc-ef4e147da5d1] [obs: en-019d9889-1844-7723-97fc-6d016733c5f8].

### 9-12 months
By year-end, **OWASP GenAI Top 10** and **METR** become boardroom references because prompt injection and long-task reliability define the category's practical ceiling more than raw model announcements do [obs: en-019d9889-2829-7921-917a-2ba1fa5a649a] [obs: en-019d9889-2846-7a52-be63-e9461853926b]. Governance overhead is now understood as infrastructure, not enterprise red tape, and labor markets bifurcate toward foremen, decomposers, evaluators, and control-plane designers instead of uniform developer replacement [obs: en-019d9889-2853-74b2-b1e4-ba14b25046eb] [obs: en-019d9889-184e-7690-a92b-9831bdc035eb].

#### Revisions to earlier predictions
The strongest year-end revision is that dark-factory narratives lose credibility. Capability improves, but the category consolidates around tightly governed augmentation stacks because security pressure, review fatigue, and benchmark skepticism prove more durable than autonomy marketing assumed [obs: en-019d9889-2846-7a52-be63-e9461853926b] [obs: en-019d9889-2863-7ea3-8cdc-9206022bb936] [obs: en-019d9888-9c4b-7e70-935f-008938f500c5].

## Active Directions

#### Near-term adoption will favor harnessed machine-tool platforms over unrestricted autonomous coders.
**Direction ID:** en-019d9885-6ebd-7e30-bfd6-541628591258

The strongest practical direction for the next 90 days is that directed software evolution will advance through harnessed machine-tool platforms, not unrestricted autonomous coders. The external evidence points in the same direction: OpenAI is productizing Codex as a local terminal agent with explicit orchestration, guardrails, and eval surfaces, while Anthropic is positioning Claude Code across terminal, IDE, Slack, and web rather than as a hidden fully autonomous backend. That is exactly where practitioners can adopt quickly: inside bounded repositories, approved execution environments, and repeatable deployment paths.

Technically, this favors architectures where models generate candidate actions but platform primitives enforce state transitions, policy checks, and verification cascades. Teams that win will define stable action vocabularies and replayable harnesses first, then let increasingly capable models explore inside those bounds. The counter-position—that better frontier models alone will unlock broad autonomous software evolution in the next quarter—is weaker, because the remaining bottleneck is trustworthy selection and progress measurement. In other words, the near-term compounding advantage belongs to organizations that treat verification, auditability, and deployable action surfaces as product infrastructure, not as after-the-fact safety add-ons.

Supporting observations: [obs: en-019d9885-6e93-7642-a0e8-20c5f1459282], [obs: en-019d9885-6e9f-7820-9aee-6c49e4815340], [obs: en-019d9885-6ea9-7eb0-967a-47e0857e5834], [obs: en-019d9885-6eb3-7ff3-a129-6101ae4e0d8c]

**Counterfactual:** If general autonomous coders become reliably self-directing before harnessed action substrates mature, teams that invested heavily in governed machine-tool layers may appear slower initially; however, absent such a jump, most ungoverned deployments will fail trust and reliability tests.

#### Directed Software Evolution will advance fastest where teams treat harnesses as institutions for selection, not merely tests for generated code.
**Direction ID:** en-019d9885-abca-7900-8d79-f75b9480de20

Across economics, biology, and organizational theory, the strongest thesis is that Directed Software Evolution is about building selective institutions, not just stronger generators. In the next 90 days, the winning systems will be the ones that encode governance, execution, and compliance into the development substrate so that variation can increase without collapsing trust. This extends the model's core claim that rigor and autonomy compound together: the practical compounding mechanism is institutional. Harnesses act like market rules, immune filters, and organizational boundaries at once. They determine which mutations are affordable to test, which are admissible to deploy, and which failures are locally containable.

The crucial implication is that the field should optimize for throughput of selection rather than volume of generation. That means repo-local evals, policy gates, archival memory, audit trails, and artifact-mediated coordination should receive more investment than yet another layer of conversational orchestration. Hierarchical control will appear where accountability is expensive, but beyond a point, systems will evolve toward stigmergic coordination through shared traces and machine-readable constraints. If this thesis is right, Directed Software Evolution will look less like autonomous pair programming and more like an artificial ecosystem with explicit niches, carrying capacities, and continuous selective pressure.

Supporting observations: [obs: en-019d9885-aba0-7072-9dec-7f306a3605cc], [obs: en-019d9885-abaa-7ca1-aa46-b71f3d07d678], [obs: en-019d9885-abb4-7fe0-be9d-1cf487b20875], [obs: en-019d9885-abbf-79c0-b788-dcf23f16429e]

**Counterfactual:** If this direction is wrong, raw model quality and centralized orchestration will dominate, and teams with weaker institutional scaffolding will still achieve reliable autonomous evolution through generation alone.

#### By day 365, the standard architecture is agent-operable CI: repo instructions plus verification harnesses plus sandboxed background executors.
**Direction ID:** en-019d9888-9c5e-7183-be71-1c73f43f74f6

Over the next year, directed software evolution matures into a CI-centered orchestration pattern rather than an autonomous-coder monoculture. The winning stack pairs interactive local agents with remote issue executors, both anchored to repo-resident instructions, deterministic setup scripts, and verification harnesses that let agents prove work through tests, logs, and reviewable diffs. This matches the external product direction: OpenAI and GitHub both move toward sandboxed background execution with PR-oriented evidence, while Anthropic emphasizes persistent project instructions, plan-first workflows, and verification loops. Practitioners will adopt the pieces that fit existing software delivery controls: branch protections, pull request review, test gates, and issue routing.

The constraint that shapes the market is not model cleverness alone but operational throughput under governance. Teams that make repositories agent-readable and verifier-friendly will see compounding gains: more parallel backlog burn-down, faster triage, and better handoff between humans and agents. Teams that do not will experience noisy patches, evaluation drift, and cost blowouts from remote execution. As a result, the standard deployment pattern by day 365 is an agent platform embedded into CI/CD and task systems, with explicit budget controls, test shards, policy gates, and versioned instruction layers. The practical thesis is simple: software organizations stop asking whether agents can code and start competing on whether their repos, evals, and governance surfaces are agent-operable.

Supporting observations: [obs: en-019d9888-9c12-7be1-a788-b343926a0290], [obs: en-019d9888-9c26-7893-9b2a-a67efaddc00c], [obs: en-019d9888-9c39-7e80-9481-be0653b4c1d4], [obs: en-019d9888-9c4b-7e70-935f-008938f500c5]

**Counterfactual:** If this direction is wrong and raw model quality dominates without repository and CI adaptation, then heavy investments in instruction files, eval packaging, and governed orchestration will be overbuilt relative to simpler chat-first workflows.

#### Directed software evolution consolidates around governed multi-agent institutions rather than free-form autonomous swarms.
**Direction ID:** en-019d9889-1858-7371-9aa4-9ad8ef8389f1

After one year, directed software evolution has matured into an institutional design problem, not a pure model-progress race. The recurring pattern across the state and external evidence is that systems improve when they are organized like governed ecologies: agents occupy bounded niches, operate inside isolated substrates, and are evaluated by increasingly explicit verification and escalation mechanisms. Anthropic's SWE-bench writeup shows that agent scaffolding materially changes outcomes, while Anthropic's agents guidance and OpenAI's Codex architecture both point toward simple composable control structures, task isolation, and iterative testing as the practical basis of scale. This means the unit of competition is no longer just model capability; it is the firm's ability to construct selective environments where useful agent behavior survives and error cascades die early.

The strategic implication is that the winning organizations will look like layered coordination systems with internal markets for task routing, verification, and exception handling. Their advantage will come from governance density without bureaucratic paralysis: clear interfaces, measurable acceptance tests, economic incentives for decomposition, and enough human judgment concentrated at the right chokepoints. The major failure mode is ideological overcommitment to agent autonomy. If leaders treat agents as substitutes for institutions rather than participants inside them, they will get local velocity and global disorder. The field should therefore optimize for institutional fitness—verification capacity, orchestration quality, and role redesign—because those are the mechanisms that compound over a year.

Supporting observations: [obs: en-019d9889-182f-7830-8b67-a02de6be767c], [obs: en-019d9889-183a-72d3-b9cc-ef4e147da5d1], [obs: en-019d9889-1844-7723-97fc-6d016733c5f8], [obs: en-019d9889-184e-7690-a92b-9831bdc035eb]

**Counterfactual:** If this thesis is wrong, then open-ended autonomous agents will become reliable enough that centralized control layers, verification chokepoints, and institutional redesign matter much less than expected.

#### By year one, Directed Software Evolution consolidates as a tightly governed augmentation stack, while broad autonomy claims lose credibility after security pressure, benchmark overreach, and organizational backlash.
**Direction ID:** en-019d9889-2879-7531-8329-df78e3de615a

The year validated a narrower thesis than the market wanted: Directed Software Evolution is real as a governed productivity layer, but brittle as a claim of broad autonomous software production. Three constraints kept reappearing. First, security did not become a solved wrapper problem. OWASP's 2025 treatment of prompt injection as a top-tier risk maps directly onto coding-agent workflows, where repositories, tickets, logs, and fetched web content all act as adversarial prompt surfaces. That means every expansion in tool access also expands the attack surface, which forces organizations toward narrower permissions and higher-friction review loops.

Second, evaluation credibility lagged capability marketing. SWE-bench remained an important benchmark, but over the year it became clearer that benchmark success is not equivalent to reliable performance on long-horizon, organization-embedded engineering work. METR's task-horizon framing is useful precisely because it exposes the remaining gap: many valuable engineering tasks require persistence, context management, and exception recovery beyond what current agents sustain without supervision. As a result, the winners were not the teams claiming dark factories; they were the teams building verification, provenance, rollback, and approval infrastructure around bounded agent work.

The implication is skeptical but not dismissive: the category survives, yet its center of gravity moves away from pure autonomy and toward auditable orchestration. Vendors and internal platform teams that continue to sell replacement narratives will trigger security incidents, trust loss, and budget backlash. Those that recast agents as governed, inspectable collaborators will still compound value over time.

Supporting observations: [obs: en-019d9889-2829-7921-917a-2ba1fa5a649a], [obs: en-019d9889-2837-7152-873e-ef61442f34ef], [obs: en-019d9889-2846-7a52-be63-e9461853926b], [obs: en-019d9889-2853-74b2-b1e4-ba14b25046eb], [obs: en-019d9889-2863-7ea3-8cdc-9206022bb936]

**Counterfactual:** If this thesis is wrong and broad autonomy becomes reliable faster than expected, organizations that over-indexed on governance-heavy human checkpoints may undercapture productivity upside and lose speed to more aggressive adopters.

## What Surprised Us

- The strongest surprise was not technical incapacity but **mergeability failure**: roughly half of benchmark-passing SWE-bench PRs still would not be merged, which is a harsher limit on production readiness than most vendor narratives implied [obs: en-019d9885-c7c9-71d0-acf8-7171b627b77f].
- Another surprise was that **manager-agent accumulation can become a bottleneck**; shared artifacts and stigmergic coordination sometimes scale better than thicker central planning [obs: en-019d9885-abb4-7fe0-be9d-1cf487b20875].
- A third surprise was how much **prompt injection through issues, comments, docs, and logs** remained a routine control-surface problem even after broader sandbox adoption [obs: en-019d9885-c7d2-7823-ab5b-6356248c1438] [obs: en-019d9889-2829-7921-917a-2ba1fa5a649a].
- We also underestimated the degree of **developer backlash against opaque provenance**: reviewability and reversibility became adoption gates even where aggregate throughput improved [obs: en-019d9889-2863-7ea3-8cdc-9206022bb936].
- Finally, the labor-market effect looked more like **role bifurcation** than replacement, with judgment, decomposition, and control-plane design growing scarcer rather than disappearing [obs: en-019d9889-184e-7690-a92b-9831bdc035eb].

## Top 5 Predictions with Falsification Criteria

1. **Prediction:** By 2026-06-30, enterprise agent platforms standardize on a two-lane operating model pairing local terminal agents with cloud backlog executors.
   - **Measurable indicator:** At least 2 distinct execution modes are present in most serious platform reference architectures and internal rollout docs.
   - **Confidence:** high
   - **Falsification:** If by 2026-06-30 most production deployments still rely on a single interactive agent mode without a separate queued sandbox execution path, this prediction is wrong because the expected workload split and parallelization mechanism did not materialize.
   - **Supporting observations:** [obs: en-019d9888-9c12-7be1-a788-b343926a0290], [obs: en-019d9888-9c4b-7e70-935f-008938f500c5]

2. **Prediction:** By 2026-09-30, repo-resident instruction layers and packaged verification harnesses become mandatory for successful agent rollout in medium-to-large engineering orgs.
   - **Measurable indicator:** Successful rollouts require at least 4 packaged assets per repo: instruction file, deterministic setup, smoke tests, and eval fixtures.
   - **Confidence:** high
   - **Falsification:** If by 2026-09-30 organizations can achieve stable agent throughput without versioned instruction files and reproducible verification packages, this prediction is wrong because governance-compatible reliability turned out not to depend on repository adaptation.
   - **Supporting observations:** [obs: en-019d9888-9c26-7893-9b2a-a67efaddc00c], [obs: en-019d9888-9c39-7e80-9481-be0653b4c1d4]

3. **Prediction:** By 2026-10-31, prompt-injection controls become a gating requirement for any coding agent with broad tool access.
   - **Measurable indicator:** 0 unrestricted write-capable agent roles remain in regulated or customer-facing production workflows without provenance checks and permission narrowing.
   - **Confidence:** medium
   - **Falsification:** If by 2026-10-31 broad-write coding agents are still commonly deployed without provenance filtering, permission minimization, and review gates—and without visible security rollback—this prediction is wrong because the market tolerated far more attack-surface risk than expected.
   - **Supporting observations:** [obs: en-019d9889-2829-7921-917a-2ba1fa5a649a], [obs: en-019d9885-c7d2-7823-ab5b-6356248c1438]

4. **Prediction:** By 2026-11-30, benchmark-only autonomy claims lose procurement power relative to repo-specific acceptance metrics and long-task reliability evidence.
   - **Measurable indicator:** Procurement reviews require at least 3 non-benchmark metrics: mergeability, long-task completion, and review burden.
   - **Confidence:** high
   - **Falsification:** If by 2026-11-30 leaderboard gains on SWE-bench still dominate buying decisions without accompanying mergeability or long-task evidence, this prediction is wrong because benchmark theater retained more market value than organizational experience suggested.
   - **Supporting observations:** [obs: en-019d9885-c7c9-71d0-acf8-7171b627b77f], [obs: en-019d9889-2837-7152-873e-ef61442f34ef], [obs: en-019d9889-2846-7a52-be63-e9461853926b]

5. **Prediction:** By 2026-12-31, the highest-performing organizations are those with layered task-routing, verification, and escalation institutions rather than the largest raw agent counts.
   - **Measurable indicator:** Top-performing teams operate at least 3 explicit layers—routing, execution, compliance—and keep verification queue growth below 1.0.
   - **Confidence:** medium
   - **Falsification:** If by 2026-12-31 flat autonomous swarms without strong institutional layering consistently outperform governed systems on reliability and deployment velocity, this prediction is wrong because coordination architecture mattered less than projected.
   - **Supporting observations:** [obs: en-019d9885-abaa-7ca1-aa46-b71f3d07d678], [obs: en-019d9885-abbf-79c0-b788-dcf23f16429e], [obs: en-019d9889-1844-7723-97fc-6d016733c5f8]

## Decision Points

#### Decision Point 1
- **Decision:** Whether to deploy policy-as-code gates for agent actions on the CI path before expanding write-capable coding agents.
- **Timing trigger:** First production incident review involving agent-written code or by 2026-07 when more than 20% of backlog tickets are agent-addressable [obs: en-019d9889-2829-7921-917a-2ba1fa5a649a] [obs: en-019d9889-2853-74b2-b1e4-ba14b25046eb].
- **Option A:** Deploy **Cedar** policy gates on repository actions and approval boundaries in CI
  — **Tradeoff:** 3-5 engineering-weeks; requires policy modeling discipline and ongoing rule maintenance.
- **Option B:** Deploy **OPA/Rego** admission checks for agent-triggered workflows and artifact promotion
  — **Tradeoff:** 2-4 engineering-weeks; faster if the platform team already uses OPA, but policy-debugging overhead can be high.
- **Option C:** Keep lightweight branch protections only and defer deeper policy controls
  — **Tradeoff:** 1 engineering-week now, but materially higher risk of prompt-injection or provenance failures surfacing later.
- **Recommended:** Option A — Cedar-style explicit action governance best matches the evidence that trust boundaries and approval semantics are becoming core infrastructure, not optional wrappers [obs: en-019d9885-c7d2-7823-ab5b-6356248c1438] [obs: en-019d9889-2829-7921-917a-2ba1fa5a649a].

#### Decision Point 2
- **Decision:** How to standardize agent-operable repositories before scaling from pilots to portfolio-wide use.
- **Timing trigger:** When the organization expands from 5 pilot repos to 25+ repos or by 2026-08 after the first review-burden spike [obs: en-019d9888-9c26-7893-9b2a-a67efaddc00c] [obs: en-019d9889-2863-7ea3-8cdc-9206022bb936].
- **Option A:** Mandate **AGENTS.md** plus deterministic setup scripts and smoke-test shards in every supported repo
  — **Tradeoff:** 4-6 engineering-weeks across platform templates and repo migrations; strong standardization payoff.
- **Option B:** Standardize on **CLAUDE.md** plus Anthropic-oriented hooks, skills, and permission scaffolds
  — **Tradeoff:** 3-5 engineering-weeks; faster for Claude-heavy teams but creates vendor-specific conventions.
- **Option C:** Let each repo team define its own prompts and scripts ad hoc
  — **Tradeoff:** 1-2 engineering-weeks upfront, but high long-run rediscovery cost and inconsistent agent behavior.
- **Recommended:** Option A — the evidence most strongly favors repo-resident, versioned instruction layers that remain portable across local and cloud execution modes [obs: en-019d9885-6ea9-7eb0-967a-47e0857e5834] [obs: en-019d9888-9c39-7e80-9481-be0653b4c1d4].

#### Decision Point 3
- **Decision:** Which execution substrate should handle bounded background work as agent volume rises.
- **Timing trigger:** When queue depth for bounded tasks exceeds reviewer capacity for 2 consecutive sprints or by 2026-09 when remote-agent minutes become a visible budget line [obs: en-019d9888-9c12-7be1-a788-b343926a0290] [obs: en-019d9885-aba0-7072-9dec-7f306a3605cc].
- **Option A:** Run queued work in **GitHub Actions** sandboxes with pull-request evidence collection
  — **Tradeoff:** 2-4 engineering-weeks; convenient integration, but Actions-minute costs can climb quickly.
- **Option B:** Use **Kubernetes** job runners with isolated namespaces, artifact retention, and custom budget controls
  — **Tradeoff:** 5-8 engineering-weeks; higher platform complexity but better control over scaling and isolation.
- **Option C:** Keep all agent execution local and human-invoked
  — **Tradeoff:** 1-2 engineering-weeks; lowest infrastructure cost, but limited parallelism and weaker backlog burn-down.
- **Recommended:** Option B — the year-one pattern favors sandboxed background execution with explicit control of queueing, isolation, and budget surfaces as volume rises [obs: en-019d9888-9c12-7be1-a788-b343926a0290] [obs: en-019d9888-9c4b-7e70-935f-008938f500c5].

## Assumptions & Limitations

1. **Assumption:** Verification throughput remains the primary bottleneck rather than raw model intelligence.
   - **If-wrong:** A sharp jump in long-horizon reliability would make current emphasis on harnesses and review gates look temporarily overbuilt [obs: en-019d9885-6e9f-7820-9aee-6c49e4815340] [obs: en-019d9889-183a-72d3-b9cc-ef4e147da5d1].
   - **Confidence:** medium-high

2. **Assumption:** Security and provenance pressure keep broad autonomous write access politically and operationally expensive.
   - **If-wrong:** If prompt-injection mitigations harden much faster than expected, organizations may widen permissions earlier and capture more upside from aggressive autonomy [obs: en-019d9885-c7d2-7823-ab5b-6356248c1438] [obs: en-019d9889-2829-7921-917a-2ba1fa5a649a].
   - **Confidence:** high

3. **Assumption:** Organizational adoption is constrained by review burden, trust, and role redesign, not only by tool availability.
   - **If-wrong:** If engineers rapidly adapt to opaque provenance and long diffs, some of the projected backlash and institutional friction will be overstated [obs: en-019d9889-2863-7ea3-8cdc-9206022bb936] [obs: en-019d9889-184e-7690-a92b-9831bdc035eb].
   - **Confidence:** medium

## Methodology

- 3 independent probes per step: practitioner, critic, adjacent-domain.
- 2 time steps over a 1-year horizon.
- 25 total observations and 5 active directions synthesized from actual entity data.
- Convergence emphasized cross-probe confirmation around verification bottlenecks, governance pressure, benchmark skepticism, and institutional design.
- The synthesis cites observation entities directly and uses observation-backed evidence for executive summary, findings, temporal progression, predictions, surprises, and decisions.

