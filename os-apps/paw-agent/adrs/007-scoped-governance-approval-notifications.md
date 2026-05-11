# ADR-007: Scoped Governance Approval Notifications

- Status: Accepted
- Date: 2026-05-11

## Context

Governed tool denials pause a Session with a pending decision. The
`request_approval` integration forwards that decision to the originating chat
channel, where the Discord or Slack transport records the reviewer response.

The prior approval payload only identified the principal, resource breadth, and
duration. Temper's Cedar policy scope matrix also requires an action dimension,
and session-scoped grants require a session id. The notification copy also hid
important decision fields, so reviewers saw vague prompts without enough context
to choose a safe scope.

## Decision

Approval notifications include persisted pending-decision details when
available: action, resource, module, session, reason, request preview, and
decision id.

Discord and Slack now expose four choices:

- Allow Always: this agent, this action, any resource of this type.
- Allow Session: the same action/resource-type grant, limited to this session.
- Allow Once: this action on this exact resource, limited to this session.
- Deny: reject the attempt.

Transport handlers fetch the pending decision before approving and construct the
full Cedar scope body, including `action=this_action`. Session and once-style
approvals require the decision's `session_id` so the grant is not silently
widened.

Legacy `approve:{decision_id}` button ids remain accepted, but are routed
through the complete scope body as Allow Always.

The CLI/TUI exposes the same scopes as commands:

- `/approve-always <decision>` (legacy `/approve` remains an alias)
- `/approve-session <decision>`
- `/approve-once <decision>`
- `/deny <decision>`

## Consequences

- Chat approvals satisfy Temper's richer Cedar scope requirements.
- Reviewers can understand what the agent is trying to do before deciding.
- The approval surface supports durable, session-limited, and narrow
  resource-scoped grants across Discord, Slack, and the CLI/TUI without
  introducing a separate orchestration layer.
- "Allow Once" is intentionally represented as exact-resource and
  session-limited. A true consume-once grant would require a platform policy
  lifetime primitive in Temper.
