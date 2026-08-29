## Decisions & Tradeoffs

**Decision:** Grant the shadow-sweep write actions to `principal is Agent` gated
on `agent_type == "operator" && agentTypeVerified == true`, not to a broadened
Admin permit or a looser agent predicate.
**Came up because:** the sweep's key resolves to the operator AgentCredential
(verified empirically: the temperpaw secret == the prod openpaw operator key by
sha256; it bootstraps as AgentType "operator"). The denied actions need a permit
for THAT principal.
**Options:** widen the existing Admin permit to cover the operator (rejected -
the operator is an Agent, not Admin, and widening Admin is broader than needed);
grant any `principal is Agent` (rejected - any self-declared agent could then
write records; unsafe); mirror the kernel's verified-operator predicate (chosen).
**Chose the verified-operator predicate because:** it is the exact identity the
kernel already trusts for operator actions (`seed_operator_manage_policies` uses
the same `agent_type == "operator" && agentTypeVerified == true`), and the
`agentTypeVerified == true` clause excludes header-self-declared agents. Given
up: nothing - it is the narrowest grant that lets the sweep work.
**Where:** `os-apps/paw-patrol/policies/patrol.cedar`.

**Decision:** Three per-resource permits (each listing only that resource's
denied actions), not one permit with a combined action+resource set.
**Came up because:** "Ingest" exists on both ReviewRun and ProofPacket, but
Record/MarkAgree/MarkDisagree only on ShadowVerdict, and IngestRecord only on
ReviewRun / IngestProof only on ProofPacket.
**Chose per-resource permits because:** a single combined permit would nominally
allow non-existent pairs (e.g. ShadowVerdict.Ingest, ReviewRun.Record); per-
resource permits grant exactly the real denied pairs and read as the intent.
Given up: a few more lines.
**Where:** `os-apps/paw-patrol/policies/patrol.cedar`.
