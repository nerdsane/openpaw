# Frozen corpus — the LLM API market (models, pricing, providers), as of 2024-12-15 (hindcast vantage)

Nothing in this file is dated after 2024-12-15. This is the world the
hindcast engine is allowed to know.

## Providers and models (state of play, December 2024)

- OpenAI: GPT-4o ($2.50/M input, $10/M output since the Aug 2024 cut),
  GPT-4o mini ($0.15/$0.60); o1-preview and o1-mini (Sept 12, 2024); full
  o1 plus the $200/mo ChatGPT Pro tier (Dec 5, 2024). The "12 Days of
  OpenAI" launch series is mid-run at vantage (Sora video generation
  released Dec 9). Realtime API and prompt caching shipped at DevDay
  (Oct 2024). Closed a $6.6B round at a $157B valuation (Oct 2024).
  Press reports (Nov 2024) claim the next big pretrain ("Orion") shows
  diminishing returns; no GPT-5 or GPT-4.5 has been announced.
- Anthropic: Claude 3.5 Sonnet (upgraded Oct 22, 2024, $3/$15) with the
  computer-use beta; Claude 3.5 Haiku (Nov 4, 2024, $1/$5 — a 4x price
  hike over Claude 3 Haiku that drew criticism). Model Context Protocol
  (MCP) open-sourced Nov 25, 2024. Amazon committed an additional $4B
  (Nov 22, 2024; $8B total). No Claude 4 announced.
- Google: Gemini 1.5 Pro ($1.25/$5 under 128K) and 1.5 Flash
  ($0.075/$0.30); Gemini 2.0 Flash experimental announced Dec 11, 2024
  with agent demos (Mariner, enhanced Astra). Trillium TPUs GA.
- Meta: Llama 3.3 70B open weights (Dec 6, 2024); Llama 3.1 405B
  (July 2024). Free weights continue to anchor the low end of pricing.
- Amazon: Nova model family announced at re:Invent (Dec 3, 2024) —
  Micro/Lite/Pro shipping, Premier promised for 2025; Bedrock as the
  multi-model storefront.
- xAI: Grok-2 on X and via API beta (late 2024); Colossus cluster
  (~100K H100s, Sept 2024); raised $6B at ~$24B (May 2024).
- DeepSeek: V2.5 serving at aggressive prices (the May 2024 China price
  war started at ~$0.14/M input); R1-Lite-Preview reasoning model
  announced Nov 20, 2024 with a promise of open weights to come.
- Alibaba Qwen: Qwen2.5 family; QwQ-32B-Preview open reasoning model
  (Nov 28, 2024). Mistral: Large 2 (July), Pixtral Large (Nov 18).

## Structural facts already determined at vantage

- Reasoning-token pricing exists (o1 at $15/$60) but only OpenAI ships a
  production reasoning model; everyone else has previews.
- Per-token prices for comparable capability fell all year; cheap tiers
  (4o mini, Flash, Haiku) are the volume play.
- The US election is decided (Nov 2024); the new administration takes
  office Jan 20, 2025 — AI policy direction is unset at vantage.
- MCP exists (Nov 2024) but is Anthropic-only at vantage.

## Open questions the market argued about at vantage (unresolved)

- What OpenAI ships in the remaining "12 Days" — and whether an o2/o3
  class reasoning model arrives in 2025H1.
- Whether GPT-5 (or a GPT-4.5 stopgap) ships in 2025H1.
- Whether Anthropic ships Claude 4 in 2025H1.
- Whether DeepSeek releases full R1 open-weights — and whether a Chinese
  lab reaching frontier reasoning moves Western pricing.
- Whether Google's Gemini 2.x closes the gap on coding/reasoning.
- Whether xAI ships Grok 3 off the Colossus buildout.
- Whether Meta ships Llama 4 in 2025H1.
- Whether MCP gets adopted beyond Anthropic.
- Frontier-lab fundraising: who raises next, and at what valuation.
- Whether any frontier lab approaches the public markets.
