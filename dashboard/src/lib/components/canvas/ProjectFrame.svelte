<script lang="ts">
  import type { ProjectNodeData } from '$lib/canvas-types';

  let { data }: { data: ProjectNodeData } = $props();

  let projectName = $derived(data.team.name.replace(/\s*Team$/i, ''));
  let activeWc = $derived(data.workCycles.filter(wc => !['Completed', 'Cancelled'].includes(wc.Status)));
</script>

<div class="frame">
  <div class="header">
    <span class="name">{projectName}</span>
    {#if data.harness?.tech_stack}
      <span class="stack">{data.harness.tech_stack}</span>
    {/if}
  </div>

  <!-- Agents render inside via Svelte Flow grouping -->

  {#if data.workCycles.length > 0}
    <div class="cycles">
      <span class="cycles-label">{data.workCycles.length} cycles</span>
      {#if activeWc.length > 0}
        <span class="cycles-active">{activeWc.length} active</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .frame {
    width: 100%;
    height: 100%;
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    border-radius: 6px;
    background: rgba(255,255,255,0.015);
    position: relative;
  }

  .header {
    display: flex;
    align-items: baseline;
    gap: 12px;
    padding: 16px 20px;
  }

  .name {
    font-family: var(--font-sans, Inter, sans-serif);
    font-size: 22px;
    font-weight: 600;
    color: var(--text-1, #ededed);
    letter-spacing: -0.02em;
  }

  .stack {
    font-family: var(--font-mono, monospace);
    font-size: 10px;
    color: var(--text-3, #666);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .cycles {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    display: flex;
    gap: 8px;
    padding: 8px 20px;
    border-top: 1px solid var(--border, rgba(255,255,255,0.08));
    font-family: var(--font-mono, monospace);
    font-size: 10px;
  }

  .cycles-label { color: var(--text-3, #666); }
  .cycles-active { color: var(--accent, #00DC82); }
</style>
