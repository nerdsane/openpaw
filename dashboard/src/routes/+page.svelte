<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { fade } from 'svelte/transition';
  import { loadSessions, sessions, activeSessions } from '$lib/stores/sessions';
  import { connectSSE, disconnectSSE, events } from '$lib/sse';
  import { refreshSession } from '$lib/stores/sessions';
  import SessionCard from '$lib/components/SessionCard.svelte';
  import PawLogo from '$lib/components/PawLogo.svelte';

  let loaded = $state(false);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      await loadSessions();
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
    if (latest.entity_type === 'Session') {
      refreshSession(latest.entity_id);
    }
  });

  let completedSessions = $derived(
    $sessions.filter((a) => ['Completed', 'Failed', 'Cancelled'].includes(a.Status))
  );

  let hasActive = $derived($activeSessions.length > 0);
  let hasCompleted = $derived(completedSessions.length > 0);
  let hasAny = $derived($sessions.length > 0);
</script>

<div class="floor">
  <header class="floor-header">
    <h1>Factory Floor</h1>
    <p class="floor-subtitle">Active work across all projects</p>
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
      <p class="floor-empty-text">No active sessions</p>
    </div>
  {:else}
    {#if hasActive}
      <section class="floor-section">
        <h2 class="floor-section-title">Active Sessions</h2>
        <div class="floor-grid">
          {#each $activeSessions as session (session.Id)}
            <SessionCard {session} />
          {/each}
        </div>
      </section>
    {:else}
      <div class="floor-empty" transition:fade={{ duration: 200 }}>
        <div class="floor-watermark">
          <PawLogo size={80} />
        </div>
        <p class="floor-empty-text">No active sessions</p>
      </div>
    {/if}

    {#if hasCompleted}
      <section class="floor-section floor-section--recent">
        <h2 class="floor-section-title">Recent</h2>
        <div class="floor-grid floor-grid--dimmed">
          {#each completedSessions as session (session.Id)}
            <SessionCard {session} />
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

  /* Project Context */
  .project-context {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding-bottom: var(--space-4);
    border-bottom: 1px solid var(--border);
  }

  .context-block {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .context-heading {
    font-family: var(--font-serif);
    font-size: var(--text-base);
    font-weight: 500;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-size: 0.6875rem;
  }

  .project-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .project-card__name {
    font-size: var(--text-sm);
    font-weight: 500;
    color: var(--text-primary);
  }

  .project-card__url {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    word-break: break-all;
  }

  .project-card__stack {
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }

  .project-card__status {
    margin-top: 2px;
  }

  .team-roster {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .team-member {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--space-1) var(--space-2);
    background: var(--surface-raised);
    border-radius: var(--radius-sm);
  }

  .team-member__name {
    font-size: var(--text-sm);
    color: var(--text-primary);
    font-weight: 500;
  }

  .team-member__desc {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }

  .skills-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: var(--space-2);
  }

  .skill-card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: var(--space-2);
    background: var(--surface-raised);
    border-radius: var(--radius-sm);
  }

  .skill-card__header {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    cursor: pointer;
  }

  .skill-card__name {
    font-size: var(--text-sm);
    color: var(--text-primary);
    font-weight: 500;
  }

  .skill-card__scope {
    font-family: var(--font-mono);
    font-size: 0.625rem;
    color: var(--text-secondary);
    background: var(--surface-overlay);
    padding: 1px 6px;
    border-radius: var(--radius-sm);
  }

  .skill-card__desc {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }

  .skill-card__toggle {
    font-size: var(--text-xs);
    color: var(--text-secondary);
    padding: 2px 0;
    text-align: left;
    width: fit-content;
    cursor: pointer;
    background: none;
    border: none;
  }

  .skill-card__toggle:hover {
    color: var(--text-primary);
  }

  .skill-card__content {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-secondary);
    background: var(--surface-overlay);
    padding: var(--space-2);
    border-radius: var(--radius-sm);
    white-space: pre-wrap;
    overflow-x: auto;
    max-height: 300px;
    overflow-y: auto;
  }

  .workcycle-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .workcycle-row {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    text-decoration: none;
    color: inherit;
    transition: transform var(--duration-fast) var(--ease);
  }

  .workcycle-row:hover {
    text-decoration: none;
    transform: translateY(-1px);
  }

  .workcycle-row--dimmed {
    opacity: 0.6;
  }

  .workcycle-row__top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }

  .workcycle-row__task {
    font-size: var(--text-sm);
    color: var(--text-primary);
  }

  .workcycle-row__gates {
    margin-top: 2px;
  }

  .workcycle-row__pr {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    word-break: break-all;
  }

  @media (max-width: 700px) {
    .floor-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
