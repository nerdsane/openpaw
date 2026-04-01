<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { fade } from 'svelte/transition';
  import { loadSessions, sessions, activeSessions } from '$lib/stores/sessions';
  import { connectSSE, disconnectSSE, events } from '$lib/sse';
  import { refreshSession } from '$lib/stores/sessions';
  import { queryEntities, fetchFileContent } from '$lib/api';
  import type { ProjectHarness, Soul, WorkCycle, Skill } from '$lib/types';
  import SessionCard from '$lib/components/SessionCard.svelte';
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

    // Load project context (non-blocking)
    try { projects = (await queryEntities('ProjectHarnesses', undefined, undefined, 10)) as unknown as ProjectHarness[]; } catch { /* may not exist */ }
    try { souls = (await queryEntities('Souls', undefined, undefined, 20)) as unknown as Soul[]; } catch { /* may not exist */ }
    try { workcycles = (await queryEntities('WorkCycles', undefined, 'SequenceNr desc', 20)) as unknown as WorkCycle[]; } catch { /* may not exist */ }
    try { skills = (await queryEntities('Skills', undefined, undefined, 20)) as unknown as Skill[]; } catch { /* may not exist */ }
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
  let hasContext = $derived(projects.length > 0 || souls.length > 0 || workcycles.length > 0 || skills.length > 0);

  // Expandable skill content on the floor page
  let expandedSkillId = $state<string | null>(null);
  let skillContentCache = $state<Record<string, string>>({});

  async function toggleSkillContent(skill: Skill) {
    const id = skill.Id;
    if (expandedSkillId === id) {
      expandedSkillId = null;
      return;
    }
    expandedSkillId = id;
    const fileId = skill.content_file_id;
    if (fileId && !skillContentCache[id]) {
      const content = await fetchFileContent(fileId);
      skillContentCache = { ...skillContentCache, [id]: content };
    }
  }

  let activeWorkCycles = $derived(
    workcycles.filter((w) => !['Completed', 'Failed', 'Cancelled'].includes(w.Status))
  );
  let recentWorkCycles = $derived(
    workcycles.filter((w) => ['Completed', 'Failed', 'Cancelled'].includes(w.Status)).slice(0, 5)
  );
</script>

<div class="floor">
  <header class="floor-header">
    <h1>Operations Floor</h1>
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

      {#if skills.length > 0}
        <div class="context-block">
          <h2 class="context-heading">Skills</h2>
          <div class="skills-grid">
            {#each skills as skill (skill.Id)}
              <div class="skill-card">
                <div class="skill-card__header" onclick={() => toggleSkillContent(skill)} role="button" tabindex="0" onkeydown={(e) => { if (e.key === 'Enter') toggleSkillContent(skill); }}>
                  <span class="skill-card__name">{skill.name || skill.Name || skill.Id}</span>
                  {#if skill.scope}
                    <span class="skill-card__scope">{skill.scope}</span>
                  {/if}
                </div>
                {#if skill.description || skill.Description}
                  <span class="skill-card__desc">{skill.description || skill.Description}</span>
                {/if}
                {#if skill.content_file_id}
                  <button class="skill-card__toggle" onclick={() => toggleSkillContent(skill)}>
                    {expandedSkillId === skill.Id ? 'Hide content' : 'View content'}
                  </button>
                {/if}
                {#if expandedSkillId === skill.Id && skillContentCache[skill.Id]}
                  <pre class="skill-card__content">{skillContentCache[skill.Id]}</pre>
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
        <h2 class="floor-section-title">Active Agents</h2>
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
