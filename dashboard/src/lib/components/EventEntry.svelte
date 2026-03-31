<script lang="ts">
  import type { StateChangeEvent } from '$lib/sse';

  let { event }: { event: StateChangeEvent } = $props();

  let shortId = $derived(event.entity_id?.slice(0, 8) ?? '');

  let dotColor = $derived.by(() => {
    switch (event.status) {
      case 'Completed':
      case 'Complete':
      case 'Resolved':
      case 'Done':
        return 'var(--status-success)';
      case 'Failed':
      case 'Cancelled':
      case 'Escalate':
        return 'var(--status-error)';
      case 'WaitingForApproval':
      case 'Reviewing':
      case 'Testing':
        return 'var(--status-warning)';
      case 'Thinking':
      case 'Executing':
      case 'InProgress':
      case 'Triaging':
      case 'Planning':
      case 'Running':
        return 'var(--status-active)';
      default:
        return 'var(--status-idle)';
    }
  });

  let entityUrl = $derived(`/entities/${event.entity_type}/${event.entity_id}`);

  let timeLabel = $derived.by(() => {
    // SSE events don't carry a timestamp; show seq as a fallback
    return `#${event.seq}`;
  });
</script>

<a href={entityUrl} class="event-entry">
  <span class="event-dot" style:background={dotColor}></span>
  <code class="event-type">{event.entity_type}</code>
  <span class="event-desc">
    {event.entity_type} {shortId}: {event.action} &rarr; {event.status}
  </span>
  <span class="event-time">{timeLabel}</span>
</a>

<style>
  .event-entry {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: 6px 0;
    text-decoration: none;
    color: inherit;
    border-bottom: 1px solid var(--border);
    transition: background var(--duration-fast) var(--ease);
  }

  .event-entry:hover {
    background: var(--brand-subtle);
    text-decoration: none;
  }

  .event-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .event-type {
    font-family: var(--font-mono);
    font-size: 0.625rem;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.03em;
    flex-shrink: 0;
    min-width: 70px;
  }

  .event-desc {
    font-size: var(--text-sm);
    color: var(--text-secondary);
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .event-time {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    flex-shrink: 0;
    margin-left: auto;
  }
</style>
