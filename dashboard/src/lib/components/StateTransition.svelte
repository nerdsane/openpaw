<script lang="ts">
  import { slide } from 'svelte/transition';
  import type { StateChangeEvent } from '$lib/sse';
  import type { Session } from '$lib/types';
  import StatusBadge from './StatusBadge.svelte';

  let {
    event,
    agentSnapshot,
    timestamp,
    fromStatus,
    authz,
  }: {
    event: StateChangeEvent;
    agentSnapshot?: Session;
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

  let shortId = $derived(event.entity_id ?? '');

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
    {#if authz}
      <span class="authz-badge" class:authz-badge--allowed={authz.allowed} class:authz-badge--denied={!authz.allowed}>
        {authz.allowed ? 'ALLOWED' : 'DENIED'}
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
        <span class="counter">TURNS: {turnDelta}</span>
      {/if}
      {#if tokenInfo}
        <span class="counter">IN: {tokenInfo.input.toLocaleString()}</span>
        <span class="counter">OUT: {tokenInfo.output.toLocaleString()}</span>
      {/if}
    </div>
  {/if}

  {#if hasToolCalls}
    <button
      class="transition__expand"
      onclick={() => expanded = !expanded}
    >
      {expanded ? '[-] HIDE' : '[+] SHOW'} TOOL CALLS
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
    padding: var(--space-md) 0;
    border-bottom: 1px solid var(--border);
  }

  .transition__header {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
  }

  .transition__action {
    font-family: var(--font-body);
    font-size: var(--body-sm);
    color: var(--text-primary);
    font-weight: 500;
  }

  .transition__flow {
    font-family: var(--font-mono);
    font-size: var(--label);
    color: var(--text-disabled);
    letter-spacing: 0.04em;
  }

  .transition__time {
    font-family: var(--font-mono);
    font-size: var(--label);
    color: var(--text-disabled);
    margin-left: auto;
  }

  .transition__counters {
    display: flex;
    gap: var(--space-md);
  }

  .counter {
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.06em;
    color: var(--text-disabled);
  }

  .transition__expand {
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.06em;
    color: var(--text-secondary);
    padding: var(--space-2xs) 0;
    text-align: left;
    width: fit-content;
  }

  .transition__expand:hover {
    color: var(--text-display);
  }

  .transition__tools {
    font-size: var(--label);
    max-height: 200px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-all;
    background: var(--surface);
    border: 1px solid var(--border);
    padding: var(--space-sm);
    border-radius: var(--radius-sm);
  }

  .authz-badge {
    font-family: var(--font-mono);
    font-size: var(--label);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 1px 8px;
    border-radius: var(--radius-sm);
  }
  .authz-badge--allowed {
    color: var(--status-success);
    background: var(--accent-subtle);
  }
  .authz-badge--denied {
    color: var(--status-error);
    background: rgba(232, 91, 129, 0.1);
  }
  .authz-resource {
    font-family: var(--font-mono);
    font-size: var(--label);
    color: var(--status-error);
  }
</style>
