<script lang="ts">
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import { fetchDecisions, fetchPolicies, queryEntities, fetchFileContent, type PendingDecision, type PolicyEntry } from '$lib/api';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import GatePipeline from '$lib/components/GatePipeline.svelte';
  import type { WorkCycle } from '$lib/types';

  let loaded = $state(false);

  // Data
  let harnesses = $state<Record<string, unknown>[]>([]);
  let souls = $state<Record<string, unknown>[]>([]);
  let skills = $state<Record<string, unknown>[]>([]);
  let workCycles = $state<Record<string, unknown>[]>([]);
  let decisions = $state<PendingDecision[]>([]);
  let policies = $state<PolicyEntry[]>([]);

  // Expandable
  let expandedHarness = $state<string | null>(null);
  let expandedPolicy = $state<string | null>(null);
  let expandedSoul = $state<string | null>(null);
  let expandedSkill = $state<string | null>(null);

  // File content cache for souls and skills
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

  onMount(async () => {
    const [ha, so, sk, wc, dec, pol] = await Promise.all([
      queryEntities('ProjectHarnesses').catch(() => []),
      queryEntities('Souls').catch(() => []),
      queryEntities('Skills').catch(() => []),
      queryEntities('WorkCycles').catch(() => []),
      fetchDecisions().then(r => r.decisions).catch(() => []),
      fetchPolicies().catch(() => []),
    ]);
    harnesses = ha;
    souls = so;
    skills = sk;
    workCycles = wc;
    decisions = dec;
    policies = pol;
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
    <h1>Project</h1>
    <p class="subtitle">Operating context: harnesses, team, skills, work cycles, and authorization</p>
  </header>

  {#if !loaded}
    <div class="empty"><p class="empty-text">Loading...</p></div>
  {:else}

    <!-- HARNESS -->
    <section class="section">
      <h2 class="section-title">Harness</h2>
      <p class="section-desc">Project harnesses govern what agents can do within a repository</p>
      {#if harnesses.length === 0}
        <p class="empty-text">No harnesses configured</p>
      {:else}
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
                  <span class="detail-label">Harness ID</span>
                  <code class="detail-value">{harnessId}</code>

                  <span class="detail-label">Repository</span>
                  <span class="detail-value">{field(harness, 'repo_url')}</span>

                  <span class="detail-label">Tech Stack</span>
                  <span class="detail-value">{field(harness, 'tech_stack')}</span>

                  {#if field(harness, 'conventions')}
                    <span class="detail-label">Conventions</span>
                    <pre class="detail-pre">{field(harness, 'conventions')}</pre>
                  {/if}
                </div>
              </div>
            {/if}
          </div>
        {/each}
      {/if}
    </section>

    <!-- TEAM -->
    <section class="section">
      <h2 class="section-title">Team</h2>
      <p class="section-desc">Souls define agent identities and what roles they can take</p>
      {#if souls.length === 0}
        <p class="empty-text">No souls configured</p>
      {:else}
        <div class="card-grid">
          {#each souls as soul (field(soul, 'Id'))}
            {@const soulId = field(soul, 'Id')}
            {@const soulFileId = field(soul, 'ContentFileId') || field(soul, 'content_file_id')}
            <div class="card">
              <div class="card-header">
                <span class="card-dot" style:background={statusColor(field(soul, 'Status'))}></span>
                <span class="card-name">{field(soul, 'Name') || field(soul, 'name') || 'Unnamed'}</span>
                <code class="card-id">{shortId(soulId)}</code>
              </div>
              <p class="card-desc">{field(soul, 'Description') || field(soul, 'description') || '--'}</p>
              <StatusBadge status={field(soul, 'Status')} />
              {#if soulFileId}
                <button class="view-btn" onclick={() => toggleFileContent(soulId, soulFileId, 'soul')}>
                  {expandedSoul === soulId ? 'Hide Soul' : 'View Soul'}
                </button>
              {/if}
              {#if expandedSoul === soulId && fileContentCache[soulId]}
                <pre class="file-content" transition:slide={{ duration: 150 }}>{fileContentCache[soulId]}</pre>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <!-- SKILLS -->
    <section class="section">
      <h2 class="section-title">Skills</h2>
      <p class="section-desc">Capabilities available to agents</p>
      {#if skills.length === 0}
        <p class="empty-text">No skills configured</p>
      {:else}
        <div class="card-grid">
          {#each skills as skill (field(skill, 'Id'))}
            {@const skillId = field(skill, 'Id')}
            {@const skillFileId = field(skill, 'content_file_id') || field(skill, 'ContentFileId')}
            <div class="card">
              <div class="card-header">
                <span class="card-dot" style:background="var(--status-active)"></span>
                <span class="card-name">{field(skill, 'Name') || field(skill, 'name') || 'Unnamed'}</span>
              </div>
              <p class="card-desc">{field(skill, 'Description') || field(skill, 'description') || '--'}</p>
              <div class="card-meta-row">
                {#if field(skill, 'scope')}
                  <span class="meta-tag">scope: {field(skill, 'scope')}</span>
                {/if}
                {#if field(skill, 'agent_filter')}
                  <span class="meta-tag">filter: {field(skill, 'agent_filter')}</span>
                {/if}
              </div>
              {#if skillFileId}
                <button class="view-btn" onclick={() => toggleFileContent(skillId, skillFileId, 'skill')}>
                  {expandedSkill === skillId ? 'Hide Content' : 'View Content'}
                </button>
              {/if}
              {#if expandedSkill === skillId && fileContentCache[skillId]}
                <pre class="file-content" transition:slide={{ duration: 150 }}>{fileContentCache[skillId]}</pre>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <!-- WORK CYCLES -->
    <section class="section">
      <h2 class="section-title">Work Cycles</h2>
      <p class="section-desc">Governed implementation loops with gate enforcement</p>
      {#if workCycles.length === 0}
        <p class="empty-text">No work cycles</p>
      {:else}
        <div class="list">
          {#each workCycles as wc (field(wc, 'Id'))}
            <div class="list-row">
              <div class="list-header">
                <span class="list-dot" style:background={statusColor(field(wc, 'Status'))}></span>
                <span class="list-name">{field(wc, 'task_summary') || 'Untitled'}</span>
                <StatusBadge status={field(wc, 'Status')} />
              </div>
              <div class="wc-detail">
                <GatePipeline workcycle={wc as unknown as WorkCycle} />
                <div class="wc-meta">
                  <span>planner: <code>{field(wc, 'planner_id') || '--'}</code></span>
                  {#if field(wc, 'pr_url')}
                    <span>PR: <code>{field(wc, 'pr_url')}</code></span>
                  {/if}
                </div>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <!-- CEDAR POLICIES -->
    <section class="section">
      <h2 class="section-title">Cedar Policies</h2>
      <p class="section-desc">Active authorization policies governing agent actions</p>
      {#if policies.length === 0}
        <p class="empty-inline">No runtime Cedar policies loaded</p>
      {:else}
        <div class="list">
          {#each policies as policy (policy.policy_id)}
            <div class="list-row">
              <div class="list-header" onclick={() => expandedPolicy = expandedPolicy === policy.policy_id ? null : policy.policy_id}>
                <span class="list-dot" style:background={policy.enabled ? 'var(--status-success)' : 'var(--status-idle)'}></span>
                <code class="list-name">{policy.policy_id}</code>
                {#if policy.source}<span class="tool-tag">{policy.source}</span>{/if}
                {#if policy.created_by}<span class="list-meta">by {policy.created_by}</span>{/if}
                <span class="list-meta" style="margin-left:auto">{policy.enabled ? 'enabled' : 'disabled'}</span>
              </div>
              {#if expandedPolicy === policy.policy_id}
                <div class="list-detail" transition:slide={{ duration: 150 }}>
                  <pre class="detail-pre">{policy.cedar_text}</pre>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <!-- AUTHORIZATION DECISIONS -->
    <section class="section">
      <h2 class="section-title">Authorization Decisions</h2>
      <p class="section-desc">Cedar policy evaluations -- what was requested, who granted or denied</p>
      {#if decisions.length === 0}
        <p class="empty-inline">No authorization decisions yet</p>
      {:else}
        <div class="list">
          {#each decisions as decision (decision.id)}
            <div class="list-row">
              <div class="list-header">
                <span class="list-dot" style:background={statusColor(decision.status)}></span>
                <span class="list-meta">{decision.status}</span>
                <span class="list-name">{decision.action}</span>
                <span class="list-meta">on {decision.resource_type}:{shortId(decision.resource_id)}</span>
                <span class="list-meta" style="margin-left:auto">by <code>{shortId(decision.agent_id)}</code></span>
                {#if decision.decided_by}
                  <span class="list-meta">decided by {decision.decided_by}</span>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </section>

  {/if}
</div>

<style>
  .page { display: flex; flex-direction: column; gap: var(--space-4); }
  .subtitle { color: var(--text-secondary); font-size: var(--text-sm); }

  .section { display: flex; flex-direction: column; gap: var(--space-2); }
  .section-title { font-size: var(--text-lg); }
  .section-desc { font-size: var(--text-xs); color: var(--text-tertiary); margin-bottom: var(--space-1); }

  .empty { display: flex; align-items: center; justify-content: center; padding: var(--space-6) 0; }
  .empty-text { color: var(--text-tertiary); font-size: var(--text-sm); }
  .empty-inline { color: var(--text-tertiary); font-size: var(--text-xs); font-style: italic; }

  /* Cards grid (for souls, skills) */
  .card-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: var(--space-2); }
  .card {
    display: flex; flex-direction: column; gap: 6px;
    padding: var(--space-2); background: var(--surface-raised); border-radius: var(--radius-md);
  }
  .card-header { display: flex; align-items: center; gap: var(--space-1); }
  .card-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .card-name { font-size: var(--text-sm); font-weight: 600; color: var(--text-primary); }
  .card-id { font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-tertiary); }
  .card-desc { font-size: var(--text-xs); color: var(--text-secondary); line-height: 1.4; }
  .card-meta-row { display: flex; flex-wrap: wrap; gap: 6px; }
  .meta-tag {
    font-family: var(--font-mono); font-size: 0.625rem; color: var(--text-secondary);
    background: var(--surface-overlay); padding: 1px 6px; border-radius: var(--radius-sm);
  }
  .view-btn {
    font-size: var(--text-xs); color: var(--text-secondary); padding: 2px 0;
    text-align: left; width: fit-content; cursor: pointer;
    background: none; border: none;
  }
  .view-btn:hover { color: var(--text-primary); }
  .file-content {
    font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-secondary);
    background: var(--surface-overlay); padding: var(--space-2); border-radius: var(--radius-sm);
    white-space: pre-wrap; overflow-x: auto; max-height: 400px; overflow-y: auto;
  }

  /* List rows */
  .list { display: flex; flex-direction: column; }
  .list-row { border-bottom: 1px solid var(--border); }
  .list-header {
    display: flex; align-items: center; gap: var(--space-1);
    padding: var(--space-2) 0; cursor: pointer;
    transition: background var(--duration-fast) var(--ease);
  }
  .list-header:hover { background: var(--brand-subtle); }
  .list-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .list-name { font-size: var(--text-sm); font-weight: 500; color: var(--text-primary); }
  .list-meta { font-size: var(--text-xs); color: var(--text-tertiary); }
  .list-meta code { font-family: var(--font-mono); }

  /* Detail panels */
  .list-detail {
    padding: var(--space-2) var(--space-3); background: var(--surface-raised);
    border-radius: var(--radius-md); margin-bottom: var(--space-1);
  }
  .detail-grid { display: grid; grid-template-columns: 120px 1fr; gap: 6px var(--space-2); align-items: baseline; }
  .detail-label { font-size: 0.625rem; color: var(--text-tertiary); text-transform: uppercase; letter-spacing: 0.05em; }
  .detail-value { font-size: var(--text-sm); color: var(--text-primary); word-break: break-all; }
  .detail-pre {
    font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-secondary);
    background: var(--surface-overlay); padding: var(--space-2); border-radius: var(--radius-sm);
    white-space: pre-wrap; overflow-x: auto; max-height: 300px; overflow-y: auto; grid-column: 1 / -1;
  }

  /* Tool tags */
  .tool-tag {
    font-family: var(--font-mono); font-size: 0.625rem; color: var(--text-secondary);
    background: var(--surface-overlay); padding: 1px 6px; border-radius: var(--radius-sm);
  }

  /* WorkCycle inline */
  .wc-detail { padding: 0 0 var(--space-1) var(--space-2); }
  .wc-meta { display: flex; gap: var(--space-2); font-size: var(--text-xs); color: var(--text-tertiary); margin-top: 6px; }
  .wc-meta code { font-family: var(--font-mono); }

  @media (max-width: 800px) {
    .card-grid { grid-template-columns: 1fr; }
    .detail-grid { grid-template-columns: 1fr; }
  }
</style>
