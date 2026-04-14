<script lang="ts">
  import type { SoulNodeData } from '$lib/canvas-types';

  let { data }: { data: SoulNodeData } = $props();

  let soulName = $derived(data.soul.name || data.soul.Name);
  let description = $derived.by(() => {
    const desc = data.soul.description || data.soul.Description || '';
    return desc.length > 40 ? desc.slice(0, 40) + '...' : desc;
  });
</script>

<div class="soul-node">
  <span class="soul-dot"></span>
  <span class="soul-name">{soulName}</span>
  {#if description}
    <span class="soul-desc">{description}</span>
  {/if}
</div>

<style>
  .soul-node {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-1) var(--sp-2);
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    border-radius: var(--radius);
    min-width: 80px;
  }

  .soul-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--status-active);
    flex-shrink: 0;
  }

  .soul-name {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-2);
    white-space: nowrap;
  }

  .soul-desc {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
