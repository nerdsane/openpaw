<script lang="ts">
  import type { SessionNodeData } from '$lib/canvas-types';
  import { parsePendingToolCalls, formatToolInput } from '$lib/parse';
  import { Handle, Position } from '@xyflow/svelte';
  import StatusBadge from '../StatusBadge.svelte';

  let { data }: { data: SessionNodeData } = $props();

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
    return calls.slice(-6);
  });

  let turns = $derived((data.session as any).turn_count ?? (data.session as any).fields?.turn_count ?? 0);
  let task = $derived.by(() => {
    const msg = (data.session as any).user_message ?? (data.session as any).fields?.user_message ?? '';
    return msg.length > 60 ? msg.slice(0, 60) + '...' : msg;
  });
</script>

<div class="session-node" class:active={isActive} class:waiting={isWaiting}>
  <Handle type="target" position={Position.Top} style="background: var(--accent, #00DC82); width: 6px; height: 6px;" />
  <Handle type="source" position={Position.Bottom} style="background: var(--text-3, #666); width: 4px; height: 4px;" />
  <div class="header">
    <span class="agent-name">{data.agentName}</span>
    <StatusBadge status={data.session.Status} />
  </div>

  {#if task}
    <div class="task">{task}</div>
  {/if}

  <div class="terminal">
    {#if isWaiting}
      <div class="waiting-banner">[AWAITING APPROVAL]</div>
    {/if}
    {#if recentCalls.length > 0}
      {#each recentCalls as tc}
        <div class="line">
          <span class="prompt">&gt;</span>
          <span class="cmd">{tc.name}</span>
          <span class="args">{tc.args}</span>
        </div>
      {/each}
    {:else if isActive}
      <div class="idle"># thinking...</div>
    {:else}
      <div class="idle"># done ({turns}t)</div>
    {/if}
  </div>

  <div class="footer">
    <span class="stat">{turns}t</span>
    {#if isActive}
      <span class="live">● live</span>
    {/if}
  </div>
</div>

<style>
  .session-node {
    width: 100%; height: 100%;
    display: flex; flex-direction: column;
    background: var(--bg, #0a0a0a);
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    border-radius: var(--radius, 4px);
    overflow: hidden;
    font-family: var(--font-mono, monospace);
    cursor: pointer;
    transition: border-color 150ms ease;
  }
  .session-node:hover { border-color: var(--text-2, #a0a0a0); }
  .session-node.active { border-color: var(--accent, #00DC82); box-shadow: 0 0 6px rgba(0,220,130,0.1); }
  .session-node.waiting { border-color: var(--status-warning, #eab308); box-shadow: 0 0 6px rgba(234,179,8,0.1); }

  .header {
    display: flex; align-items: center; gap: 6px;
    padding: 4px 8px;
    background: var(--surface, #141414);
    border-bottom: 1px solid var(--border, rgba(255,255,255,0.08));
    flex-shrink: 0;
  }
  .agent-name { font-size: 10px; font-weight: 600; color: var(--accent, #00DC82); }

  .task {
    padding: 3px 8px;
    font-size: 9px; color: var(--text-3, #666);
    border-bottom: 1px solid var(--border, rgba(255,255,255,0.08));
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    flex-shrink: 0;
  }

  .terminal {
    flex: 1; padding: 4px 8px; overflow: hidden;
    display: flex; flex-direction: column; gap: 1px;
    font-size: 10px; line-height: 1.5;
  }

  .line { display: flex; gap: 3px; white-space: nowrap; overflow: hidden; }
  .prompt { color: var(--accent, #00DC82); flex-shrink: 0; }
  .cmd { color: var(--accent, #00DC82); flex-shrink: 0; }
  .args { color: var(--text-2, #a0a0a0); overflow: hidden; text-overflow: ellipsis; }

  .waiting-banner { color: var(--status-warning, #eab308); font-weight: 600; font-size: 9px; }
  .idle { color: var(--text-3, #666); font-size: 9px; }

  .footer {
    display: flex; gap: 8px; padding: 3px 8px;
    border-top: 1px solid var(--border, rgba(255,255,255,0.08));
    flex-shrink: 0; font-size: 9px;
  }
  .stat { color: var(--text-3, #666); }
  .live { color: var(--accent, #00DC82); }
</style>
