<script lang="ts">
  import type { StateChangeEvent } from '$lib/sse';
  import type { Agent } from '$lib/types';
  import StatusBadge from './StatusBadge.svelte';

  let {
    event,
    agentSnapshot,
    timestamp,
    fromStatus,
    authz,
  }: {
    event: StateChangeEvent;
    agentSnapshot?: Agent;
    timestamp?: string;
    fromStatus?: string;
    authz?: {
      allowed: boolean;
      denied_resource?: string;
    };
  } = $props();

  let timeDisplay = $derived.by(() => {
    if (!timestamp) return event.seq ? `#${event.seq}` : '';
    const d = new Date(timestamp);
    if (isNaN(d.getTime())) return '';
    return d.toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
  });

  let expanded = $state(false);

  let shortId = $derived(event.entity_id?.slice(0, 8) ?? '');

  let hasToolCalls = $derived(
    !!agentSnapshot?.pending_tool_calls &&
    agentSnapshot.pending_tool_calls !== '[]' &&
    agentSnapshot.pending_tool_calls !== ''
  );

  let turnDelta = $derived.by(() => {
    if (!agentSnapshot) return null;
    return agentSnapshot.turn_count;
  });

  let tokenInfo = $derived.by(() => {
    if (!agentSnapshot) return null;
    return {
      input: agentSnapshot.input_tokens ?? 0,
      output: agentSnapshot.output_tokens ?? 0,
    };
  });
</script>

<div class="transition">
  <div class="transition__header">
    <StatusBadge status={event.status} />
    <span class="transition__action">{event.action}</span>
    {#if fromStatus}
      <span class="transition__flow">{fromStatus} &rarr; {event.status}</span>
    {/if}
    {#if authz}
      <span class="authz-badge" class:authz-badge--allowed={authz.allowed} class:authz-badge--denied={!authz.allowed}>
        {authz.allowed ? 'Allowed' : 'Denied'}
      </span>
      {#if authz.denied_resource}
        <code class="authz-resource">{authz.denied_resource}</code>
      {/if}
    {/if}
    <span class="transition__time">{timeDisplay}</span>
  </div>

  {#if turnDelta !== null || tokenInfo !== null}
    <div class="transition__counters">
      {#if turnDelta !== null}
        <span class="counter">turns: {turnDelta}</span>
      {/if}
      {#if tokenInfo}
        <span class="counter">in: {tokenInfo.input.toLocaleString()}</span>
        <span class="counter">out: {tokenInfo.output.toLocaleString()}</span>
      {/if}
    </div>
  {/if}

  {#if hasToolCalls}
    <button
      class="transition__expand"
      onclick={() => expanded = !expanded}
    >
      {expanded ? 'Hide' : 'Show'} tool calls
    </button>
    <div class="transition__tools-wrapper" class:transition__tools-wrapper--open={expanded}>
      <pre class="transition__tools">{agentSnapshot?.pending_tool_calls}</pre>
    </div>
  {/if}
</div>

<style>
  .transition {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }

  .transition__header {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  .transition__action {
    font-size: var(--text-sm);
    color: var(--text-primary);
    font-weight: 500;
  }

  .transition__id {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }

  .transition__flow {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }

  .transition__time {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    margin-left: auto;
  }

  .transition__counters {
    display: flex;
    gap: var(--space-2);
  }

  .counter {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }

  .transition__expand {
    font-size: var(--text-xs);
    color: var(--text-secondary);
    padding: 2px 0;
    text-align: left;
    width: fit-content;
  }

  .transition__expand:hover {
    color: var(--text-primary);
  }

  .transition__tools-wrapper {
    max-height: 0;
    overflow: hidden;
    transition: max-height var(--duration-base) var(--ease);
  }

  .transition__tools-wrapper--open {
    max-height: 200px;
  }

  .transition__tools {
    font-size: var(--text-xs);
    max-height: 200px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .authz-badge {
    font-size: 0.625rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    padding: 1px 5px;
    border-radius: var(--radius-sm);
  }
  .authz-badge--allowed {
    color: var(--status-success);
    background: rgba(61, 122, 61, 0.1);
  }
  .authz-badge--denied {
    color: var(--status-error);
    background: rgba(139, 58, 58, 0.1);
  }
  .authz-resource {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--status-error);
  }
</style>
