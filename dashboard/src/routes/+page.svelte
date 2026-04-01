<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { loadAgents, agents, activeAgents } from '$lib/stores/agents';
  import { connectSSE, disconnectSSE, events } from '$lib/sse';
  import { refreshAgent } from '$lib/stores/agents';
  import { queryEntities } from '$lib/api';
  import type { ProjectHarness, Soul, WorkCycle, Skill } from '$lib/types';
  import AgentCard from '$lib/components/AgentCard.svelte';
  import GatePipeline from '$lib/components/GatePipeline.svelte';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import PawLogo from '$lib/components/PawLogo.svelte';

  let loaded = $state(false);
  let error = $state<string | null>(null);

  let projects = $state<ProjectHarness[]>([]);
  let souls = $state<Soul[]>([]);
  let workcycles = $state<WorkCycle[]>([]);
  let skills = $state<Skill[]>([]);

  onMount(async () => {
    try {
      await loadAgents();
    } catch {
      error = 'Could not reach the API server';
    }
    loaded = true;
    connectSSE();

    // Load project context in parallel (non-blocking)
    const [p, s, w, sk] = await Promise.all([
      queryEntities('ProjectHarnesses', undefined, undefined, 10).catch(() => []),
      queryEntities('Souls', undefined, undefined, 20).catch(() => []),
      queryEntities('WorkCycles', undefined, 'SequenceNr desc', 20).catch(() => []),
      queryEntities('Skills', undefined, undefined, 20).catch(() => []),
    ]);
    projects = p as unknown as ProjectHarness[];
    souls = s as unknown as Soul[];
    workcycles = w as unknown as WorkCycle[];
    skills = sk as unknown as Skill[];
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
  let hasContext = $derived(projects.length > 0 || souls.length > 0 || workcycles.length > 0);

  let activeWorkCycles = $derived(
    workcycles.filter((w) => !['Completed', 'Failed', 'Cancelled'].includes(w.Status))
  );
  let recentWorkCycles = $derived(
    workcycles.filter((w) => ['Completed', 'Failed', 'Cancelled'].includes(w.Status)).slice(0, 5)
  );
</script>

<div class="floor">
  <header class="floor-header">
    <h1>Factory Floor</h1>
    <p class="floor-subtitle">All agents across projects</p>
  </header>

  {#if hasContext}
    <section class="project-context">
      {#if projects.length > 0}
        <div class="context-block">
          <h2 class="context-heading">Project</h2>
          {#each projects as project (project.Id)}
            <div class="project-card card">
              <div class="project-card__name">{project.project_name || project.Id}</div>
              {#if project.repo_url}
                <code class="project-card__url">{project.repo_url}</code>
              {/if}
              {#if project.tech_stack}
                <span class="project-card__stack">{project.tech_stack}</span>
              {/if}
              <div class="project-card__status">
                <StatusBadge status={project.Status} />
              </div>
            </div>
          {/each}
        </div>
      {/if}

      {#if souls.length > 0}
        <div class="context-block">
          <h2 class="context-heading">Team</h2>
          <div class="team-roster">
            {#each souls as soul (soul.Id)}
              <div class="team-member">
                <span class="team-member__name">{soul.name || soul.Id}</span>
                {#if soul.description}
                  <span class="team-member__desc">{soul.description}</span>
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {/if}

      {#if activeWorkCycles.length > 0}
        <div class="context-block">
          <h2 class="context-heading">Active Work Cycles</h2>
          <div class="workcycle-list">
            {#each activeWorkCycles as wc (wc.Id)}
              <a href="/entities/WorkCycle/{wc.Id}" class="workcycle-row card">
                <div class="workcycle-row__top">
                  <span class="workcycle-row__task">{wc.task_summary || 'Untitled cycle'}</span>
                  <StatusBadge status={wc.Status} />
                </div>
                <div class="workcycle-row__gates">
                  <GatePipeline workcycle={wc} />
                </div>
                {#if wc.pr_url}
                  <code class="workcycle-row__pr">{wc.pr_url}</code>
                {/if}
              </a>
            {/each}
          </div>
        </div>
      {/if}

      {#if recentWorkCycles.length > 0}
        <div class="context-block">
          <h2 class="context-heading">Recent Work Cycles</h2>
          <div class="workcycle-list">
            {#each recentWorkCycles as wc (wc.Id)}
              <a href="/entities/WorkCycle/{wc.Id}" class="workcycle-row workcycle-row--dimmed card">
                <div class="workcycle-row__top">
                  <span class="workcycle-row__task">{wc.task_summary || 'Untitled cycle'}</span>
                  <StatusBadge status={wc.Status} />
                </div>
                <div class="workcycle-row__gates">
                  <GatePipeline workcycle={wc} />
                </div>
              </a>
            {/each}
          </div>
        </div>
      {/if}
    </section>
  {/if}

  {#if !loaded}
    <div class="floor-empty" >
      <p class="floor-empty-text">Loading...</p>
    </div>
  {:else if error}
    <div class="floor-empty" >
      <div class="floor-watermark">
        <PawLogo size={80} />
      </div>
      <p class="floor-empty-text">{error}</p>
    </div>
  {:else if !hasAny}
    <div class="floor-empty" >
      <div class="floor-watermark">
        <PawLogo size={80} />
      </div>
      <p class="floor-empty-text">No active agents</p>
    </div>
  {:else}
    {#if hasActive}
      <section class="floor-section">
        <h2 class="floor-section-title">Active Agents</h2>
        <div class="floor-grid">
          {#each $activeAgents as agent, i (agent.Id)}
            <div style:animation-delay="{i * 30}ms" class="card-enter">
              <AgentCard {agent} />
            </div>
          {/each}
        </div>
      </section>
    {:else}
      <div class="floor-empty" >
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
          {#each completedAgents as agent, i (agent.Id)}
            <div style:animation-delay="{i * 30}ms" class="card-enter">
              <AgentCard {agent} />
            </div>
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
    gap: var(--space-3);
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
    margin-top: var(--space-3);
  }

  .floor-section-title {
    font-size: var(--text-lg);
    color: var(--text-secondary);
  }

  .floor-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 8px;
  }

  .floor-grid--dimmed {
    opacity: 0.6;
  }

  .card-enter {
    animation: card-in 150ms var(--ease-out-quart) both;
  }

  @keyframes card-in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }

  /* Project Context */
  .project-context {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding-bottom: var(--space-3);
    border-bottom: 1px solid var(--border);
  }

  .context-block {
    display: flex;
    flex-direction: column;
    gap: 8px;
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
    gap: 8px;
  }

  .team-member {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 10px;
    background: var(--surface-raised);
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

  /* ---- Tablet ---- */
  @media (max-width: 1024px) {
    .floor-grid {
      grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    }
  }

  /* ---- Mobile ---- */
  @media (max-width: 768px) {
    .floor-grid {
      grid-template-columns: 1fr;
    }

    .floor-header h1 {
      font-size: var(--text-xl);
    }

    .team-roster {
      gap: 4px;
    }

    .team-member {
      font-size: var(--text-xs);
      padding: 4px 8px;
    }

    .workcycle-row__top {
      flex-wrap: wrap;
    }
  }
</style>
