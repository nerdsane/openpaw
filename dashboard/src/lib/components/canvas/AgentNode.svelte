<script lang="ts">
  import type { AgentNodeData } from '$lib/canvas-types';
  import { Handle, Position } from '@xyflow/svelte';

  let { data }: { data: AgentNodeData } = $props();

  let hasActive = $derived(data.activeSessionCount > 0);
</script>

<div class="agent-card" class:active={hasActive} class:pending={data.hasPendingDecision}>
  <span class="dot" class:dot--active={hasActive}></span>
  <span class="name">{data.agent.name}</span>
  <span class="role">{data.agent.role}</span>
  {#if data.sessionCount > 0}
    <span class="badge">{data.activeSessionCount}/{data.sessionCount}</span>
  {/if}
  <Handle type="source" position={Position.Bottom} style="background: var(--accent, #00DC82); width: 6px; height: 6px;" />
</div>

<style>
  .agent-card {
    width: 100%; height: 100%;
    display: flex; align-items: center; gap: 6px;
    padding: 0 10px;
    background: var(--surface, #141414);
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    border-radius: var(--radius, 4px);
    font-family: var(--font-mono, monospace);
    cursor: pointer;
    transition: border-color 150ms ease;
  }
  .agent-card:hover { border-color: var(--text-2, #a0a0a0); }
  .agent-card.active { border-color: var(--accent, #00DC82); }
  .agent-card.pending { border-color: var(--status-warning, #eab308); }

  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--text-3, #666); flex-shrink: 0; }
  .dot--active { background: var(--accent, #00DC82); }

  .name { font-size: 12px; font-weight: 600; color: var(--text-1, #ededed); }
  .role { font-size: 10px; color: var(--text-3, #666); }

  .badge {
    margin-left: auto; font-size: 9px; color: var(--accent, #00DC82);
    background: rgba(0,220,130,0.1); padding: 1px 5px; border-radius: 3px;
  }
</style>
