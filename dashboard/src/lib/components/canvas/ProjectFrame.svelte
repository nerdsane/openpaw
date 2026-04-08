<script lang="ts">
  import type { ProjectNodeData } from '$lib/canvas-types';

  let { data }: { data: ProjectNodeData } = $props();

  let projectName = $derived((data.team.name || '').replace(/\s*Team$/i, ''));
  let activeWc = $derived(data.workCycles.filter(wc => !['Completed', 'Cancelled'].includes(wc.Status)));
</script>

<div class="frame">
  <div class="header">
    <div class="title-row">
      <span class="name">{projectName}</span>
      <span class="agents-count">{data.activeCount} active / {data.agentCount} agents</span>
    </div>
    {#if data.harness?.tech_stack}
      <span class="stack">{data.harness.tech_stack}</span>
    {/if}
  </div>

  <!-- Activity cards and idle group render inside via Svelte Flow grouping -->

  <div class="footer">
    {#if data.apps.length > 0}
      <div class="apps-row">
        {#each data.apps as app}
          <span class="app-pill">{app.replace('paw-', '')}</span>
        {/each}
      </div>
    {/if}
    {#if data.workCycles.length > 0}
      <span class="cycles">{data.workCycles.length} cycles{activeWc.length > 0 ? ` · ${activeWc.length} active` : ''}</span>
    {/if}
  </div>
</div>

<style>
  .frame {
    width: 100%; height: 100%;
    border: 1px solid var(--border, rgba(255,255,255,0.07));
    border-radius: 6px;
    background: rgba(255,255,255,0.01);
    position: relative;
    display: flex; flex-direction: column;
  }

  .header {
    display: flex; flex-direction: column; gap: 4px;
    padding: 12px 16px;
    flex-shrink: 0;
  }

  .title-row {
    display: flex; align-items: baseline; gap: 12px;
  }

  .name {
    font-family: var(--font-sans, Inter, sans-serif);
    font-size: 18px; font-weight: 600;
    color: var(--text-1, #fafafa);
    letter-spacing: -0.01em;
  }

  .agents-count {
    font-family: var(--font-mono, monospace);
    font-size: 10px;
    color: var(--text-3, #71717a);
  }

  .stack {
    font-family: var(--font-mono, monospace);
    font-size: 10px;
    color: var(--text-3, #71717a);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }

  .footer {
    position: absolute; bottom: 0; left: 0; right: 0;
    display: flex; align-items: center; gap: 12px;
    padding: 8px 16px;
    border-top: 1px solid var(--border, rgba(255,255,255,0.07));
    flex-shrink: 0;
  }

  .apps-row {
    display: flex; gap: 4px; flex-wrap: wrap;
  }

  .app-pill {
    font-family: var(--font-mono, monospace);
    font-size: 9px;
    color: var(--text-3, #71717a);
    background: var(--surface, #18181b);
    border: 1px solid var(--border, rgba(255,255,255,0.07));
    padding: 1px 5px;
    border-radius: 3px;
    white-space: nowrap;
  }

  .cycles {
    font-family: var(--font-mono, monospace);
    font-size: 10px;
    color: var(--text-3, #71717a);
    margin-left: auto;
  }
</style>
