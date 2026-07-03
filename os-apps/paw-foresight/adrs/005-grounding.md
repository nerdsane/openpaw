# ADR-005: Grounding the corridor in present reality

Status: Accepted
Date: 2026-06-13

## Context

The first live run (run-1) exposed a grounding gap (G1): the engine minted
already-true facts as uncertain future forecasts. The worst instance — "first-
party agents grow as rivals", dated 2026-09-30 at p=0.55, when OpenAI Codex and
Anthropic Claude Code already competed at run time. Such a "forecast" resolves
yes for free and poisons calibration. Three causes compounded:

1. **No web grounding.** Agents had web tools but no `exa_api_key`, so they fell
   back to a stale training prior and could not tell what was already true.
2. **No reconcile pass.** Nothing collapsed an authored node that merely
   restated a determined/present fact before it was registered as a forecast.
3. **Eyeballed dates.** The repairer authored intermediate-event dates by
   picking tidy month/quarter ends; the world's lag table was never consulted.

## Decision

Ground every imagined future and every bridge in dated present reality, with
three mechanisms, in priority order (upstream prevention first, backstop last):

1. **Web-grounded present state.** With `exa_api_key` set, the surveyor verifies
   what is already determined via web search and records it as `determined`
   EventNodes (probability 1.0). In hindcast mode the corpus is the only source.
   This is the primary fix: the determined skeleton is the present-state anchor
   the repairer reuses. (The key is a tenant secret; the surveyor already had
   the web tools.)

2. **Lag-disciplined dates.** The repairer prompt now inlines the world's lag
   table and a DATE DISCIPLINE contract: date each authored node by adding the
   historical lag for its transition (from the table) to its prerequisite's
   date, between TODAY and the horizon; compressing below a historical lag
   raises a `lag` cost flag (severity by how far below), tying rushed dates to
   the existing pricing. A missing lag table degrades loudly. "Today" is
   rhetorical (no stored present-date field; `frontier_date` is the scoreable
   horizon, not the present); live repairers verify the date via web, hindcast
   repairers read the corpus vantage.

3. **Reconcile backstop.** `register_forecasts` refuses to register an authored
   node that restates a determined fact. It embeds the determined reference and
   the registrable authored candidates (one batch, via the D1 embedding
   capability — ADR-006) and collapses any candidate within a strict cosine
   distance of a determined node, logging every decision with its distance and
   the matched fact. It degrades to exact-text matching when no embedder is
   reachable — never a silent pass.

   This is deliberately a CONSERVATIVE backstop, not the primary fix: embedding
   distance alone cannot distinguish "restates a present fact" (collapse) from
   "a future change to something currently true" (keep), so the threshold
   stays strict (`RECONCILE_MAX_DISTANCE = 0.10`, a tunable prior). Calibrated
   live against mxbai-embed-large: domain restatements measure ~0.06, distinct
   forecasts ~0.29–0.35 — a wide, clean margin. Erring strict is correct: a
   false collapse drops a real forecast (worse than a missed collapse, which
   leaves one slightly-contaminated forecast the upstream fixes should prevent).

## Consequences

- Already-true facts no longer enter the gradeable set as free-resolving
  forecasts; calibration is no longer poisoned by them.
- Dates are earned from history, and rushed dates carry a priced `lag` flag
  rather than passing silently — the cost mechanism sees compression.
- The reconcile backstop is auditable (every collapse logged with distance +
  matched fact) and tunable from logged distances.
- Residual: the backstop only fires when the surveyor actually captured the
  determined fact; if the surveyor misses it, there is nothing to match
  against. The web-grounded surveyor is therefore load-bearing, not optional.
- An explicit "Present state as of <today>" brief inlined into every
  repairer/endpoint-writer prompt (beyond the determined-node list the repairer
  already reads) is a noted refinement; the determined EventNodes serve the
  anchor role today.
