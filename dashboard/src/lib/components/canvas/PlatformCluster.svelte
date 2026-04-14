<script lang="ts">
  import type { PlatformNodeData } from '$lib/canvas-types';

  let { data }: { data: PlatformNodeData } = $props();

  let skillCount = $derived(data.skills.length);
</script>

<div class="platform-cluster">
  <div class="header">
    <span class="platform-label">PLATFORM</span>
  </div>

  <!-- Body: child soul nodes render here via Svelte Flow grouping -->

  <div class="footer">
    <span class="skill-badge">{skillCount} SKILLS</span>
  </div>
</div>

<style>
  .platform-cluster {
    padding: 0;
    position: relative;
    width: 100%;
    height: 100%;
    background: var(--surface);
    border: 1px dashed var(--border);
    border-radius: var(--radius);
    display: flex;
    flex-direction: column;
    opacity: 0.6;
    transition: opacity var(--duration) var(--ease);
  }

  .platform-cluster:hover {
    opacity: 1;
  }

  .header {
    padding: var(--sp-2) var(--sp-3);
    border-bottom: 1px dashed var(--border);
    flex-shrink: 0;
  }

  .platform-label {
    font-family: var(--font-sans);
    font-size: var(--text-lg);
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-3);
  }

  .footer {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    padding: var(--sp-1) var(--sp-3);
    border-top: 1px dashed var(--border);
  }

  .skill-badge {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-3);
    padding: 1px var(--sp-1);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
</style>
