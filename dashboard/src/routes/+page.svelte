<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { fade } from 'svelte/transition';
  import { loadAgents, agents, activeAgents } from '$lib/stores/agents';
  import { connectSSE, disconnectSSE, events } from '$lib/sse';
  import { refreshAgent } from '$lib/stores/agents';
  import AgentCard from '$lib/components/AgentCard.svelte';
  import PawLogo from '$lib/components/PawLogo.svelte';

  let loaded = $state(false);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      await loadAgents();
    } catch {
      error = 'Could not reach the API server';
    }
    loaded = true;
    connectSSE();
  });

  onDestroy(() => {
    disconnectSSE();
  });

  // SSE reactivity: refresh agent on state_change
  let lastSeq = $state(0);
  $effect(() => {
    const evts = $events;
    if (evts.length === 0) return;
    const latest = evts[0];
    if (latest.seq <= lastSeq) return;
    lastSeq = latest.seq;
    if (latest.entity_type === 'Agent') {
      refreshAgent(latest.entity_id);
    }
  });

  let completedAgents = $derived(
    $agents.filter((a) => ['Completed', 'Failed', 'Cancelled'].includes(a.Status))
  );

  let hasActive = $derived($activeAgents.length > 0);
  let hasCompleted = $derived(completedAgents.length > 0);
  let hasAny = $derived($agents.length > 0);
</script>

<div class="floor">
  <header class="floor-header">
    <h1>Operations Floor</h1>
    <p class="floor-subtitle">All agents across projects</p>
  </header>

  {#if !loaded}
    <div class="floor-empty" transition:fade={{ duration: 200 }}>
      <p class="floor-empty-text">Loading...</p>
    </div>
  {:else if error}
    <div class="floor-empty" transition:fade={{ duration: 200 }}>
      <div class="floor-watermark">
        <PawLogo size={80} />
      </div>
      <p class="floor-empty-text">{error}</p>
    </div>
  {:else if !hasAny}
    <div class="floor-empty" transition:fade={{ duration: 200 }}>
      <div class="floor-watermark">
        <PawLogo size={80} />
      </div>
      <p class="floor-empty-text">No active agents</p>
    </div>
  {:else}
    {#if hasActive}
      <section class="floor-section">
        <div class="floor-grid">
          {#each $activeAgents as agent (agent.Id)}
            <AgentCard {agent} />
          {/each}
        </div>
      </section>
    {:else}
      <div class="floor-empty" transition:fade={{ duration: 200 }}>
        <div class="floor-watermark">
          <PawLogo size={80} />
        </div>
        <p class="floor-empty-text">No active agents</p>
      </div>
    {/if}

    {#if hasCompleted}
      <section class="floor-section floor-section--recent">
        <h2 class="floor-section-title">Recent</h2>
        <div class="floor-grid floor-grid--dimmed">
          {#each completedAgents as agent (agent.Id)}
            <AgentCard {agent} />
          {/each}
        </div>
      </section>
    {/if}
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
    opacity: 0.12;
    margin-bottom: var(--space-1);
  }

  .floor-empty-text {
    color: var(--text-tertiary);
    font-size: var(--text-sm);
  }

  .floor-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .floor-section--recent {
    margin-top: var(--space-4);
  }

  .floor-section-title {
    font-size: var(--text-lg);
    color: var(--text-secondary);
  }

  .floor-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: var(--space-2);
  }

  .floor-grid--dimmed {
    opacity: 0.6;
  }

  @media (max-width: 700px) {
    .floor-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
