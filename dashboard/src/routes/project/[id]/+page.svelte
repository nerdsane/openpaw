<script lang="ts">
  import { base } from '$app/paths';
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import { page } from '$app/stores';
  import { getEntity, queryEntities, queryAgentsForTeam, fetchFileContent } from '$lib/api';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import GatePipeline from '$lib/components/GatePipeline.svelte';
  import type { Team, Agent, WorkCycle, Session } from '$lib/types';

  let teamId = $derived($page.params.id);

  let loaded = $state(false);
  let team = $state<Team | null>(null);
  let agents = $state<Agent[]>([]);
  let harness = $state<Record<string, unknown> | null>(null);
  let workCycles = $state<Record<string, unknown>[]>([]);
  let plans = $state<Record<string, unknown>[]>([]);

  let expandedAgentSessions = $state<string | null>(null);
  let agentSessionsCache = $state<Record<string, Session[]>>({});
  let expandedHarness = $state(false);

  async function toggleAgentSessions(agentId: string) {
    if (expandedAgentSessions === agentId) { expandedAgentSessions = null; return; }
    expandedAgentSessions = agentId;
    if (!agentSessionsCache[agentId]) {
      try {
        const data = await queryEntities('Sessions', `agent_id eq '${agentId}'`, 'SequenceNr desc', 20);
        agentSessionsCache = { ...agentSessionsCache, [agentId]: data as unknown as Session[] };
      } catch {
        agentSessionsCache = { ...agentSessionsCache, [agentId]: [] };
      }
    }
  }

  function field(entity: Record<string, unknown>, key: string): string {
    return (entity[key] ?? entity[key.charAt(0).toUpperCase() + key.slice(1)] ?? '') as string;
  }

  function shortId(id: string): string {
    return id ?? '';
  }

  function statusColor(status: string): string {
    if (['approved', 'Active', 'Completed', 'Complete'].includes(status)) return 'var(--status-success)';
    if (['denied', 'expired', 'Failed', 'Cancelled'].includes(status)) return 'var(--status-error)';
    if (['pending', 'WaitingForApproval', 'Reviewing', 'Testing'].includes(status)) return 'var(--status-warning)';
    if (['Thinking', 'Executing', 'InProgress', 'Planning'].includes(status)) return 'var(--status-active)';
    return 'var(--status-idle)';
  }

  onMount(async () => {
    if (!teamId) {
      loaded = true;
      return;
    }

    try {
      const teamData = await getEntity('Teams', teamId);
      team = teamData as unknown as Team;

      const [ag, wc, pl] = await Promise.all([
        queryAgentsForTeam(teamId).catch(() => []),
        queryEntities('WorkCycles').catch(() => []),
        queryEntities('Plans', undefined, 'Id desc', 20).catch(() => []),
      ]);
      agents = ag as unknown as Agent[];
      workCycles = wc;
      plans = pl;

      // Load harness if team has one
      const harnessId = team.harness_id;
      if (harnessId) {
        try {
          harness = await getEntity('Harnesses', harnessId);
        } catch {
          // Try ProjectHarnesses
          try { harness = await getEntity('ProjectHarnesses', harnessId); } catch {}
        }
      }
      // If no harness_id, try to find any harness
      if (!harness) {
        try {
          const harnesses = await queryEntities('Harnesses');
          if (harnesses.length > 0) harness = harnesses[0];
        } catch {
          try {
            const harnesses = await queryEntities('ProjectHarnesses');
            if (harnesses.length > 0) harness = harnesses[0];
          } catch {}
        }
      }
    } catch {}
    loaded = true;
  });
</script>

