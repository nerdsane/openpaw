# Step 0 Rollup (day 1)

## Strongest Claims This Step
1. OpenAI Codex, Claude Code, and Cursor are being adopted first as harness-governed repo operators rather than free-form autonomous coders.
   - Evidence: "repository-specific verification harnesses, not raw model quality, are the control surface teams will invest in first" [obs: en-019d98cc-232c-7610-8005-b29e28078e9c]
   - Quantitative anchor: Codex launch dated May 16, 2025; step projects 1 day forward.
2. Anthropic Claude Code and GitHub Actions-style workflows reinforce a frontend/backend split: agentic generation up front, deterministic CI and infra gates at promotion time.
   - Evidence: "promotion still flows through deterministic harnesses such as GitHub Actions, Kubernetes admission controls, Terraform plan/apply gates" [obs: en-019d98cc-2335-7ac3-94af-f9c4be261d83]
   - Quantitative anchor: at least 4 named control surfaces appear in the observation: GitHub Actions, Kubernetes, Terraform, and repo policy checks.
3. SWE-bench Verified shows the evaluation bottleneck is human-validated selection, not candidate generation volume from Anthropic, OpenAI, Devin, Aider, or OpenHands.
   - Evidence: "SWE-bench Verified explicitly presents itself as a human-validated subset of only 500 instances" [obs: en-019d98cc-430c-7eb1-b421-3eddf26b0b85]
   - Quantitative anchor: 500 verified instances.
4. Cedar and OPA are closer to production-scale agent governance than unconstrained self-modifying software agents.
   - Evidence: "real adoption will show up first in action gating for deploys, secrets access, environment mutation, and workflow transitions" [obs: en-019d98cc-233d-7d63-9958-13ddd956341e]
   - Quantitative anchor: 4 high-leverage action classes are named: deploys, secrets, environment mutation, workflow transitions.
5. Procurement and security teams are likely to favor legible policy-wrapped autonomy over maximal autonomy, shifting advantage to control-plane vendors and platforms.
   - Evidence: "the near-term winners are unlikely to be the products with the most unconstrained autonomy; they are more likely to be the stacks that make autonomy legible to procurement, security" [obs: en-019d98cc-5f09-7121-8362-5591566dea56]
   - Quantitative anchor: the claim names 3 selection centers: procurement, security, and governance review.

## Revisions to Prior-Step Claims
First step — no prior claims to revise.
