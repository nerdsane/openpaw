<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { fade } from 'svelte/transition';
  import { loadSessions, sessions, activeSessions } from '$lib/stores/sessions';
  import { connectSSE, disconnectSSE, events } from '$lib/sse';
  import { refreshSession } from '$lib/stores/sessions';
  import type { Session } from '$lib/types';
  import TerminalWindow from '$lib/components/TerminalWindow.svelte';
  import AgentGroup from '$lib/components/AgentGroup.svelte';
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

  // SSE reactivity: refresh session on state_change
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

  // Group completed sessions by agent (agent_id or soul_id fallback),
  // sorted newest-first within each group and groups sorted by their latest session
  let agentGroups = $derived.by((): Array<{ key: string; sessions: Session[] }> => {
    const map = new Map<string, Session[]>();
    for (const s of completedSessions) {
      const key = s.agent_id || s.soul_id || 'unknown';
      const list = map.get(key) ?? [];
      list.push(s);
      map.set(key, list);
    }
    // Sort sessions within each group newest-first (by heartbeat or sequence)
    const groups = Array.from(map.entries()).map(([key, sessions]) => {
      sessions.sort((a, b) => {
        const ta = new Date(a.last_heartbeat_at || 0).getTime();
        const tb = new Date(b.last_heartbeat_at || 0).getTime();
        return tb - ta;
      });
      return { key, sessions };
    });
    // Sort groups by their latest session (newest group first)
    groups.sort((a, b) => {
      const ta = new Date(a.sessions[0]?.last_heartbeat_at || 0).getTime();
      const tb = new Date(b.sessions[0]?.last_heartbeat_at || 0).getTime();
      return tb - ta;
    });
    return groups;
  });

  let hasActive = $derived($activeSessions.length > 0);
  let hasCompleted = $derived(completedSessions.length > 0);
  let hasAny = $derived($sessions.length > 0);
</script>

<div class="floor" style="max-width: none">
  <header class="floor-header">
    <span class="floor-label">FACTORY FLOOR</span>
  </header>

  {#if !loaded}
    <div class="floor-empty" transition:fade={{ duration: 200 }}>
      <span class="floor-loading">[LOADING...]</span>
    </div>
  {:else if error}
    <div class="floor-empty" transition:fade={{ duration: 200 }}>
      <div class="floor-watermark">
        <PawLogo size={80} />
      </div>
      <span class="floor-error">[ERROR: {error}]</span>
    </div>
  {:else if !hasAny}
    <div class="floor-empty" transition:fade={{ duration: 200 }}>
      <div class="floor-watermark">
        <PawLogo size={80} />
      </div>
      <span class="floor-empty-text">NO ACTIVE SESSIONS</span>
    </div>
  {:else}
    {#if hasActive}
      <section class="floor-section">
        <span class="section-label">ACTIVE</span>
        <div class="terminal-grid">
          {#each $activeSessions as session (session.Id)}
            <TerminalWindow {session} />
          {/each}
        </div>
      </section>
    {:else}
      <div class="floor-empty" transition:fade={{ duration: 200 }}>
        <div class="floor-watermark">
          <PawLogo size={80} />
        </div>
        <span class="floor-empty-text">NO ACTIVE SESSIONS</span>
      </div>
    {/if}

    {#if hasCompleted}
      <section class="floor-section floor-section--recent">
        <span class="section-label">RECENT</span>
        <div class="recent-list">
          {#each agentGroups as group (group.key)}
            <AgentGroup agentKey={group.key} sessions={group.sessions} />
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
    gap: var(--space-xl);
  }

  .floor-header {
    display: flex;
    flex-direction: column;
    gap: var(--space-xs);
  }

  .floor-label {
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.08em;
    color: var(--text-secondary);
  }

  .floor-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-md);
    padding: var(--space-4xl) 0;
  }

  .floor-watermark {
    color: var(--text-disabled);
    opacity: 0.15;
  }

  .floor-loading,
  .floor-error,
  .floor-empty-text {
    font-family: var(--font-mono);
    font-size: var(--caption);
    letter-spacing: 0.06em;
    color: var(--text-disabled);
  }

  .floor-error {
    color: var(--status-error);
  }

  .floor-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
  }

  .floor-section--recent {
    margin-top: var(--space-lg);
  }

  .section-label {
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.08em;
    color: var(--text-secondary);
  }

  .terminal-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(420px, 1fr));
    gap: var(--space-md);
  }

  .recent-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
    opacity: 0.7;
  }

  @media (max-width: 500px) {
    .terminal-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