<div class="page">
  {#if !loaded}
    <div class="empty"><span class="empty-text">[LOADING...]</span></div>
  {:else if !team}
    <div class="empty"><span class="empty-text">[PROJECT NOT FOUND]</span></div>
  {:else}
    <header class="page-header">
      <span class="page-label">PROJECT</span>
      <h1 class="project-name">{(team.name || 'Unnamed').replace(/ Team$/i, '')}</h1>
      {#if team.description}
        <p class="project-desc">{team.description}</p>
      {/if}
      <StatusBadge status={team.Status} />
    </header>

    <!-- HARNESS -->
    {#if harness}
    <section class="section">
      <span class="section-label">HARNESS</span>
      <div class="harness-card">
        <button
          type="button"
          class="harness-header"
          onclick={() => expandedHarness = !expandedHarness}
        >
          <span class="harness-repo">{field(harness, 'repo_url').replace('https://github.com/', '')}</span>
          <span class="harness-stack">{field(harness, 'tech_stack')}</span>
          <span class="expand-icon">{expandedHarness ? '[-]' : '[+]'}</span>
        </button>
        {#if expandedHarness}
          <div class="harness-detail" transition:slide={{ duration: 150 }}>
            {#if field(harness, 'conventions')}
              <pre class="conventions">{field(harness, 'conventions')}</pre>
            {/if}
          </div>
        {/if}
      </div>

      <!-- Work Cycle Flow -->
      <div class="flow">
        <span class="flow-title">WORK CYCLE FLOW</span>
        <div class="flow-diagram">
          <!-- Row 1: States -->
          <div class="flow-row">
            <div class="flow-cell"><span class="flow-node">PLANNING</span></div>
            <div class="flow-arrow">→</div>
            <div class="flow-cell"><span class="flow-node">PLANNED</span></div>
            <div class="flow-arrow">→</div>
            <div class="flow-cell"><span class="flow-node">IN PROGRESS</span></div>
            <div class="flow-arrow">→</div>
            <div class="flow-cell flow-cell--gate">
              <span class="flow-gate-label">L1 GATES</span>
              <div class="flow-gate-list">
                <span class="flow-gate">MIGRATIONS</span>
                <span class="flow-gate">TYPECHECK</span>
                <span class="flow-gate">UNIT TESTS</span>
              </div>
            </div>
            <div class="flow-arrow">→</div>
            <div class="flow-cell"><span class="flow-node">TESTING</span></div>
            <div class="flow-arrow">→</div>
            <div class="flow-cell flow-cell--gate">
              <span class="flow-gate-label">L2 GATES</span>
              <div class="flow-gate-list">
                <span class="flow-gate">DST</span>
                <span class="flow-gate">POLICY</span>
              </div>
            </div>
            <div class="flow-arrow">→</div>
            <div class="flow-cell"><span class="flow-node">REVIEWING</span></div>
            <div class="flow-arrow">→</div>
            <div class="flow-cell"><span class="flow-node flow-node--final">COMPLETE</span></div>
          </div>
        </div>
      </div>
    </section>
    {/if}

    <!-- AGENTS -->
    {#if agents.length > 0}
    <section class="section">
      <span class="section-label">AGENTS ({agents.length})</span>
      <div class="card-grid">
        {#each agents as agent (agent.Id)}
          <div class="card">
            <div class="card-header">
              <span class="card-dot" style:background={statusColor(agent.Status)}></span>
              <span class="card-name">{agent.name || 'Unnamed'}</span>
              <code class="card-id">{shortId(agent.Id)}</code>
            </div>
            {#if agent.role}
              <span class="agent-role">{agent.role}</span>
            {/if}
            {#if agent.description}
              <p class="card-desc">{agent.description}</p>
            {/if}
            <div class="card-meta-row">
              {#if agent.model}
                <span class="meta-tag">{agent.model}</span>
              {/if}
              {#if agent.provider}
                <span class="meta-tag">{agent.provider}</span>
              {/if}
              {#if agent.soul_id}
                <span class="meta-tag">SOUL: {agent.soul_id}</span>
              {/if}
            </div>
            <StatusBadge status={agent.Status} />
            <button class="view-btn" onclick={() => toggleAgentSessions(agent.Id)}>
              {expandedAgentSessions === agent.Id ? '[-] HIDE SESSIONS' : '[+] VIEW SESSIONS'}
            </button>
            {#if expandedAgentSessions === agent.Id}
              <div class="agent-sessions" transition:slide={{ duration: 150 }}>
                {#if (agentSessionsCache[agent.Id] ?? []).length === 0}
                  <span class="empty-text-inline">NO SESSIONS</span>
                {:else}
                  {#each agentSessionsCache[agent.Id] ?? [] as sess (sess.Id)}
                    <a href="{base}/sessions/{sess.Id}" class="session-row">
                      <span class="session-id">{sess.Id}</span>
                      <StatusBadge status={sess.Status} />
                      {#if sess.user_message}
                        <span class="session-task">{sess.user_message.length > 60 ? sess.user_message.slice(0, 60) + '...' : sess.user_message}</span>
                      {/if}
                    </a>
                  {/each}
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </section>
    {/if}

    <!-- WORK CYCLES -->
    {#if workCycles.length > 0}
    <section class="section">
      <span class="section-label">WORK CYCLES ({workCycles.length})</span>
      <div class="wc-list">
        {#each workCycles as wc (field(wc, 'Id'))}
          <div class="wc-row">
            <div class="wc-header">
              <span class="card-dot" style:background={statusColor(field(wc, 'Status'))}></span>
              <span class="wc-task">{field(wc, 'task_summary') || 'Untitled'}</span>
              <StatusBadge status={field(wc, 'Status')} />
            </div>
            <div class="wc-meta">
              {#if field(wc, 'planner_id')}
                <span class="meta-tag">PLANNER: {field(wc, 'planner_id')}</span>
              {/if}
              {#if field(wc, 'pr_url')}
                <span class="meta-tag">PR: {field(wc, 'pr_url')}</span>
              {/if}
            </div>
            <div class="wc-gates">
              <span class="gate-item" class:gate-item--pass={wc.has_plan}>PLAN</span>
              <span class="gate-item" class:gate-item--pass={wc.tests_passed}>TESTS</span>
            </div>
          </div>
        {/each}
      </div>
    </section>
    {/if}

    <!-- PLANS -->
    {#if plans.length > 0}
    <section class="section">
      <span class="section-label">PLANS ({plans.length})</span>
      <div class="card-grid">
        {#each plans as plan (field(plan, 'Id'))}
          <div class="card">
            <div class="card-header">
              <StatusBadge status={field(plan, 'Status')} />
              <code class="card-id">{field(plan, 'Id')}</code>
            </div>
            <span class="card-name">{field(plan, 'Description') || field(plan, 'description') || 'Untitled'}</span>
            {#if field(plan, 'PlanText') || field(plan, 'plan_text')}
              <details class="plan-detail">
                <summary>[+] VIEW PLAN</summary>
                <pre class="plan-text">{field(plan, 'PlanText') || field(plan, 'plan_text')}</pre>
              </details>
            {/if}
          </div>
        {/each}
      </div>
    </section>
    {/if}

  {/if}
</div>

<style>
  .page { display: flex; flex-direction: column; gap: var(--sp-8); }

  .page-header { display: flex; flex-direction: column; gap: var(--sp-1); }
  .page-label {
    font-family: var(--font-mono); font-size: var(--text-xs);
    letter-spacing: 0.08em; color: var(--text-2);
  }
  .project-name {
    font-family: var(--font-sans); font-size: var(--text-xl);
    font-weight: 500; color: var(--text-1);
  }
  .project-desc { font-size: var(--text-sm); color: var(--text-2); line-height: 1.5; }

  .section { display: flex; flex-direction: column; gap: var(--sp-4); }
  .section-label {
    font-family: var(--font-mono); font-size: var(--text-xs);
    letter-spacing: 0.08em; color: var(--text-2);
  }

  .empty { display: flex; align-items: center; justify-content: center; padding: var(--sp-8) 0; }
  .empty-text, .empty-text-inline {
    font-family: var(--font-mono); font-size: var(--text-xs);
    color: var(--text-3); letter-spacing: 0.04em;
  }

  /* Harness */
  .harness-card {
    background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--radius); overflow: hidden;
  }
  .harness-header {
    display: flex; align-items: center; gap: var(--sp-4);
    padding: var(--sp-4) var(--sp-6); cursor: pointer;
    transition: background var(--duration) var(--ease);
  }
  .harness-header:hover { background: var(--accent-subtle); }
  .harness-repo { font-family: var(--font-mono); font-size: var(--text-sm); color: var(--text-1); }
  .harness-stack { font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-3); margin-left: auto; }
  .expand-icon { font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-3); }
  .harness-detail { padding: 0 var(--sp-6) var(--sp-6); }
  .conventions {
    font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-2);
    background: var(--bg); border: 1px solid var(--border);
    padding: var(--sp-4); border-radius: var(--radius);
    white-space: pre-wrap; max-height: 400px; overflow-y: auto;
  }

  /* Cards */
  .card-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: var(--sp-4); }
  .card {
    display: flex; flex-direction: column; gap: var(--sp-1);
    padding: var(--sp-4); background: var(--surface);
    border: 1px solid var(--border); border-radius: var(--radius);
  }
  .card-header { display: flex; align-items: center; gap: var(--sp-2); }
  .card-dot { width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }
  .card-name { font-size: var(--text-sm); font-weight: 500; color: var(--text-1); }
  .card-id { font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .card-desc { font-size: var(--text-xs); color: var(--text-2); line-height: 1.4; }
  .card-meta-row { display: flex; flex-wrap: wrap; gap: var(--sp-1); }
  .meta-tag {
    font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-2);
    letter-spacing: 0.04em;
    background: var(--surface-raised); padding: 1px var(--sp-2); border-radius: var(--radius);
  }
  .agent-role {
    font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-1);
    letter-spacing: 0.06em;
  }
  .view-btn {
    font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-3);
    letter-spacing: 0.06em; padding: 2px 0;
    text-align: left; width: fit-content; cursor: pointer;
  }
  .view-btn:hover { color: var(--text-1); }

  /* Agent sessions */
  .agent-sessions { display: flex; flex-direction: column; gap: var(--sp-1); padding-top: var(--sp-2); }
  .session-row {
    display: flex; align-items: center; gap: var(--sp-2);
    padding: var(--sp-1) var(--sp-2); background: var(--bg);
    border: 1px solid var(--border); border-radius: var(--radius);
    text-decoration: none; color: inherit; font-size: var(--text-xs);
    transition: border-color var(--duration) var(--ease);
  }
  .session-row:hover { border-color: var(--text-1); text-decoration: none; }
  .session-id { font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .session-task { color: var(--text-2); }

  /* Work Cycles */
  .wc-list { display: flex; flex-direction: column; gap: var(--sp-2); }
  .wc-row {
    display: flex; flex-direction: column; gap: var(--sp-1);
    padding: var(--sp-4); background: var(--surface);
    border: 1px solid var(--border); border-radius: var(--radius);
  }
  .wc-header { display: flex; align-items: center; gap: var(--sp-2); }
  .wc-task { font-size: var(--text-sm); font-weight: 500; color: var(--text-1); }
  .wc-meta { display: flex; gap: var(--sp-2); flex-wrap: wrap; }
  .wc-gates { display: flex; gap: var(--sp-1); }
  .gate-item {
    font-family: var(--font-mono); font-size: var(--text-xs); letter-spacing: 0.06em;
    padding: 2px var(--sp-2); border-radius: var(--radius);
    background: var(--surface-raised); color: var(--text-3);
    border: 1px solid var(--border);
  }
  .gate-item--pass {
    color: var(--accent); border-color: var(--accent);
    background: var(--accent-subtle);
  }

  /* Plans */
  .plan-detail { margin-top: var(--sp-1); }
  .plan-detail summary {
    font-family: var(--font-mono); font-size: var(--text-xs);
    color: var(--text-3); cursor: pointer; letter-spacing: 0.04em;
  }
  .plan-text {
    font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-2);
    white-space: pre-wrap; max-height: 300px; overflow-y: auto;
    margin-top: var(--sp-1); padding: var(--sp-2);
    background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius);
  }

  /* Work Cycle Flow Diagram */
  .flow {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin-top: var(--sp-4);
  }

  .flow-title {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.08em;
    color: var(--text-3);
  }

  .flow-diagram {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--sp-8) var(--sp-6);
    overflow-x: auto;
  }

  .flow-row {
    display: flex;
    align-items: center;
    gap: 0;
    min-width: min-content;
  }

  .flow-cell {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .flow-cell--gate {
    flex-direction: column;
    gap: var(--sp-1);
    padding: var(--sp-2) var(--sp-4);
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
  }

  .flow-arrow {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text-3);
    padding: 0 var(--sp-2);
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  .flow-node {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.06em;
    padding: var(--sp-2) var(--sp-4);
    border-radius: var(--radius);
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text-1);
    white-space: nowrap;
    text-align: center;
  }

  .flow-node--final {
    background: var(--accent);
    color: var(--bg);
    border-color: var(--accent);
    font-weight: 700;
  }

  .flow-gate-label {
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.1em;
    color: var(--text-3);
    text-align: center;
  }

  .flow-gate-list {
    display: flex;
    gap: var(--sp-1);
    flex-wrap: wrap;
    justify-content: center;
  }

  .flow-gate {
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.04em;
    padding: 3px var(--sp-2);
    background: var(--surface);
    border: 1px solid var(--border);
    color: var(--text-2);
    border-radius: var(--radius);
    white-space: nowrap;
  }

  /* Responsive: stack vertically on small screens */
  @media (max-width: 900px) {
    .flow-row {
      flex-direction: column;
      align-items: stretch;
      gap: 0;
    }

    .flow-cell {
      width: 100%;
    }

    .flow-cell--gate {
      margin: 0;
    }

    .flow-node {
      width: 100%;
      text-align: center;
    }

    .flow-arrow {
      justify-content: center;
      padding: var(--sp-1) 0;
    }

    .flow-arrow::after {
      content: '↓';
    }

    /* Hide the → text, show ↓ via ::after */
    .flow-arrow {
      font-size: 0;
    }

    .flow-arrow::after {
      font-size: var(--text-sm);
    }
  }

  @media (max-width: 800px) {
    .card-grid { grid-template-columns: 1fr; }
  }
</style>
