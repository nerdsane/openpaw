<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { slide, fade } from 'svelte/transition';
  import { connectSSE, disconnectSSE, events, type StateChangeEvent } from '$lib/sse';
  import EventEntry from '$lib/components/EventEntry.svelte';

  let entityFilter = $state<string | null>(null);

  const filterOptions = ['All', 'Agent', 'WorkCycle', 'AlertCycle', 'Issue'] as const;

  let filteredEvents = $derived.by(() => {
    const evts = $events.slice(0, 200);
    if (!entityFilter || entityFilter === 'All') return evts;
    return evts.filter((e) => e.entity_type === entityFilter);
  });

  onMount(() => {
    connectSSE();
  });

  onDestroy(() => {
    disconnectSSE();
  });
</script>

<div class="activity">
  <header class="activity-header">
    <h1>Activity</h1>
  </header>

  <div class="filter-bar">
    {#each filterOptions as option}
      <button
        class="filter-btn"
        class:filter-btn--active={entityFilter === option || (option === 'All' && !entityFilter)}
        onclick={() => entityFilter = option === 'All' ? null : option}
      >
        {option}
      </button>
    {/each}
  </div>

  <div class="event-list">
    {#if filteredEvents.length === 0}
      <div class="activity-empty" transition:fade={{ duration: 200 }}>
        <p class="activity-empty-text">Waiting for events...</p>
      </div>
    {:else}
      {#each filteredEvents as event (event.seq)}
        <div transition:slide={{ duration: 150 }}>
          <EventEntry {event} />
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .activity {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .filter-bar {
    display: flex;
    gap: 4px;
  }

  .filter-btn {
    padding: 4px var(--space-1);
    font-size: var(--text-xs);
    color: var(--text-secondary);
    border-radius: var(--radius-sm);
    transition: color var(--duration-fast) var(--ease),
                background var(--duration-fast) var(--ease);
  }

  .filter-btn:hover {
    color: var(--text-primary);
    background: var(--brand-subtle);
  }

  .filter-btn--active {
    color: var(--text-primary);
    background: var(--surface-raised);
  }

  .event-list {
    display: flex;
    flex-direction: column;
  }

  .activity-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-8) 0;
  }

  .activity-empty-text {
    color: var(--text-tertiary);
    font-size: var(--text-sm);
  }
</style>
