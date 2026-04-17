# Step 1 Rollup (day 365)

## Strongest Claims This Step
1. OpenAI Codex matured into a bounded cloud workcell with isolated environments, evidence-bearing logs, and 1-30 minute task loops rather than a free-ranging software organism.
   - Evidence: "each task runs in its own isolated environment, can run tests/linters/type checkers, usually takes 1-30 minutes, and returns verifiable evidence" [obs: en-019d98cf-f875-7d91-9c33-8bad5b518049]
   - Quantitative anchor: 1-30 minute task duration.
2. GitHub, Anthropic, and OpenAI are turning governance into a platform-default product feature, with admin-enabled model policies and repository-level cloud-agent controls.
   - Evidence: "model selection is available for Claude and Codex third-party coding agents on github.com, but only when the relevant Anthropic/OpenAI policy is enabled by an administrator" [obs: en-019d98d0-5b48-7ad1-b276-00b32f57a3d3]
   - Quantitative anchor: 2 vendor policies named explicitly: Anthropic and OpenAI.
3. SWE-bench Verified remains stuck at a 500-instance human-validated legitimacy layer, showing evaluation scarcity persists even as agent products scale.
   - Evidence: "SWE-bench Verified page still defines the benchmark as a human-validated subset of only 500 instances" [obs: en-019d98d0-11a7-72a3-baee-8bdb3b18aad0]
   - Quantitative anchor: 500 instances.
4. Cursor and OpenHands signal a shift from single-agent coding to managed agent fleets, where cost, queueing, and supervisory bandwidth become core constraints.
   - Evidence: "Cursor's public pricing shows a ladder from Pro at $20 per month to Pro+ at $60 per month and Ultra at $200 per month" [obs: en-019d98d0-11ba-7a00-b60f-0d7a88940af0]
   - Quantitative anchor: $20, $60, and $200 monthly tiers.
5. Governance did not converge on a single winner like Cedar or OPA; the practical enterprise stack is mixed across repo policy, CI rules, Kubernetes admission, Terraform workflows, and agent-local permissions.
   - Evidence: "mixed stacks are winning because the operational boundary is fragmented" [obs: en-019d98d0-11c4-78f3-9115-f0300472d2d5]
   - Quantitative anchor: at least 5 enforcement layers are named: repo permissions, CI, Kubernetes, Terraform, and agent-local permissions.

## Revisions to Prior-Step Claims
- Step 0 claim "Cedar and OPA are closer to production-scale agent governance than unconstrained self-modifying software agents." — status: revised by [obs: en-019d98cf-f889-7930-85b3-576ed62d6562]. Mechanism: platform-native GitHub controls and mixed-stack operations absorbed much of the governance surface, so Cedar/OPA matter but do not monopolize it.
- Step 0 claim "SWE-bench Verified shows the evaluation bottleneck is human-validated selection, not candidate generation volume from Anthropic, OpenAI, Devin, Aider, or OpenHands." — status: strengthened by [obs: en-019d98d0-11a7-72a3-baee-8bdb3b18aad0]. Mechanism: the benchmark still relies on a 500-instance curated subset a year later, proving the scarcity persisted.
- Step 0 claim "Procurement and security teams are likely to favor legible policy-wrapped autonomy over maximal autonomy, shifting advantage to control-plane vendors and platforms." — status: strengthened by [obs: en-019d98d0-5b48-7ad1-b276-00b32f57a3d3]. Mechanism: GitHub encoded administrator policy enablement directly into the product surface for Claude and Codex agents.
- Step 0 claim "OpenAI Codex, Claude Code, and Cursor are being adopted first as harness-governed repo operators rather than free-form autonomous coders." — status: strengthened by [obs: en-019d98d0-5b3d-74d1-ae93-6922d15f517e]. Mechanism: Codex productized isolated task sandboxes with harness hooks instead of autonomous production mutation.
