# Decisions and tradeoffs

## D1 — Keep the operational model in a separate DSF factory app

**Decision:** Add `os-apps/dsf-factory` and extend existing SDLC deployment linkage where required.

**Came up because:** DSF resources and observations outlive individual Efforts, while the user explicitly excluded migrating DSF product logic into Temper.

**Options:** Put operational records on each Effort; migrate DSF product entities; use a separate operational app linked to existing Efforts.

**Chose the operational app because:** It preserves stable resource identity and existing product implementation. It adds one published app and explicit cross-app dependencies.

**Where:** docs/efforts/ARN-467/spec.md, Operational records and Resource operations and delivery.

## D2 — Preserve Foundry's run system and Temper's work records

**Decision:** Link Foundry runs to Temper Efforts and render existing Asks through a narrow integration.

**Came up because:** Foundry already provides direct computer chat, durable commands and transcripts, but currently declines MCP elicitation and overwrites harness configuration during bootstrap.

**Options:** Replace Foundry orchestration; duplicate Effort/Ask state in Foundry; retain each system's existing state and add explicit linkage and request/reply handling.

**Chose explicit linkage because:** It reuses working computer/chat behavior and gives both interfaces one authoritative decision record. The integration must handle synchronous elicitation and recovery deliberately.

**Where:** docs/efforts/ARN-467/spec.md, Foundry and agent access; private fork arni-labs/foundry at 6ca87a793df711c79e609111560ee0c7491b0c1b.

## D3 — Use subscriptions for agents and bound additional spend

**Decision:** Use existing agent subscriptions without automatic API-key fallback and cap additional overnight costs at $100.

**Came up because:** Rita approved $100 and clarified that agents should run on subscriptions; DSF product verification can independently call metered APIs.

**Options:** Allow metered agent fallback; stop all paid product verification; keep agent auth subscription-only and account for necessary product API calls plus hosting/compute under the cap.

**Chose the third option because:** It matches the user's authorization while permitting real application proof. Unavailable subscription consent remains a concrete blocker, not permission to change billing modes.

**Where:** docs/efforts/ARN-467/intent.md and ARN-467 authorization record.
