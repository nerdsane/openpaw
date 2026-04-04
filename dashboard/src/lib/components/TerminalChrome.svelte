<script lang="ts">
  import StatusBadge from './StatusBadge.svelte';

  let { name, id, status, active }: {
    name: string;
    id: string;
    status: string;
    active: boolean;
  } = $props();

  let shortId = $derived(id ?? '');
</script>

<div class="terminal-chrome">
  <div class="left">
    <span class="name">{name}</span>
    <code class="id">{shortId}</code>
  </div>
  <div class="right">
    <StatusBadge {status} />
    {#if active}
      <span class="active-dot"></span>
    {/if}
  </div>
</div>

<style>
  .terminal-chrome {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 12px;
    background: var(--terminal-chrome);
    border-bottom: 1px solid var(--terminal-border);
    font-family: var(--font-mono);
    font-size: var(--label);
  }

  .left {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
  }

  .name {
    color: var(--terminal-text);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .id {
    color: var(--terminal-dim);
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.02em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .right {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
  }

  .active-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    animation: pulse 2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }

  @media (prefers-reduced-motion: reduce) {
    .active-dot {
      animation: none;
    }
  }
</style>
