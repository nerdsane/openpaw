# Engine Directions (Run 000)

Total: 3 directions from step 0 only

## Direction 1: Over the next 90 days, directed software evolution will gain attention faster than it earns operational trust because approval economics and workflow brittleness will dominate model capability gains.

**Proposer:** aj-019d9680-a18b-7af1-8afc-4e189e3c3d9d
**Step:** 0
**Status:** Proposed

### Reasoning
The near-term failure mode for directed software evolution is not that Anthropic, OpenAI, Cursor, Devin, or OpenHands suddenly stop producing useful code; it is that enterprises discover the cost of supervising action chains is still higher than the value of delegating them. When agent output moves from draft code into Kubernetes changes, Terraform plans, or CI remediation, the real unit of work becomes approval and rollback, not generation. Cedar and OPA can restrict who is allowed to act, but they do not by themselves prove that the chosen action sequence is safe, cost-effective, or reviewable under production conditions. In that environment, every escalation, exception, and revert becomes evidence that workflow reliability is lagging behind model capability.

That points to a likely market split. Tools like Cursor, Aider, and Cline will keep expanding because they preserve developer agency while compressing drafting time; their governance surface is smaller and their ROI is easier to explain. By contrast, platforms like Temper that try to make autonomous or semi-autonomous execution auditable will be judged on integration burden: how quickly they can encode a real approval chain, how many exceptions they need, and whether incident responders can reconstruct why an action happened. If production pilots show more than a small fraction of agent-generated pull requests being reworked, or if onboarding a governed workflow still takes weeks, buyers will conclude that the category is promising but not yet operationally mature. The strongest 90-day winners will therefore be systems that narrow scope, expose explicit checkpoints, and treat autonomy as a governed escalation path rather than a default operating mode.

### Counterfactual
If enterprises unexpectedly accept higher review overhead and tolerate imperfect auditability, full-stack autonomous coding platforms could expand faster than this thesis predicts.

---

## Direction 2: The near-term winners in directed software evolution will be teams that build policy-governed selection systems around multiple coding agents, not teams that merely adopt the strongest single model.

**Proposer:** aj-019d9680-a199-7b60-8d30-b910790eed29
**Step:** 0
**Status:** Proposed

### Reasoning
Directed software evolution will advance in the next 90 days where organizations treat AI coding as a governed evolutionary process rather than as a better autocomplete market. The adjacent-domain pattern is clear: biology, portfolio economics, and industrial control all reward systems that separate variation from selection. Anthropic and OpenAI models, along with Cursor, Aider, Cline, OpenHands, and Cognition Devin, will keep improving as generators, but generators are only one half of an evolutionary machine. The operational differentiator will be infrastructure that allocates tasks across agents, constrains authority with Cedar or OPA, and evaluates changes against stable harnesses in Kubernetes and Terraform environments. Temper is important here not because it writes the best patch, but because it can behave like a machine tool and workflow governor: create bounded experiments, route them through policy, and keep an audit trail of what changed and why.

The economics of adoption will therefore look like factory modernization, not like a consumer software feature race. Firms that instrument their systems well enough to measure rollback rates, policy violations, accepted-change yield, and cost per verified change will discover that multi-agent portfolio routing beats single-model dependence. They will qualify different tools for different roles: a cheaper explorer for breadth, a premium model for synthesis, a verifier for adversarial review, and policy engines to reject unsafe moves before production. In organizational terms, the winning teams will create trust tiers for agents the same way supply chains create approved-vendor lists. The next wave of durable advantage will come from selection pressure, not raw generation power; if this thesis is wrong and model quality alone dominates, then organizations using a single frontier model without deep harnessing should outperform governed multi-agent systems within the quarter.

### Counterfactual
If the thesis is wrong, the next 90 days will show single-model workflows from one vendor outperforming governed multi-agent systems on accepted-change yield, incident rate, and cost without requiring stronger policy or instrumentation layers.

---

## Direction 3: In the next 90 days, practitioners will make directed software evolution real by narrowing agent scope into harnessed control-plane actions rather than pursuing broad autonomous coding.

**Proposer:** aj-019d9680-a17e-78d3-a591-88a3b491a746
**Step:** 0
**Status:** Proposed

### Reasoning
Over the next 90 days, directed software evolution will advance most in teams that treat Anthropic and OpenAI models as proposal generators inside a governed control plane, not as autonomous engineers. The operational pattern is already visible: use Temper to represent work as entity state transitions, route side effects through WASM integrations, evaluate authorization with Cedar and OPA, and constrain infrastructure changes through Terraform plan review plus Kubernetes admission control. In this setup, models from Anthropic or OpenAI, and tools like Cursor, Aider, Cline, or OpenHands, compete upstream on candidate generation, while downstream selection is determined by the verification cascade: tests, property checks, policy evaluation, replay, and rollback metrics. That architecture is deployable now because it matches how platform teams already ship control-plane software.

The practical winner will be the organization that reduces free-form surface area. Instead of asking a model to run a whole repo or cluster, practitioners will define bounded actions such as propose Terraform change, summarize plan risk, patch Kubernetes manifest, evaluate Cedar policy impact, or execute approved remediation through WASM. This makes the system observable and auditable enough for real deployment, and it creates the data needed for directed evolution: pass rates by action, rollback frequency, policy violation counts, and time-to-merge by tool. The dominant narrative says more capable agents like Devin-class systems or better IDE copilots will directly create dark factories; the near-term reality is narrower. Teams that decompose work into harnessed, measurable actions will outperform teams that chase broad autonomy, because selection pressure on governed actions compounds faster than raw model capability on unconstrained tasks.

### Counterfactual
If this thesis is wrong, broad autonomous coding products from Anthropic, OpenAI, Cursor, or Cognition will prove reliable across large unstructured repos without heavy harness engineering, and bounded action architectures like Temper plus Cedar or OPA will be overtaken by general-purpose terminal agents.

---

