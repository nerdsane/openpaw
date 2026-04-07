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
    background: var(--terminal-bg);
    border: 1px dashed var(--terminal-border);
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    opacity: 0.6;
    transition: opacity var(--duration-fast) var(--ease);
  }

  .platform-cluster:hover {
    opacity: 1;
  }

  .header {
    padding: 8px 14px;
    border-bottom: 1px dashed var(--terminal-border);
    flex-shrink: 0;
  }

  .platform-label {
    font-family: var(--font-sans, var(--font-mono));
    font-size: 18px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-disabled);
  }

  .footer {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    padding: 6px 14px;
    border-top: 1px dashed var(--terminal-border);
  }

  .skill-badge {
    font-family: var(--font-mono);
    font-size: var(--label);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-disabled);
    padding: 1px 6px;
    border: 1px solid var(--terminal-border);
    border-radius: var(--radius-sm);
  }
</style>
