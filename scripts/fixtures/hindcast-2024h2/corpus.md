# Frozen corpus — AI coding tools, as of 2024-06-15 (hindcast vantage)

Nothing in this file is dated after 2024-06-15. This is the world the
hindcast engine is allowed to know.

## Products and players (state of play, June 2024)

- GitHub Copilot: dominant installed base — Microsoft reported 1.3M paid
  subscribers and 50K+ enterprise customers in early-2024 earnings. Tiers:
  Individual $10/mo, Business $19, Enterprise $39 (launched Feb 2024).
  Copilot Chat GA since Dec 2023; Copilot Workspace (issue-to-PR
  environment) in technical preview since April 2024; Copilot Extensions
  announced at Build, May 2024.
- Devin (Cognition): announced March 2024 as an "AI software engineer",
  citing 13.86% on SWE-bench unassisted; $175M raise at ~$2B (April 2024,
  Founders Fund). Independent re-tests in April 2024 disputed parts of the
  launch demos. Access is waitlisted; no GA product as of vantage.
- Cursor (Anysphere): VS Code-fork AI IDE, seed-funded by the OpenAI
  Startup Fund (2023); strong word-of-mouth growth among developers in
  spring 2024 but still a small company at vantage.
- Codeium: free-for-individuals assistant; $65M Series B (Jan 2024) at a
  reported ~$500M valuation; enterprise self-host positioning.
- Amazon Q Developer: GA April 2024 (rebrand/expansion of CodeWhisperer).
- Google: Gemini Code Assist announced at Cloud Next, April 2024.
- Apple WWDC June 10-14, 2024: Xcode 16 announced with on-device predictive
  code completion, and "Swift Assist" (cloud, Apple-hosted) announced as
  coming "later this year" — not shipped at vantage.
- Stack Overflow signed an API partnership with OpenAI (May 2024).

## Models (as of vantage)

- OpenAI: GPT-4o (May 13, 2024) — $5/M input, $15/M output, half of
  GPT-4 Turbo's price; free-tier ChatGPT runs it. GPT-4o mini not yet
  announced. No reasoning-specialized model has shipped; "Q*" rumors
  (Nov 2023) remain rumors. No announced GPT-5 date.
- Anthropic: Claude 3 family (March 2024) — Opus $15/$75, Sonnet $3/$15,
  Haiku $0.25/$1.25. Opus is the coding benchmark leader of the family.
- Google: Gemini 1.5 Pro GA at I/O May 2024 (1M-token context); Gemini
  1.5 Flash announced as the cheap fast tier.
- Open-weight code models: Meta Code Llama 70B (Jan 2024) and Llama 3
  8B/70B (April 2024); Mistral Codestral 22B (May 29, 2024,
  non-production license); BigCode StarCoder2 (Feb 2024); DeepSeek-Coder
  (since late 2023) with aggressively low API prices.

## Structural facts already determined at vantage

- SWE-bench (Oct 2023) is the emerging agentic-coding benchmark; top
  published scaffold scores at vantage are under ~20% (SWE-agent 12.47%,
  April 2024).
- Enterprise dev-tool procurement runs 3-9 months; 2024 budgets were fixed
  in late 2023.
- GitHub Universe is calendared for late October 2024; AWS re:Invent for
  early December 2024.
- Token prices fell through 2024H1 (GPT-4 Turbo -> GPT-4o halving; Haiku
  and Flash as cheap tiers); no signs of reversal at vantage.
- US AI policy: no federal statute restricting code-generation tools.

## Open questions the market argued about at vantage (unresolved)

- Whether OpenAI ships GPT-5 — or any reasoning-specialized successor —
  in 2024.
- Whether Copilot Workspace reaches GA in 2024.
- Whether Copilot stays OpenAI-only or goes multi-model.
- Whether Devin converts its demo into a generally available product.
- Whether Cursor/Codeium can grow against Copilot's distribution.
- Whether Swift Assist actually ships inside Xcode 16 this year.
- Token-price trajectory for frontier coding models in 2024H2.
