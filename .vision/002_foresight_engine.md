# Foresight

## What Foresight Is

Foresight is an engine inside OpenPaw that projects a product's plausible futures, surfaces the strongest directions, and delivers them as working code on branches — ready for a human to select and merge.

Today, product managers imagine where a product should go, write specs, and hand them to engineers. Foresight inverts this. The engine observes the product's current state — its codebase, its health signals, its usage patterns, its history — and projects multiple possible futures grounded in that reality. Each future is a creative but plausible direction the product could take. Each direction is implemented, tested, and accompanied by a narrative explaining why it exists. The product manager's job shifts from "imagine where to go" to "choose from working futures."

## What Foresight Achieves

**A product that knows its own future.** Not one predicted future — many. The strongest directions are the ones that multiple independent perspectives arrive at separately, that hold up under scrutiny, and that would have prevented past problems if they had existed earlier.

**Directions grounded in reality, not imagination.** Every proposed direction traces back to real signals: observed error rates, dependency health, usage patterns, alert history, codebase structure. The reasoning is auditable — a chain of cause and effect from today's facts to tomorrow's projection, with honest confidence that decays over distance.

**Creative futures, not just reactive fixes.** Foresight doesn't only predict what will break. It notices what users are trying to do but can't. It sees where the codebase could be simplified. It identifies capabilities that the current architecture almost supports. It proposes removals as confidently as additions. The engine is generative — it imagines directions that nobody asked for but that the signals support.

**Choose your own adventure.** The human sees 2-5 branches, each with passing tests, each with a narrative artifact explaining the signal chain that led to it. They pick the direction that matches their judgment. Or they pick none — and the engine learns from that too.

## The Product Model

At the center of Foresight is a living model of the product. Not just the code — the full picture:

- The codebase: its structure, its dependencies, its test coverage, its architectural patterns
- Its health: error rates, latency, resource usage, alert frequency, what breaks and how often
- Its history: what changed recently, what areas are volatile, what's been stable, what was attempted and reverted
- Its environment: dependency EOL dates, ecosystem changes, known CVEs, upstream breaking changes
- Its usage: which features are growing, which are stagnant, where users push against limitations

This model is real, queryable, and continuously updated from live signals. It is the substrate that Probes inhabit.

## Probes

Probes are agents that inhabit the product model's projected future. They are not observers looking in from outside — they live within the model as it advances through time, noticing what happens, reasoning about what it means, and proposing directions based on what they see.

Probes are not assigned a checklist. They have no prescribed actions. They notice what they notice. They reason as deeply as they can. They challenge each other. Different Probes may see the same signal and draw different conclusions — when they disagree, that disagreement itself is information.

Different Probes may run on different models. A Probe powered by one model might excel at deep causal reasoning. Another might see quantitative patterns more clearly. Another might understand code structure at a level the others can't. The engine doesn't care which model powers which Probe — it cares whether their observations survive scrutiny.

## Signal Chains

When a Probe proposes a direction, it doesn't just assert "we should do X." It produces a signal chain: a causal sequence from today's observed reality to the projected future, grounded at each step in specific data. Confidence decays naturally along the chain — near-term projections are strong, far-term projections are honest about uncertainty.

Signal chains are the reasoning trace, the validation evidence, and the narrative artifact — all in one structure. When a direction graduates and gets implemented, the signal chain becomes the story of why it was built. Auditable, traceable, grounded.

## Temporal Projection

The product model advances through time. Signals are extrapolated. Dependencies age. Traffic patterns shift. Entropy accumulates — the natural degradation that happens when nobody acts.

Probes experience this advancing model and react to what emerges. Near-term projections are fine-grained. Far-term projections are broader. The resolution adapts to the horizon — like a telescope, sharp up close and directional at distance.

The projection horizon is weeks to months. The world moves fast. Six months is the outer edge. Weeks are where the sharpest, most actionable directions live.

## Quality Through Structure

Foresight does not use scoring functions to decide which directions are good. Quality emerges from structural properties:

- **Peer confirmation**: an observation only matters when a different Probe independently says it matters
- **Blind review**: reviewers cannot see each other's assessments, preventing groupthink
- **Graduation**: every objection must be answered before a direction advances — not "good enough," but "every challenge met"
- **Convergence**: when multiple Probes, potentially running different models, independently arrive at the same direction — that is the strongest signal the engine can produce
- **Counterfactual grounding**: a direction is stronger when it would have prevented past problems, and weaker when past problems would have occurred regardless
- **Cross-horizon convergence**: when the same direction appears in both short-term and long-term projections — urgent AND strategic — that is a direction worth pursuing

These are not opinions. They are structural facts about how the Probe swarm behaved. The engine observes convergence; it does not manufacture it.

## Branching Futures

When Probes disagree about what happens next, the projection forks. Both timelines continue. Both accumulate observations. The one that more Probes independently validate persists. The other fades.

Branching is not a feature to be triggered. It is what naturally happens when intelligent agents with different perspectives project the same reality forward. The engine holds space for disagreement rather than forcing consensus.

## Implementation as Output

This is what makes Foresight different from prediction, speculation, or analysis. Graduated directions don't produce reports. They produce working code.

A Developer agent takes the graduated direction into a sandbox, implements it on a feature branch, writes tests, and opens a PR. The signal chain becomes the PR description. The branch is real, tested, and mergeable.

The product manager selects from working branches, not from documents.

## The Artifact

Each direction produces a narrative artifact alongside its implementation. This artifact tells the story of the direction: what signals gave rise to it, what causal chain connects today to the projected future, what would have happened without it, and what the implementation changes. It is generated from the signal chain — honest, grounded, and specific.

These artifacts serve as the product's memory of its own projected futures — including the ones that were not selected.

## Future-Readiness

Foresight is designed so that its value increases as models improve, without redesign.

A more capable model inhabiting the same substrate produces deeper signal chains, more non-obvious observations, sharper counterfactual reasoning, finer temporal resolution, and more creative directions. The gates are the same. The entities are the same. The quality just increases.

A new model becomes a new Probe. Cross-model consensus — where fundamentally different reasoning approaches independently converge — becomes possible the moment a second model is available.

The ceiling is the model's capability, not the engine's design.

## Target

The first product managed by Foresight is Deep Sci-Fi, which already has a team and harness configured in OpenPaw. Foresight will observe Deep Sci-Fi's codebase, health signals, and development history, and begin projecting plausible futures for the platform.
