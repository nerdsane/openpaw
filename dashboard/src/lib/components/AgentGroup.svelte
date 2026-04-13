<script lang="ts">
  import { base } from '$app/paths';
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import type { Session } from '$lib/types';
  import { resolveAgentName } from '$lib/stores/agents';
  import { computeMetrics } from '$lib/parse';
  import StatusBadge from './StatusBadge.svelte';

  let { agentKey, sessions }: { agentKey: string; sessions: Session[] } = $props();

  let agentLabel = $state<string | null>(null);
  let expanded = $state(false);

  onMount(async () => {
    const first = sessions[0];
    if (!first) return;
    agentLabel = await resolveAgentName(first);
  });

  // Sessions are already sorted newest-first by the parent
  let latest = $derived(sessions[0]);
  let totalMetrics = $derived.by(() => {
    let totalOut = 0;
    for (const s of sessions) {
      const m = computeMetrics(s._events ?? []);
      totalOut += m.totalOutputTokens;
    }
    return totalOut;
  });

  function relativeTime(ts: string | undefined): string {
    if (!ts) return '';
    const then = new Date(ts).getTime();
    if (isNaN(then)) return ts;
    const diff = Math.max(0, Date.now() - then);
    const seconds = Math.floor(diff / 1000);
    if (seconds < 60) return `${seconds}s ago`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    return `${Math.floor(hours / 24)}d ago`;
  }

  function truncate(s: string | undefined, len: number): string {
    if (!s) return '';
    return s.length > len ? s.slice(0, len) + '...' : s;
  }
</script>

<div class="agent-group">
  <button class="agent-group__header" onclick={() => expanded = !expanded}>
    <div class="agent-group__top">
      <span class="agent-group__name">{agentLabel ?? agentKey}</span>
      <span class="agent-group__count">{sessions.length} SESSION{sessions.length !== 1 ? 'S' : ''}</span>
      <span class="agent-group__time">{relativeTime(latest?.last_heartbeat_at)}</span>
    </div>
    <div class="agent-group__meta">
      <StatusBadge status={latest?.Status ?? 'Idle'} />
      <span class="agent-group__stats">
        <span class="agent-group__stat">{totalMetrics.toLocaleString()} OUTPUT TOKENS</span>
      </span>
      <span class="agent-group__expand">{expanded ? '[-]' : '[+]'}</span>
    </div>
    {#if latest?.user_message && !expanded}
      <p class="agent-group__task">{truncate(latest.user_message, 100)}</p>
    {/if}
  </button>

  {#if expanded}
    <div class="agent-group__sessions" transition:slide={{ duration: 150 }}>
      {#each sessions as session (session.Id)}
        <a href="{base}/sessions/{session.Id}" class="session-row">
          <StatusBadge status={session.Status} />
          <code class="session-row__id">{session.Id}</code>
          <span class="session-row__task">{truncate(session.user_message, 80)}</span>
          <span class="session-row__meta">
            <span>{session.turn_count ?? 0}t</span>
            <span>{relativeTime(session.last_heartbeat_at)}</span>
          </span>
        </a>
      {/each}
    </div>
  {/if}
</div>

<style>
  .agent-group {
    background: var(--terminal-bg);
    border: 1px solid var(--terminal-border);
    border-radius: var(--radius-md);
    overflow: hidden;
  }

  .agent-group__header {
    display: flex;
    flex-direction: column;
    gap: var(--space-sm);
    padding: var(--space-lg);
    width: 100%;
    text-align: left;
    cursor: pointer;
    font-family: var(--font-mono);
    transition: background var(--duration-fast) var(--ease);
  }

  .agent-group__header:hover {
    background: var(--accent-subtle);
  }

  .agent-group__top {
    display: flex;
    align-items: baseline;
    gap: var(--space-sm);
  }

  .agent-group__name {
    font-family: var(--font-mono);
    font-size: var(--body);
    font-weight: 500;
    color: var(--terminal-text);
  }

  .agent-group__count {
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.06em;
    color: var(--terminal-dim);
  }

  .agent-group__time {
    font-family: var(--font-mono);
    font-size: var(--label);
    color: var(--terminal-dim);
    margin-left: auto;
  }

  .agent-group__meta {
    display: flex;
    align-items: center;
    gap: var(--space-md);
  }

  .agent-group__stats {
    display: flex;
    gap: var(--space-md);
    margin-left: auto;
  }

  .agent-group__stat {
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.04em;
    color: var(--terminal-dim);
  }

  .agent-group__expand {
    font-family: var(--font-mono);
    font-size: var(--label);
    color: var(--terminal-dim);
    letter-spacing: 0.04em;
  }

  .agent-group__task {
    font-family: var(--font-mono);
    font-size: var(--body-sm);
    color: var(--text-secondary);
    line-height: 1.5;
  }

  .agent-group__sessions {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--terminal-border);
  }

  .session-row {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    padding: var(--space-sm) var(--space-lg);
    text-decoration: none;
    color: inherit;
    background: var(--terminal-bg);
    border-bottom: 1px solid var(--terminal-border);
    font-family: var(--font-mono);
    transition: background var(--duration-fast) var(--ease);
  }

  .session-row:last-child {
    border-bottom: none;
  }

  .session-row:hover {
    background: var(--accent-subtle);
    text-decoration: none;
  }

  .session-row__id {
    font-family: var(--font-mono);
    font-size: var(--label);
    color: var(--terminal-dim);
    flex-shrink: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-row__task {
    font-family: var(--font-mono);
    font-size: var(--caption);
    color: var(--text-secondary);
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .session-row__meta {
    display: flex;
    gap: var(--space-sm);
    font-family: var(--font-mono);
    font-size: var(--label);
    color: var(--terminal-dim);
    flex-shrink: 0;
    margin-left: auto;
  }
</style>
