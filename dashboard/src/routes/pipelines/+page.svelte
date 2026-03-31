<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { fade } from 'svelte/transition';
  import { loadWorkCycles, workcycles, refreshWorkCycle } from '$lib/stores/workcycles';
  import { connectSSE, disconnectSSE, events } from '$lib/sse';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import GatePipeline from '$lib/components/GatePipeline.svelte';

  let loaded = $state(false);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      await loadWorkCycles();
    } catch {
      error = 'Could not load work cycles';
    }
    loaded = true;
    connectSSE('WorkCycle');
  });

  onDestroy(() => {
    disconnectSSE();
  });

  // SSE reactivity
  let lastSeq = $state(0);
  $effect(() => {
    const evts = $events;
    if (evts.length === 0) return;
    const latest = evts[0];
    if (latest.seq <= lastSeq) return;
    lastSeq = latest.seq;
    if (latest.entity_type === 'WorkCycle') {
      refreshWorkCycle(latest.entity_id);
    }
  });

  let hasWorkCycles = $derived($workcycles.length > 0);
</script>

<div class="pipelines">
  <header class="pipelines-header">
    <h1>Work Cycles</h1>
  </header>

  {#if !loaded}
    <div class="pipelines-empty" transition:fade={{ duration: 200 }}>
      <p class="pipelines-empty-text">Loading...</p>
    </div>
  {:else if error}
    <div class="pipelines-empty" transition:fade={{ duration: 200 }}>
      <p class="pipelines-empty-text">{error}</p>
    </div>
  {:else if !hasWorkCycles}
    <div class="pipelines-empty" transition:fade={{ duration: 200 }}>
      <p class="pipelines-empty-text">No work cycles</p>
    </div>
  {:else}
    <div class="pipeline-list">
      {#each $workcycles as wc (wc.Id)}
        <a href="/entities/WorkCycle/{wc.Id}" class="pipeline-entry card">
          <div class="pipeline-entry__top">
            <div class="pipeline-entry__info">
              <span class="pipeline-entry__task">{wc.task_summary || 'Untitled cycle'}</span>
              <code class="pipeline-entry__id">{wc.Id?.slice(0, 8)}</code>
            </div>
            <StatusBadge status={wc.Status} />
          </div>
          {#if wc.planner_id}
            <div class="pipeline-entry__agent">
              <span class="pipeline-entry__label">Agent</span>
              <code class="pipeline-entry__agent-id">{wc.planner_id.slice(0, 8)}</code>
            </div>
          {/if}
          <div class="pipeline-entry__gates">
            <GatePipeline workcycle={wc} />
          </div>
        </a>
      {/each}
    </div>
  {/if}
</div>

<style>
  .pipelines {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .pipelines-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-8) 0;
  }

  .pipelines-empty-text {
    color: var(--text-tertiary);
    font-size: var(--text-sm);
  }

  .pipeline-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .pipeline-entry {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    text-decoration: none;
    color: inherit;
    transition: transform var(--duration-fast) var(--ease),
                box-shadow var(--duration-fast) var(--ease);
  }

  .pipeline-entry:hover {
    text-decoration: none;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
  }

  .pipeline-entry__top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-2);
  }

  .pipeline-entry__info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .pipeline-entry__task {
    font-size: var(--text-sm);
    color: var(--text-primary);
  }

  .pipeline-entry__id {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }

  .pipeline-entry__agent {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .pipeline-entry__label {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }

  .pipeline-entry__agent-id {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }

  .pipeline-entry__gates {
    margin-top: var(--space-1);
  }
</style>
