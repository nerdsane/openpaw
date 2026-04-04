# ADR-0013: Agent Plan Review Workflow

## Status

Accepted

## Context

When OpenPaw agents receive non-trivial tasks, they should research and plan before implementing. However, there was no structured way for plans to be reviewed:

1. Agents saved plans to `temper.save_memory()` — invisible, unreviewed, lost in memory
2. No dashboard visibility — humans couldn't see what agents planned before they executed
3. No review chain — the lead agent (Ren) had no mechanism to approve/reject plans from team members
4. Agents implemented unreviewed plans, leading to wasted effort on wrong approaches (e.g., multiple failed Datadog endpoint attempts)

The Temper platform has a built-in `Plan` entity (Draft → Active → Completed/Failed) but it lacked review-specific states and fields for the plan content and authorship.

## Decision

### Plans as first-class reviewed entities

Extend the Plan entity with review states and fields:

**New states:** `UnderReview`, `Escalated` (added to existing Draft/Active/Paused/Completed/Failed)

**New fields:** `plan_text`, `author_agent_id`, `reviewer_agent_id`, `review_notes`

**New actions:**
- `SubmitForReview` — Draft → UnderReview
- `Approve` — UnderReview → Active
- `RequestChanges` — UnderReview → Draft (with feedback)
- `Escalate` — UnderReview → Escalated (for human review)

### Review chain

```
Worker (SWE) creates Plan → SubmitForReview
    ↓
Lead (Ren) reviews
    ├─ Approve → Worker implements
    ├─ RequestChanges → Worker revises
    └─ Escalate → Human reviews in dashboard
         ├─ Human approves → Worker implements
         └─ Human rejects → Plan fails
```

### Dashboard integration

Plans appear on the Project page with:
- Status badges (Draft, UnderReview, Escalated, Active, Completed)
- Escalated plans highlighted with red border
- Plans under review highlighted with yellow border
- Expandable plan text
- Review notes visible

### Skill updates

- **openpaw-agent skill** (all agents): Changed from "save plan to memory" to "create Plan entity → submit for review → call done()"
- **openpaw-lead skill** (new, for leads): Instructions for reviewing plans, approving/rejecting/escalating

## Consequences

### Positive

- **Human visibility.** Every plan appears in the dashboard before implementation starts.
- **Quality gate.** The lead agent reviews plans, catching wrong approaches early.
- **Audit trail.** Plan entity records who proposed what, who approved it, and any feedback.
- **Escalation path.** Uncertain plans reach the human without the lead approving blindly.
- **Agent efficiency.** Plans stop agents from spending 100+ turns on wrong approaches — the review catches it at turn 5-10.

### Negative

- **Added latency.** Plans must be reviewed before implementation, adding a round-trip. Mitigated by: leads can auto-approve trivial plans, and the `Activate` action bypasses review for direct activation.
- **Lead bottleneck.** If the lead is busy, plans queue up. Mitigated by: plans can be escalated to human, and the dashboard shows all pending plans.
