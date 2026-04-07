<script lang="ts">
  import type { ActivityCardData } from '$lib/canvas-types';
  import { parsePendingToolCalls, formatToolInput, computeMetrics } from '$lib/parse';
  import StatusBadge from '../StatusBadge.svelte';

  let { data }: { data: ActivityCardData } = $props();

  const TERMINAL = ['Completed', 'Failed', 'Cancelled'];
  let isActive = $derived(!TERMINAL.includes(data.session.Status));
  let isWaiting = $derived(data.session.Status === 'WaitingForApproval');

  let recentCalls = $derived.by(() => {
    const events = (data.session as any)._events ?? [];
    const calls: Array<{ name: string; args: string }> = [];
    for (const e of events) {
      if (e.action === 'ProcessToolCalls' && e.params?.pending_tool_calls) {
        const tcs = parsePendingToolCalls(e.params.pending_tool_calls as string);
        for (const tc of tcs) {
          calls.push({ name: tc.name, args: formatToolInput(tc.name, tc.input) });
        }
      }
    }
    return calls.slice(-5);
  });

  let metrics = $derived(computeMetrics((data.session as any)._events ?? []));
  let task = $derived.by(() => {
    const msg = (data.session as any).user_message ?? (data.session as any).fields?.user_message ?? '';
    return msg.length > 80 ? msg.slice(0, 80) + '...' : msg;
  });
</script>

<div class="card" class:card--active={isActive} class:card--waiting={isWaiting}>
  <!-- Agent identity bar -->
  <div class="agent-bar">
    <span class="dot" class:dot--active={isActive} class:dot--waiting={isWaiting}></span>
    <span class="agent-name">{data.agent.name}</span>
    {#if data.agent.role && data.agent.role !== data.agent.name}
      <span class="agent-role">{data.agent.role}</span>
    {/if}
    <StatusBadge status={data.session.Status} />
    <span class="turns">{metrics.turns}t</span>
    {#if metrics.inputTokens > 0}
      <span class="tokens">ctx:{(metrics.inputTokens / 1000).toFixed(1)}K</span>
    {/if}
  </div>

  <!-- Task -->
  {#if task}
    <div class="task">{task}</div>
  {/if}

  <!-- Terminal body -->
  <div class="terminal">
    {#if isWaiting}
      <div class="waiting">[AWAITING APPROVAL]</div>
      {#if (data.session as any).pending_tool_context ?? (data.session as any).fields?.pending_tool_context}
        <div class="waiting-ctx">{(data.session as any).pending_tool_context ?? (data.session as any).fields?.pending_tool_context}</div>
      {/if}
    {/if}
    {#if recentCalls.length > 0}
      {#each recentCalls as tc}
        <div class="line">
          <span class="prompt">&gt;</span>
          <span class="cmd">{tc.name}</span>
          <span class="args">{tc.args}</span>
        </div>
      {/each}
    {:else if isActive && !isWaiting}
      <div class="thinking"># thinking...</div>
    {/if}
  </div>
</div>

<style>
  .card {
    width: 100%; height: 100%;
    display: flex; flex-direction: column;
    background: var(--bg, #09090b);
    border: 1px solid var(--border, rgba(255,255,255,0.07));
    border-radius: var(--radius, 4px);
    overflow: hidden;
    font-family: var(--font-mono, monospace);
    cursor: pointer;
    transition: border-color 150ms ease;
  }
  .card:hover { border-color: var(--border-strong, rgba(255,255,255,0.14)); }
  .card--active { border-left: 2px solid var(--accent, #34d399); }
  .card--waiting { border-left: 2px solid var(--status-warning, #facc15); }

  .agent-bar {
    display: flex; align-items: center; gap: 6px;
    padding: 6px 10px;
    background: var(--surface, #18181b);
    border-bottom: 1px solid var(--border, rgba(255,255,255,0.07));
    flex-shrink: 0;
  }

  .dot { width: 5px; height: 5px; border-radius: 50%; background: var(--status-idle, #52525b); flex-shrink: 0; }
  .dot--active { background: var(--accent, #34d399); }
  .dot--waiting { background: var(--status-warning, #facc15); }

  .agent-name { font-size: 11px; font-weight: 600; color: var(--text-1, #fafafa); }
  .agent-role { font-size: 10px; color: var(--text-3, #71717a); }
  .turns { font-size: 9px; color: var(--text-3, #71717a); margin-left: auto; }
  .tokens { font-size: 9px; color: var(--text-3, #71717a); }

  .task {
    padding: 4px 10px;
    font-size: 10px; color: var(--text-2, #a1a1aa);
    border-bottom: 1px solid var(--border, rgba(255,255,255,0.07));
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    flex-shrink: 0;
  }

  .terminal {
    flex: 1; padding: 6px 10px; overflow: hidden;
    display: flex; flex-direction: column; gap: 2px;
    font-size: 11px; line-height: 1.5;
  }

  .line { display: flex; gap: 4px; white-space: nowrap; overflow: hidden; }
  .prompt { color: var(--accent, #34d399); flex-shrink: 0; user-select: none; }
  .cmd { color: var(--text-1, #fafafa); font-weight: 500; flex-shrink: 0; }
  .args { color: var(--text-3, #71717a); overflow: hidden; text-overflow: ellipsis; }

  .waiting { color: var(--status-warning, #facc15); font-size: 10px; font-weight: 500; }
  .waiting-ctx { color: var(--text-3, #71717a); font-size: 10px; }
  .thinking { color: var(--text-3, #71717a); font-size: 10px; }
</style>
