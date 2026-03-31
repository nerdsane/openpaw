<script lang="ts">
  import { slide } from 'svelte/transition';
  import type { StateChangeEvent } from '$lib/sse';
  import type { Agent } from '$lib/types';
  import StatusBadge from './StatusBadge.svelte';

  let {
    event,
    agentSnapshot,
    timestamp,
    fromStatus,
  }: {
    event: StateChangeEvent;
    agentSnapshot?: Agent;
    timestamp?: string;
    fromStatus?: string;
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

<div class="transition" transition:slide={{ duration: 200 }}>
  <div class="transition__header">
    <StatusBadge status={event.status} />
    <span class="transition__action">{event.action}</span>
    {#if fromStatus}
      <span class="transition__flow">{fromStatus} &rarr; {event.status}</span>
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
    {#if expanded}
      <pre class="transition__tools" transition:slide={{ duration: 150 }}>{agentSnapshot?.pending_tool_calls}</pre>
    {/if}
  {/if}
</div>

<style>
  .transition {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: var(--space-2) 0;
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

  .transition__tools {
    font-size: var(--text-xs);
    max-height: 200px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-all;
  }
</style>
