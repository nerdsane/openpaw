<script lang="ts">
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import { fetchDecisions, fetchPolicies, queryEntities, queryTeams, queryAgentsForTeam, fetchFileContent, type PendingDecision, type PolicyEntry } from '$lib/api';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import GatePipeline from '$lib/components/GatePipeline.svelte';
  import type { WorkCycle, Agent, Team, Session, Plan } from '$lib/types';

  let loaded = $state(false);

  let harnesses = $state<Record<string, unknown>[]>([]);
  let teams = $state<Team[]>([]);
  let teamAgents = $state<Record<string, Agent[]>>({});
  let plans = $state<Plan[]>([]);
  let souls = $state<Record<string, unknown>[]>([]);
  let skills = $state<Record<string, unknown>[]>([]);
  let workCycles = $state<Record<string, unknown>[]>([]);
  let decisions = $state<PendingDecision[]>([]);
  let policies = $state<PolicyEntry[]>([]);

  let expandedAgentSessions = $state<string | null>(null);
  let agentSessionsCache = $state<Record<string, Session[]>>({});

  let expandedHarness = $state<string | null>(null);
  let expandedPolicy = $state<string | null>(null);
  let expandedSoul = $state<string | null>(null);
  let expandedSkill = $state<string | null>(null);

  let fileContentCache = $state<Record<string, string>>({});

  async function toggleFileContent(id: string, fileId: string, setter: 'soul' | 'skill') {
    if (setter === 'soul') {
      if (expandedSoul === id) { expandedSoul = null; return; }
      expandedSoul = id;
    } else {
      if (expandedSkill === id) { expandedSkill = null; return; }
      expandedSkill = id;
    }
    if (fileId && !fileContentCache[id]) {
      const content = await fetchFileContent(fileId);
      fileContentCache = { ...fileContentCache, [id]: content };
    }
  }

  async function toggleAgentSessions(agentId: string) {
    if (expandedAgentSessions === agentId) {
      expandedAgentSessions = null;
      return;
    }
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

  onMount(async () => {
    const [ha, tm, pl, so, sk, wc, dec, pol] = await Promise.all([
      queryEntities('Harnesses').catch(() => queryEntities('ProjectHarnesses').catch(() => [])),
      queryTeams().catch(() => []),
      queryEntities('Plans', undefined, 'Id desc', 20).catch(() => []),
      queryEntities('Souls').catch(() => []),
      queryEntities('Skills').catch(() => []),
      queryEntities('WorkCycles').catch(() => []),
      fetchDecisions().then(r => r.decisions).catch(() => []),
      fetchPolicies().catch(() => []),
    ]);
    harnesses = ha;
    teams = tm as unknown as Team[];
    plans = pl as unknown as Plan[];
    souls = so;
    skills = sk;
    workCycles = wc;
    decisions = dec;
    policies = pol;

    const agentMap: Record<string, Agent[]> = {};
    await Promise.all(teams.map(async (team) => {
      try {
        const agents = await queryAgentsForTeam(team.Id);
        agentMap[team.Id] = agents as unknown as Agent[];
      } catch {
        agentMap[team.Id] = [];
      }
    }));
    teamAgents = agentMap;

    loaded = true;
  });

  function field(entity: Record<string, unknown>, key: string): string {
    return (entity[key] ?? entity[key.charAt(0).toUpperCase() + key.slice(1)] ?? '') as string;
  }

  function shortId(id: string): string {
    return id?.slice(0, 12) ?? '';
  }

  function statusColor(status: string): string {
    if (['approved', 'Active', 'Completed', 'Complete'].includes(status)) return 'var(--status-success)';
    if (['denied', 'expired', 'Failed', 'Cancelled'].includes(status)) return 'var(--status-error)';
    if (['pending', 'WaitingForApproval', 'Reviewing', 'Testing'].includes(status)) return 'var(--status-warning)';
    if (['Thinking', 'Executing', 'InProgress', 'Planning'].includes(status)) return 'var(--status-active)';
    return 'var(--status-idle)';
  }
</script>

<div class="page">
  <header class="page-header">
    <span class="page-label">PROJECT</span>
    <p class="page-desc">Operating context: harnesses, team, skills, and authorization</p>
  </header>

  {#if !loaded}
    <div class="empty"><span class="empty-text">[LOADING...]</span></div>
  {:else}

    <!-- HARNESS -->
    {#if harnesses.length > 0}
    <section class="section">
      <span class="section-label">HARNESS ({harnesses.length})</span>
      <p class="section-desc">Project harnesses govern what agents can do within a repository</p>
        {#each harnesses as harness (field(harness, 'Id'))}
          {@const harnessId = field(harness, 'Id')}
          <div class="list-row">
            <div class="list-header" onclick={() => expandedHarness = expandedHarness === harnessId ? null : harnessId}>
              <span class="list-dot" style:background={statusColor(field(harness, 'Status'))}></span>
              <span class="list-name">{field(harness, 'repo_url').replace('https://github.com/', '') || 'Unnamed'}</span>
              <StatusBadge status={field(harness, 'Status')} />
              <span class="list-meta">{field(harness, 'tech_stack')}</span>
            </div>

            {#if expandedHarness === harnessId}
              <div class="list-detail" transition:slide={{ duration: 150 }}>
                <div class="detail-grid">
                  <span class="detail-label">HARNESS ID</span>
                  <code class="detail-value">{harnessId}</code>

                  <span class="detail-label">REPOSITORY</span>
                  <span class="detail-value">{field(harness, 'repo_url')}</span>

                  <span class="detail-label">TECH STACK</span>
                  <span class="detail-value">{field(harness, 'tech_stack')}</span>

                  {#if field(harness, 'conventions')}
                    <span class="detail-label">CONVENTIONS</span>
                    <pre class="detail-pre">{field(harness, 'conventions')}</pre>
                  {/if}
                </div>
              </div>
            {/if}

            <div class="harness-flow">
              <div class="flow-row">
                <div class="flow-state">PLANNING</div>
                <span class="flow-arrow">&rarr;</span>
                <div class="flow-state">PLANNED</div>
                <span class="flow-arrow">&rarr;</span>
                <div class="flow-state">IN PROGRESS</div>
                <span class="flow-arrow">&rarr;</span>
                <div class="flow-gates">
                  <span class="flow-gates-label">L1 GATES</span>
                  <div class="flow-gate">MIGRATIONS</div>
                  <div class="flow-gate">TYPECHECK</div>
                  <div class="flow-gate">UNIT TESTS</div>
                </div>
                <span class="flow-arrow">&rarr;</span>
                <div class="flow-state">TESTING</div>
                <span class="flow-arrow">&rarr;</span>
                <div class="flow-gates">
                  <span class="flow-gates-label">L2 GATES</span>
                  <div class="flow-gate">DST</div>
                  <div class="flow-gate">POLICY</div>
                </div>
                <span class="flow-arrow">&rarr;</span>
                <div class="flow-state">REVIEWING</div>
                <span class="flow-arrow">&rarr;</span>
                <div class="flow-state flow-state--final">COMPLETE</div>
              </div>
            </div>
          </div>
        {/each}
    </section>
    {/if}

    <!-- TEAMS & AGENTS -->
    {#if teams.length > 0}
    <section class="section">
      <span class="section-label">TEAM ({teams.length})</span>
      <p class="section-desc">Persistent agent members for this project</p>
        {#each teams as team (team.Id)}
          <div class="team-block">
            <div class="team-header">
              <span class="card-dot" style:background={statusColor(team.Status)}></span>
              <span class="team-name">{team.name || 'Unnamed Team'}</span>
              <StatusBadge status={team.Status} />
            </div>
            {#if team.description}
              <p class="team-desc">{team.description}</p>
            {/if}

            {#if (teamAgents[team.Id] ?? []).length === 0}
              <span class="empty-text">No agents in this team</span>
            {:else}
              <div class="card-grid">
                {#each teamAgents[team.Id] ?? [] as agent (agent.Id)}
                  <div class="card agent-member-card">
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
                      {#if agent.tools_enabled}
                        <span class="meta-tag">TOOLS: {agent.tools_enabled}</span>
                      {/if}
                      {#if agent.skill_ids}
                        <span class="meta-tag">SKILLS: {agent.skill_ids}</span>
                      {/if}
                    </div>
                    <StatusBadge status={agent.Status} />
                    <button class="view-btn" onclick={() => toggleAgentSessions(agent.Id)}>
                      {expandedAgentSessions === agent.Id ? '[-] HIDE SESSIONS' : '[+] VIEW SESSIONS'}
                    </button>
                    {#if expandedAgentSessions === agent.Id}
                      <div class="agent-sessions" transition:slide={{ duration: 150 }}>
                        {#if (agentSessionsCache[agent.Id] ?? []).length === 0}
                          <span class="empty-text">No sessions for this agent</span>
                        {:else}
                          {#each agentSessionsCache[agent.Id] ?? [] as sess (sess.Id)}
                            <a href="/sessions/{sess.Id}" class="agent-session-row">
                              <span class="agent-session-id">{sess.Id.slice(0, 8)}</span>
                              <StatusBadge status={sess.Status} />
                              {#if sess.user_message}
                                <span class="agent-session-task">{sess.user_message.length > 60 ? sess.user_message.slice(0, 60) + '...' : sess.user_message}</span>
                              {/if}
                            </a>
                          {/each}
                        {/if}
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
    </section>
    {/if}

    <!-- PLANS -->
    {#if plans.length > 0}
    <section class="section">
      <span class="section-label">PLANS ({plans.length})</span>
      <p class="section-desc">Agent plans — research, review, implementation</p>
        <div class="card-grid">
          {#each plans as plan (plan.Id)}
            <div class="card" class:plan-escalated={plan.Status === 'Escalated'} class:plan-review={plan.Status === 'UnderReview'}>
              <div class="card-header">
                <StatusBadge status={plan.Status} />
                <code class="card-id">{plan.Id?.slice(0, 8)}</code>
              </div>
              <h4 class="card-name">{plan.Description || plan.description || 'Untitled Plan'}</h4>
              {#if plan.AuthorAgentId || plan.author_agent_id}
                <p class="card-meta">Author: <code>{(plan.AuthorAgentId || plan.author_agent_id)?.slice(0, 12)}</code></p>
              {/if}
              {#if plan.ReviewerAgentId || plan.reviewer_agent_id}
                <p class="card-meta">Reviewer: <code>{(plan.ReviewerAgentId || plan.reviewer_agent_id)?.slice(0, 12)}</code></p>
              {/if}
              {#if plan.ReviewNotes || plan.review_notes}
                <p class="card-meta review-notes">Notes: {plan.ReviewNotes || plan.review_notes}</p>
              {/if}
              {#if plan.PlanText || plan.plan_text}
                <details class="plan-detail">
                  <summary>View Plan</summary>
                  <pre class="plan-text">{plan.PlanText || plan.plan_text}</pre>
                </details>
              {/if}
            </div>
          {/each}
        </div>
    </section>
    {/if}

    <!-- SOULS -->
    {#if souls.length > 0}
    <section class="section">
      <span class="section-label">SOULS ({souls.length})</span>
      <p class="section-desc">Agent personalities and identity definitions</p>
        <div class="card-grid">
          {#each souls as soul (field(soul, 'Id'))}
            {@const soulId = field(soul, 'Id')}
            {@const soulFileId = field(soul, 'ContentFileId') || field(soul, 'content_file_id')}
            <div class="card">
              <div class="card-header">
                <span class="card-dot" style:background="var(--terminal-text)"></span>
                <span class="card-name">{field(soul, 'Name') || field(soul, 'name') || 'Unnamed'}</span>
              </div>
              <p class="card-desc">{field(soul, 'Description') || field(soul, 'description') || '--'}</p>
              <StatusBadge status={field(soul, 'Status')} />
              {#if soulFileId}
                <button class="view-btn" onclick={() => toggleFileContent(soulId, soulFileId, 'soul')}>
                  {expandedSoul === soulId ? '[-] HIDE' : '[+] VIEW SOUL'}
                </button>
              {/if}
              {#if expandedSoul === soulId && fileContentCache[soulId]}
                <pre class="file-content" transition:slide={{ duration: 150 }}>{fileContentCache[soulId]}</pre>
              {/if}
            </div>
          {/each}
        </div>
    </section>
    {/if}

    <!-- SKILLS -->
    {#if skills.length > 0}
    <section class="section">
      <span class="section-label">SKILLS ({skills.length})</span>
      <p class="section-desc">Capabilities available to agents</p>
        <div class="card-grid">
          {#each skills as skill (field(skill, 'Id'))}
            {@const skillId = field(skill, 'Id')}
            {@const skillFileId = field(skill, 'content_file_id') || field(skill, 'ContentFileId')}
            <div class="card">
              <div class="card-header">
                <span class="card-dot" style:background="var(--accent)"></span>
                <span class="card-name">{field(skill, 'Name') || field(skill, 'name') || 'Unnamed'}</span>
              </div>
              <p class="card-desc">{field(skill, 'Description') || field(skill, 'description') || '--'}</p>
              <div class="card-meta-row">
                {#if field(skill, 'scope')}
                  <span class="meta-tag">SCOPE: {field(skill, 'scope')}</span>
                {/if}
                {#if field(skill, 'agent_filter')}
                  <span class="meta-tag">FILTER: {field(skill, 'agent_filter')}</span>
                {/if}
              </div>
              {#if skillFileId}
                <button class="view-btn" onclick={() => toggleFileContent(skillId, skillFileId, 'skill')}>
                  {expandedSkill === skillId ? '[-] HIDE' : '[+] VIEW'}
                </button>
              {/if}
              {#if expandedSkill === skillId && fileContentCache[skillId]}
                <pre class="file-content" transition:slide={{ duration: 150 }}>{fileContentCache[skillId]}</pre>
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
      <p class="section-desc">Planned changes with quality gates</p>
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
                  <span class="meta-tag">PLANNER: {field(wc, 'planner_id').slice(0, 8)}</span>
                {/if}
                {#if field(wc, 'pr_url')}
                  <span class="meta-tag">PR: {field(wc, 'pr_url')}</span>
                {/if}
              </div>
              <div class="wc-gates">
                <span class="gate-item" class:gate-item--pass={wc.has_plan}>PLAN</span>
                <span class="gate-item" class:gate-item--pass={wc.tests_passed}>TESTS</span>
              </div>
              {#if field(wc, 'plan_summary')}
                <p class="wc-summary">{field(wc, 'plan_summary')}</p>
              {/if}
            </div>
          {/each}
        </div>
    </section>
    {/if}

  {/if}
</div>

<style>
  .page { display: flex; flex-direction: column; gap: var(--space-xl); }

  .page-header { display: flex; flex-direction: column; gap: var(--space-xs); }
  .page-label {
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.08em;
    color: var(--text-secondary);
  }
  .page-desc { font-size: var(--body-sm); color: var(--text-secondary); }

  .section { display: flex; flex-direction: column; gap: var(--space-md); }
  .section-label {
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.08em;
    color: var(--text-secondary);
  }
  .section-desc { font-size: var(--caption); color: var(--text-disabled); }

  .empty { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: var(--space-sm); padding: var(--space-3xl) 0; }
  .empty-text {
    font-family: var(--font-mono);
    font-size: var(--caption);
    color: var(--text-disabled);
    letter-spacing: 0.04em;
  }
  .empty-note {
    font-family: var(--font-mono);
    font-size: var(--caption);
    color: var(--text-disabled);
  }

  /* Cards grid */
  .card-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: var(--space-md); }
  .card {
    display: flex; flex-direction: column; gap: 6px;
    padding: var(--space-md); background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--radius-md);
  }
  .card-header { display: flex; align-items: center; gap: var(--space-sm); }
  .card-dot { width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }
  .card-name { font-size: var(--body-sm); font-weight: 500; color: var(--text-display); }
  .card-id { font-family: var(--font-mono); font-size: var(--label); color: var(--text-disabled); }
  .card-desc { font-size: var(--caption); color: var(--text-secondary); line-height: 1.4; }
  .card-meta-row { display: flex; flex-wrap: wrap; gap: 6px; }
  .meta-tag {
    font-family: var(--font-mono); font-size: var(--label); color: var(--text-secondary);
    letter-spacing: 0.04em;
    background: var(--surface-raised); padding: 1px 8px; border-radius: var(--radius-sm);
  }
  .view-btn {
    font-family: var(--font-mono); font-size: var(--label); color: var(--text-disabled);
    letter-spacing: 0.06em;
    padding: var(--space-2xs) 0; text-align: left; width: fit-content;
    cursor: pointer; background: none; border: none;
  }
  .view-btn:hover { color: var(--text-display); }
  .file-content {
    font-family: var(--font-mono); font-size: var(--label); color: var(--text-secondary);
    background: var(--surface-raised); border: 1px solid var(--border);
    padding: var(--space-md); border-radius: var(--radius-sm);
    white-space: pre-wrap; overflow-x: auto; max-height: 400px; overflow-y: auto;
  }

  /* List rows */
  .list-row { border-bottom: 1px solid var(--border); }
  .list-header {
    display: flex; align-items: center; gap: var(--space-sm);
    padding: var(--space-md) 0; cursor: pointer;
    transition: background var(--duration-fast) var(--ease);
  }
  .list-header:hover { background: var(--accent-subtle); }
  .list-dot { width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }
  .list-name { font-size: var(--body-sm); font-weight: 500; color: var(--text-display); }
  .list-meta { font-family: var(--font-mono); font-size: var(--label); color: var(--text-disabled); }

  /* Detail panels */
  .list-detail {
    padding: var(--space-md) var(--space-lg); background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md); margin-bottom: var(--space-sm);
  }
  .detail-grid { display: grid; grid-template-columns: 120px 1fr; gap: 6px var(--space-md); align-items: baseline; }
  .detail-label {
    font-family: var(--font-mono); font-size: var(--label);
    letter-spacing: 0.08em; color: var(--text-disabled);
  }
  .detail-value { font-size: var(--body-sm); color: var(--text-primary); word-break: break-all; }
  .detail-pre {
    font-family: var(--font-mono); font-size: var(--label); color: var(--text-secondary);
    background: var(--surface-raised); border: 1px solid var(--border);
    padding: var(--space-md); border-radius: var(--radius-sm);
    white-space: pre-wrap; overflow-x: auto; max-height: 300px; overflow-y: auto; grid-column: 1 / -1;
  }

  /* Team blocks */
  .team-block {
    display: flex; flex-direction: column; gap: var(--space-md);
    padding: var(--space-md) 0; border-bottom: 1px solid var(--border);
  }
  .team-header { display: flex; align-items: center; gap: var(--space-sm); }
  .team-name { font-size: var(--body-sm); font-weight: 500; color: var(--text-display); }
  .team-desc { font-size: var(--caption); color: var(--text-secondary); }
  .agent-role {
    font-family: var(--font-mono); font-size: var(--label); color: var(--text-secondary);
    letter-spacing: 0.06em;
    background: var(--surface-raised); padding: 1px 8px; border-radius: var(--radius-sm); width: fit-content;
  }
  .agent-member-card { cursor: default; }

  /* Agent sessions */
  .agent-sessions {
    display: flex; flex-direction: column; gap: var(--space-xs);
    padding-top: var(--space-sm);
  }
  .agent-session-row {
    display: flex; align-items: center; gap: var(--space-sm);
    padding: var(--space-xs) var(--space-sm); background: var(--surface-raised);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    text-decoration: none; color: inherit; font-size: var(--caption);
    transition: border-color var(--duration-fast) var(--ease);
  }
  .agent-session-row:hover { border-color: var(--border-visible); text-decoration: none; }
  .agent-session-id { font-family: var(--font-mono); font-size: var(--label); color: var(--text-disabled); }
  .agent-session-task { color: var(--text-secondary); }

  /* Harness flow diagram */
  .harness-flow {
    background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: var(--space-lg); overflow-x: auto;
  }
  .flow-row {
    display: flex; align-items: center; gap: var(--space-sm); flex-wrap: wrap;
    min-width: max-content;
  }
  .flow-state {
    font-family: var(--font-mono); font-size: var(--label); letter-spacing: 0.06em;
    padding: 6px 14px; border-radius: var(--radius-sm);
    background: var(--surface-raised); border: 1px solid var(--border);
    color: var(--text-primary); white-space: nowrap;
  }
  .flow-state--final {
    background: var(--accent); color: #000; border-color: var(--accent);
    font-weight: 700;
  }
  .flow-arrow {
    font-family: var(--font-mono);
    color: var(--text-disabled); font-size: var(--body-sm);
  }
  .flow-gates {
    display: flex; flex-direction: column; gap: var(--space-xs); align-items: center;
    padding: var(--space-sm) var(--space-md);
    border: 1px dashed var(--border-visible); border-radius: var(--radius-sm);
  }
  .flow-gates-label {
    font-family: var(--font-mono); font-size: var(--label);
    letter-spacing: 0.08em;
    color: var(--text-disabled);
  }
  .flow-gate {
    font-family: var(--font-mono); font-size: var(--label);
    letter-spacing: 0.04em;
    padding: 2px 8px;
    background: var(--surface-raised); color: var(--text-secondary);
    border-radius: var(--radius-sm);
  }

  /* Work Cycles */
  .wc-list {
    display: flex; flex-direction: column; gap: var(--space-sm);
  }
  .wc-row {
    display: flex; flex-direction: column; gap: var(--space-xs);
    padding: var(--space-md);
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius-md);
  }
  .wc-header {
    display: flex; align-items: center; gap: var(--space-sm);
  }
  .wc-task {
    font-size: var(--body-sm); font-weight: 500; color: var(--text-display);
  }
  .wc-meta {
    display: flex; gap: var(--space-sm); flex-wrap: wrap;
  }
  .wc-gates {
    display: flex; gap: var(--space-xs);
  }
  .gate-item {
    font-family: var(--font-mono); font-size: var(--label); letter-spacing: 0.06em;
    padding: 2px 8px; border-radius: var(--radius-sm);
    background: var(--surface-raised); color: var(--text-disabled);
    border: 1px solid var(--border);
  }
  .gate-item--pass {
    color: var(--accent); border-color: var(--accent);
    background: var(--accent-subtle);
  }
  .wc-summary {
    font-size: var(--caption); color: var(--text-secondary); line-height: 1.4;
  }

  /* Plan cards */
  .plan-escalated { border-color: var(--status-error) !important; border-width: 2px !important; }
  .plan-review { border-color: var(--status-warning) !important; border-width: 2px !important; }
  .review-notes { font-style: italic; color: var(--status-warning); }
  .plan-detail { margin-top: var(--space-xs); }
  .plan-detail summary { font-size: var(--label); color: var(--text-disabled); cursor: pointer; font-family: var(--font-mono); letter-spacing: 0.04em; }
  .plan-text { font-size: var(--caption); color: var(--text-secondary); white-space: pre-wrap; max-height: 300px; overflow-y: auto; margin-top: var(--space-xs); padding: var(--space-sm); background: var(--surface-raised); border-radius: var(--radius-sm); }
  .card-meta { font-size: var(--caption); color: var(--text-secondary); }

  @media (max-width: 800px) {
    .card-grid { grid-template-columns: 1fr; }
    .detail-grid { grid-template-columns: 1fr; }
    .flow-row { flex-direction: column; align-items: flex-start; }
  }
</style>
