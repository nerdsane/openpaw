<script lang="ts">
  import { onMount } from 'svelte';
  import type { Session, EntityEvent } from '$lib/types';
  import { resolveAgentName } from '$lib/stores/agents';
  import { parsePendingToolCalls, formatToolInput, computeMetrics } from '$lib/parse';
  import StatusBadge from './StatusBadge.svelte';

  let { session }: { session: Session } = $props();

  let agentName = $state<string>('...');

  onMount(async () => {
    agentName = await resolveAgentName(session);
  });

  let isActive = $derived(
    !['Completed', 'Failed', 'Cancelled'].includes(session.Status)
  );

  let isWaiting = $derived(session.Status === 'WaitingForApproval');

  /** Extract last 5 tool calls from session events */
  let recentToolCalls = $derived.by(() => {
    const events = session._events ?? [];
    const toolEvents: Array<{ name: string; args: string }> = [];

    for (const event of events) {
      if (event.action === 'ProcessToolCalls' && event.params?.pending_tool_calls) {
        const raw = event.params.pending_tool_calls as string;
        const calls = parsePendingToolCalls(raw);
        for (const tc of calls) {
          toolEvents.push({
            name: tc.name,
            args: formatToolInput(tc.name, tc.input),
          });
        }
      }
    }

    return toolEvents.slice(-5);
  });

  let windowMetrics = $derived(computeMetrics(session._events ?? []));

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

  function truncate(s: string, len: number): string {
    return s.length > len ? s.slice(0, len) + '...' : s;
  }
</script>

<a href="/sessions/{session.Id}" class="terminal-window" class:active={isActive}>
  <div class="title-bar">
    <div class="title-bar__left">
      <span class="title-bar__name">{agentName}</span>
      <span class="title-bar__id">{session.Id}</span>
    </div>
    <div class="title-bar__right">
      <StatusBadge status={session.Status} />
      {#if isActive}
        <span class="pulse-dot"></span>
      {/if}
    </div>
  </div>

  <div class="body">
    {#if isWaiting}
      <div class="approval-banner">[AWAITING APPROVAL]</div>
    {/if}

    {#if recentToolCalls.length > 0}
      {#each recentToolCalls as tc}
        <div class="tool-line">
          <span class="prompt">&gt;</span>
          <span class="tool-name">{tc.name}</span>
          <span class="tool-args">{truncate(tc.args, 60)}</span>
        </div>
      {/each}
    {:else}
      <div class="waiting"># WAITING...</div>
    {/if}
  </div>

  <div class="footer">
    <span class="footer-stat">TURNS: {windowMetrics.turns}</span>
    <span class="footer-stat">CTX: {windowMetrics.inputTokens.toLocaleString()}</span>
    <span class="footer-stat">OUT: {windowMetrics.totalOutputTokens.toLocaleString()}</span>
    <span class="footer-stat footer-stat--time">{relativeTime(session.last_heartbeat_at)}</span>
  </div>
</a>

<style>
  .terminal-window {
    display: flex;
    flex-direction: column;
    background: var(--terminal-bg);
    border: 1px solid var(--terminal-border);
    border-radius: var(--radius-md);
    overflow: hidden;
    text-decoration: none;
    color: inherit;
    transition: border-color var(--duration-fast) var(--ease);
  }

  .terminal-window:hover {
    border-color: var(--border);
    text-decoration: none;
  }

  .terminal-window.active {
    border-color: color-mix(in srgb, var(--accent) 30%, transparent);
  }

  .terminal-window.active:hover {
    border-color: var(--border);
  }

  .title-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 12px;
    background: var(--terminal-chrome);
    border-bottom: 1px solid var(--terminal-border);
    font-family: var(--font-mono);
    font-size: var(--label);
  }

  .title-bar__left {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    min-width: 0;
  }

  .title-bar__name {
    color: var(--terminal-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title-bar__id {
    color: var(--terminal-dim);
    flex-shrink: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title-bar__right {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    flex-shrink: 0;
  }

  .pulse-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--terminal-text);
    animation: pulse 2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }

  .body {
    min-height: 120px;
    max-height: 200px;
    overflow: hidden;
    padding: var(--space-sm) 12px;
    font-family: var(--font-mono);
    font-size: var(--label);
    line-height: 1.4;
    background: var(--terminal-bg);
  }

  .approval-banner {
    color: var(--status-warning);
    margin-bottom: var(--space-xs);
    font-weight: 600;
  }

  .tool-line {
    display: flex;
    gap: 6px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .prompt {
    color: var(--terminal-text);
    flex-shrink: 0;
  }

  .tool-name {
    color: var(--terminal-text);
    flex-shrink: 0;
  }

  .tool-args {
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .waiting {
    color: var(--terminal-dim);
    padding-top: var(--space-md);
  }

  .footer {
    display: flex;
    align-items: center;
    gap: var(--space-md);
    padding: 4px 12px;
    border-top: 1px solid var(--terminal-border);
    font-family: var(--font-mono);
    font-size: var(--label);
    color: var(--terminal-dim);
  }

  .footer-stat--time {
    margin-left: auto;
  }
</style>
