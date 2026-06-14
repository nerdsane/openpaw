# ADR-006: Diverse worlds — named axes + an embedding diversity gate

Status: Accepted — option A (World barrier) chosen and implemented 2026-06-13
Date: 2026-06-13

## Context

The first live run exposed a sampling-fidelity gap (G2): the modal and
anti-modal worlds converged. Stances were unverified prompt instructions
("take the 85th-percentile view"), nothing measured whether an "anti-modal"
world was actually off-consensus, and the worlds were two arbitrary stances
rather than a portfolio across named uncertainties. Worlds are expensive
(~30–50 sessions each), so near-duplicates waste the budget and the reader
learns nothing new from the second world.

Two distinct guarantees are needed, with separate machinery (do not conflate):
- **Distinct from each other** — a property of the sampled set, measurable with
  embeddings (distance between samples).
- **Surprising vs consensus** — a property relative to a reference pole, which
  embeddings alone cannot supply (distance-between-samples ≠ distance-from-
  consensus). This needs a named consensus reference.

## Decision

### Accepted and implemented: the embedding capability (D1)

`corridor_embed` is the shared, pure, deterministic core: cosine / distance /
`min_pairwise_distance` / `is_diverse` / `farthest_point_order` /
`cluster_by_threshold` / `nearest`, plus `parse_embeddings` and
`build_embed_request`. The semantic judgment comes from an external model
(`mxbai-embed-large` via local Ollama in dev, a hosted endpoint in prod —
resolved from `embedding_endpoint`/`embedding_model` config, same pattern as
web search). The HTTP fetch is a ~15-line `ctx.http_call` in each consumer;
the decisions are pure and reproducible. Egress is already module-gated in
Cedar. First consumer shipped: the D2 reconcile backstop (ADR-005).

### Proposed: named axes (sampling) + the diversity gate (verification)

1. **Named uncertainty axes (sampling side).** The surveyor names the top-K
   load-bearing uncertainty axes for the domain and the consensus pole of each,
   stored on the World. `sample_endpoints` becomes a portfolio sampler: slot 0
   is the consensus anchor; each other slot inverts a *named* axis (not a
   generic percentile), and each endpoint-writer is briefed on the sibling axes
   so it writes to differentiate. This is the "surprising vs consensus"
   guarantee, via a named reference — no embeddings.

2. **Embedding diversity gate (verification side).** Before the corridor spends
   sessions, verify the written worlds are actually distinct: embed each
   endpoint's bundle/claim-set, enforce a minimum pairwise distance
   (`farthest_point_order` / `is_diverse`), and re-steer (re-spawn the writer
   with a "differentiate from these" brief) any world that collapsed onto
   another — then release the diverse set to decomposition. This is the
   "distinct from each other" guarantee, via embeddings.

   **Open architectural question (for review).** The gate needs a barrier: it
   must see all written bundles before it can judge the set, but today each
   endpoint flows independently writer → `SubmitForRepair` → decompose, with no
   World-level wait. Two options:
   - **(A) World barrier.** A new `World.GateDiversity` phase that waits for all
     endpoints to carry bundles, embeds them, re-steers collapsed ones, and only
     then releases survivors to decomposition. Cleanest semantically; adds an
     orchestration phase + a re-steer loop + likely an Endpoint `Written` state.
   - **(B) Per-endpoint incremental gate.** At `SubmitForRepair`, before
     decomposing, check this endpoint against already-gated siblings with
     `is_diverse`; if too close, re-steer instead of decompose. No global
     barrier; order-dependent (later endpoints checked against earlier), but
     still yields a mutually-diverse set. Awkward because the re-steer must
     re-spawn a writer, which lives in `sample_endpoints`, not
     `decompose_endpoint`.

   Recommendation: (A) — the barrier models the guarantee honestly and keeps
   writer-spawning in one module; the cost is one orchestration phase.

   **Decided: option A, implemented.** The writer self-reports `BundleWritten`
   (Endpoint Sampled→Written, no decompose). `sample_endpoints` runs the
   all-written barrier on each `BundleWritten`; when no endpoint is still
   `Sampled`, it dispatches `World.GateDiversity` (round counter on the World).
   The gate embeds each Written world's bundle-head (mxbai caps at 512 tokens,
   so a ~1800-char head, not the 30KB bundle; claims don't exist pre-decompose),
   greedily keeps the diverse ones via `select_diverse` against the already-
   released references, releases them with `Endpoint.SubmitForRepair` (→
   decompose → repair), and re-steers the collapsed ones with `Endpoint.ReSteer`
   (re-spawn the writer with a "diverge from <nearest world's summary>" brief).
   After `GATE_MAX_ROUNDS` (2) re-steer rounds a persistent near-duplicate is
   Discarded. `DIVERSITY_MIN_DISTANCE` (0.15) is a tunable prior; the gate logs
   per-round counts so it calibrates from real bundle distances. If the embedder
   is unreachable the gate releases everything (loudly) rather than wedge the
   pass. Correctness — that it actually re-steers a collapsed world — is verified
   live in the flagship run.

## Consequences

- The budget only ever buys distinct, on-axis worlds; near-duplicates are
  re-steered before the expensive corridor, not after.
- Diversity (embeddings) and surprise (named consensus reference) stay separate
  mechanisms, so neither is mistaken for the other.
- The synthesis panel (D4) reuses `cluster_by_threshold` over the gated worlds'
  claims to show cross-world agreement vs divergence — the readability payoff
  of having genuinely distinct worlds.
- Until the gate lands, worlds are sampled with distinct stances but their
  mutual distinctness is not enforced — a flagship run before the gate may
  still show some convergence (an honest, logged limitation).
