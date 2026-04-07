<script lang="ts">
  import type { AgentNodeData } from '$lib/canvas-types';
  import { parsePendingToolCalls, formatToolInput } from '$lib/parse';
  import StatusBadge from '../StatusBadge.svelte';

  let { data, selected }: { data: AgentNodeData; selected?: boolean } = $props();

  const TERMINAL = ['Completed', 'Failed', 'Cancelled'];

  let activeSessions = $derived(
    data.sessions.filter(s => !TERMINAL.includes(s.Status))
  );
  let hasActive = $derived(activeSessions.length > 0);
  let latestSession = $derived(activeSessions[0] ?? data.sessions[0]);

  // Extract recent tool calls from latest session events
  let recentCalls = $derived.by(() => {
    if (!latestSession) return [];
    const events = (latestSession as any)._events ?? [];
    const calls: Array<{ name: string; args: string }> = [];
    for (const e of events) {
      if (e.action === 'ProcessToolCalls' && e.params?.pending_tool_calls) {
        const tcs = parsePendingToolCalls(e.params.pending_tool_calls as string);
        for (const tc of tcs) {
          calls.push({ name: tc.name, args: formatToolInput(tc.name, tc.input) });
        }
      }
    }
    return calls.slice(-6);
  });

  let totalTurns = $derived(
    data.sessions.reduce((sum, s) => sum + ((s as any).turn_count ?? (s as any).fields?.turn_count ?? 0), 0)
  );

  let isWaiting = $derived(latestSession?.Status === 'WaitingForApproval');
</script>

<div
  class="agent-node"
  class:agent-node--active={hasActive}
  class:agent-node--waiting={isWaiting}
  class:agent-node--selected={selected}
>
  <!-- Title bar -->
  <div class="title">
    <span class="name">{data.agent.name}</span>
    <span class="role">{data.agent.role}</span>
    <StatusBadge status={hasActive ? (latestSession?.Status ?? 'Idle') : data.agent.Status} />
  </div>

  <!-- Terminal body -->
  <div class="terminal">
    {#if isWaiting}
      <div class="waiting">[AWAITING APPROVAL]</div>
    {/if}

    {#if recentCalls.length > 0}
      {#each recentCalls as tc}
        <div class="line">
          <span class="prompt">&gt;</span>
          <span class="cmd">{tc.name}</span>
          <span class="args">{tc.args}</span>
        </div>
      {/each}
    {:else if hasActive}
      <div class="idle"># thinking...</div>
    {:else}
      <div class="idle"># idle</div>
    {/if}
  </div>

  <!-- Footer -->
  <div class="footer">
    <span class="stat">{totalTurns}t</span>
    <span class="stat">{data.sessions.length} sess</span>
    {#if hasActive}
      <span class="stat active-dot">live</span>
    {/if}
  </div>
</div>

<style>
  .agent-node {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg, #0a0a0a);
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    border-radius: var(--radius, 4px);
    overflow: hidden;
    font-family: var(--font-mono, monospace);
    cursor: pointer;
    transition: border-color 150ms ease, box-shadow 150ms ease;
  }

  .agent-node:hover {
    border-color: var(--text-2, #a0a0a0);
  }

  .agent-node--active {
    border-color: var(--accent, #00DC82);
    border-width: 1px;
    box-shadow: 0 0 8px rgba(0, 220, 130, 0.1);
  }

  .agent-node--waiting {
    border-color: var(--status-warning, #eab308);
    box-shadow: 0 0 8px rgba(234, 179, 8, 0.15);
  }

  .agent-node--selected {
    border-color: var(--text-1, #ededed);
  }

  /* Title */
  .title {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    background: var(--surface, #141414);
    border-bottom: 1px solid var(--border, rgba(255,255,255,0.08));
    flex-shrink: 0;
  }

  .name {
    font-size: 12px;
    font-weight: 600;
    color: var(--accent, #00DC82);
  }

  .role {
    font-size: 10px;
    color: var(--text-3, #666);
  }

  /* Terminal body */
  .terminal {
    flex: 1;
    padding: 6px 10px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    gap: 1px;
    font-size: 10px;
    line-height: 1.5;
  }

  .line {
    display: flex;
    gap: 4px;
    white-space: nowrap;
    overflow: hidden;
  }

  .prompt {
    color: var(--accent, #00DC82);
    flex-shrink: 0;
  }

  .cmd {
    color: var(--accent, #00DC82);
    flex-shrink: 0;
  }

  .args {
    color: var(--text-2, #a0a0a0);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .waiting {
    color: var(--status-warning, #eab308);
    font-weight: 600;
    font-size: 10px;
    margin-bottom: 2px;
  }

  .idle {
    color: var(--text-3, #666);
    font-size: 10px;
  }

  /* Footer */
  .footer {
    display: flex;
    gap: 8px;
    padding: 4px 10px;
    border-top: 1px solid var(--border, rgba(255,255,255,0.08));
    flex-shrink: 0;
  }

  .stat {
    font-size: 9px;
    color: var(--text-3, #666);
  }

  .active-dot {
    color: var(--accent, #00DC82);
  }

  .active-dot::before {
    content: '●';
    margin-right: 2px;
    font-size: 6px;
    vertical-align: middle;
  }
</style>
