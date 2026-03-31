<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { loadAgents, agents } from '$lib/stores/agents';
  import { connectSSE, disconnectSSE } from '$lib/sse';

  let loaded = $state(false);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      await loadAgents();
    } catch {
      // Server may not be running — show empty state gracefully
      error = 'Could not reach the API server';
    }
    loaded = true;
    connectSSE();
  });

  onDestroy(() => {
    disconnectSSE();
  });

  let hasAgents = $derived($agents.length > 0);
</script>

<div class="floor">
  <header class="floor-header">
    <h1>Operations Floor</h1>
    <p class="floor-subtitle">All agents across projects</p>
  </header>

  {#if !loaded}
    <div class="floor-empty">
      <p class="floor-empty-text">Loading...</p>
    </div>
  {:else if error}
    <div class="floor-empty">
      <div class="floor-watermark">
        <svg width="80" height="80" viewBox="0 0 64 64" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.12">
          <ellipse cx="32" cy="40" rx="12" ry="10" />
          <circle cx="18" cy="26" r="5" />
          <circle cx="29" cy="18" r="5" />
          <circle cx="41" cy="20" r="5" />
          <circle cx="48" cy="30" r="5" />
        </svg>
      </div>
      <p class="floor-empty-text">{error}</p>
    </div>
  {:else if !hasAgents}
    <div class="floor-empty">
      <div class="floor-watermark">
        <svg width="80" height="80" viewBox="0 0 64 64" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.12">
          <ellipse cx="32" cy="40" rx="12" ry="10" />
          <circle cx="18" cy="26" r="5" />
          <circle cx="29" cy="18" r="5" />
          <circle cx="41" cy="20" r="5" />
          <circle cx="48" cy="30" r="5" />
        </svg>
      </div>
      <p class="floor-empty-text">No active agents</p>
    </div>
  {:else}
    <div class="floor-grid">
      <!-- Agent cards will be rendered here in Phase 2 -->
    </div>
  {/if}
</div>

<style>
  .floor {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .floor-header {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .floor-subtitle {
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .floor-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    padding: var(--space-8) 0;
  }

  .floor-watermark {
    color: var(--text-tertiary);
    margin-bottom: var(--space-1);
  }

  .floor-empty-text {
    color: var(--text-tertiary);
    font-size: var(--text-sm);
  }

  .floor-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: var(--space-2);
  }
</style>
