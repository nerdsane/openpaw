<script lang="ts">
  import type { EntityBadgeData } from '$lib/canvas-types';
  import { Handle, Position } from '@xyflow/svelte';

  let { data }: { data: EntityBadgeData } = $props();
</script>

<div class="entity-badge">
  <Handle type="target" position={Position.Top} style="background: var(--text-3, #666); width: 4px; height: 4px;" />
  <span class="etype">{data.entityType}</span>
  <span class="elabel">{data.label}</span>
</div>

<style>
  .entity-badge {
    width: 100%; height: 100%;
    display: flex; align-items: center; gap: 4px;
    padding: 0 8px;
    background: var(--surface, #141414);
    border: 1px dashed var(--border, rgba(255,255,255,0.08));
    border-radius: 3px;
    font-family: var(--font-mono, monospace);
    cursor: pointer;
    font-size: 9px;
  }
  .entity-badge:hover { border-color: var(--text-3, #666); }
  .etype { color: var(--text-3, #666); text-transform: uppercase; letter-spacing: 0.04em; }
  .elabel { color: var(--text-2, #a0a0a0); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
