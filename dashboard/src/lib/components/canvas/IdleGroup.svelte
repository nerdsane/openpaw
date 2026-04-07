<script lang="ts">
  import type { IdleGroupData } from '$lib/canvas-types';

  let { data }: { data: IdleGroupData } = $props();
</script>

<div class="idle-group">
  <span class="label">IDLE</span>
  {#each data.agents as agent}
    <span class="agent">{agent.name}</span>
  {/each}
</div>

<style>
  .idle-group {
    width: 100%; height: 100%;
    display: flex; align-items: center; gap: 8px;
    padding: 0 12px;
    font-family: var(--font-mono, monospace);
    opacity: 0.45;
  }

  .label {
    font-size: 9px;
    letter-spacing: 0.06em;
    color: var(--text-3, #71717a);
    flex-shrink: 0;
  }

  .agent {
    font-size: 10px;
    color: var(--text-3, #71717a);
  }

  .agent::after { content: ' ·'; color: var(--text-3, #71717a); opacity: 0.5; }
  .agent:last-child::after { content: ''; }
</style>
