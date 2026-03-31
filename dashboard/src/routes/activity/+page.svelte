<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { slide, fade } from 'svelte/transition';
  import { connectSSE, disconnectSSE, events, type StateChangeEvent } from '$lib/sse';
  import { queryEntities } from '$lib/api';
  import type { EntityEvent } from '$lib/types';
  import EventEntry from '$lib/components/EventEntry.svelte';

  let entityFilter = $state<string | null>(null);
  let historicalEvents = $state<StateChangeEvent[]>([]);
  let loaded = $state(false);

  const filterOptions = ['All', 'Agent', 'WorkCycle', 'AlertCycle', 'Issue'] as const;
  const entitySets = ['Agents', 'WorkCycles', 'AlertCycles', 'Issues'] as const;
  const entityTypeMap: Record<string, string> = {
    Agents: 'Agent', WorkCycles: 'WorkCycle', AlertCycles: 'AlertCycle', Issues: 'Issue',
  };

  // Combine historical + live SSE events
  let allEvents = $derived.by(() => {
    // Live events first (newest), then historical
    const live = $events;
    const combined = [...live, ...historicalEvents];
    // Deduplicate by entity_id + action + status (rough)
    const seen = new Set<string>();
    return combined.filter((e) => {
      const key = `${e.entity_id}:${e.action}:${e.status}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    }).slice(0, 200);
  });

  let filteredEvents = $derived.by(() => {
    if (!entityFilter || entityFilter === 'All') return allEvents;
    return allEvents.filter((e) => e.entity_type === entityFilter);
  });

  onMount(async () => {
    // Load historical events from recent entities
    const allHistorical: StateChangeEvent[] = [];
    for (const entitySet of entitySets) {
      try {
        const entities = await queryEntities(entitySet, undefined, 'SequenceNr desc', 10);
        for (const entity of entities) {
          const events = (entity._events ?? []) as EntityEvent[];
          const entityType = entityTypeMap[entitySet] ?? entitySet;
          const entityId = (entity.Id ?? entity._entity_id ?? '') as string;
          for (const evt of events) {
            if (evt.action === 'Heartbeat') continue; // Skip noise
            allHistorical.push({
              seq: 0,
              entity_type: entityType,
              entity_id: entityId,
              action: evt.action,
              status: evt.to_status,
              tenant: 'default',
            });
          }
        }
      } catch { /* entity set may not exist yet */ }
    }
    // Sort by most recent first (reverse order of events array)
    historicalEvents = allHistorical.reverse();
    loaded = true;

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
        <p class="activity-empty-text">{loaded ? 'No events recorded' : 'Loading...'}</p>
      </div>
    {:else}
      {#each filteredEvents as event, i (event.seq || i)}
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
