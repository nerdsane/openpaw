<script lang="ts">
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import { fetchDecisions, fetchPolicies, queryEntities, fetchFileContent, type PendingDecision, type PolicyEntry } from '$lib/api';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import GatePipeline from '$lib/components/GatePipeline.svelte';
  import type { WorkCycle, Soul, Skill } from '$lib/types';

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
  let expandedRoleSoul = $state<string | null>(null);
  let expandedRoleSkill = $state<string | null>(null);
  let harnessConventionsExpanded = $state(false);

  // File content cache for souls and skills
  let fileContentCache = $state<Record<string, string>>({});

  async function loadFileContent(id: string, fileId: string): Promise<void> {
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
      Promise.all([
        queryEntities('WorkCycles').catch(() => []),
        queryEntities('DsfWorkCycles').catch(() => []),
      ]).then(([a, b]) => [...a, ...b]),
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

  // ---------- Team Roles (derived, generic) ----------

  interface TeamRole {
    name: string;
    description: string;
    soul: Record<string, unknown> | null;
    skills: Record<string, unknown>[];
    soulFileId: string;
  }

  let teamRoles = $derived.by((): TeamRole[] => {
    const roles: TeamRole[] = [];
    const matchedSkillIds = new Set<string>();

    // For each Soul: create a role entry and attach matching skills
    for (const soul of souls) {
      const soulName = field(soul, 'Name') || field(soul, 'name') || field(soul, 'Id');
      const soulId = field(soul, 'Id');
      const matchingSkills = skills.filter((sk) => {
        const filter = field(sk, 'agent_filter');
        if (!filter) return false;
        // Match if filter contains soul name or soul id
        return filter.includes(soulName) || filter.includes(soulId);
      });
      for (const sk of matchingSkills) {
        matchedSkillIds.add(field(sk, 'Id'));
      }
      roles.push({
        name: soulName,
        description: field(soul, 'Description') || field(soul, 'description') || '',
        soul,
        skills: matchingSkills,
        soulFileId: field(soul, 'ContentFileId') || field(soul, 'content_file_id'),
      });
    }

    // Skills with an agent_filter that didn't match any Soul
    const unmatchedFilteredSkills = skills.filter((sk) => {
      const filter = field(sk, 'agent_filter');
      return filter && !matchedSkillIds.has(field(sk, 'Id'));
    });
    for (const sk of unmatchedFilteredSkills) {
      matchedSkillIds.add(field(sk, 'Id'));
      roles.push({
        name: field(sk, 'Name') || field(sk, 'name') || field(sk, 'Id'),
        description: field(sk, 'Description') || field(sk, 'description') || '',
        soul: null,
        skills: [sk],
        soulFileId: '',
      });
    }

    return roles;
  });

  // Global skills (empty agent_filter)
  let sharedSkills = $derived(
    skills.filter((sk) => !field(sk, 'agent_filter'))
  );

  // ---------- Harness Flow Diagram (derived, generic) ----------

  /** All known WorkCycle states in order. Derived from actual status values observed. */
  const KNOWN_STATE_ORDER = ['Planning', 'Planned', 'InProgress', 'Testing', 'Reviewing', 'Complete', 'Completed', 'Failed', 'Cancelled'];

  let activeWorkCycle = $derived.by((): Record<string, unknown> | null => {
    // Most recent non-terminal work cycle
    const active = workCycles.find(
      (wc) => !['Completed', 'Complete', 'Failed', 'Cancelled'].includes(field(wc, 'Status'))
    );
    return active ?? (workCycles.length > 0 ? workCycles[0] : null);
  });

  interface HarnessGate {
    key: string;
    label: string;
    passed: boolean;
  }

  /** Extract gate fields generically from any boolean fields ending in _ok, _passed, or starting with has_ */
  function extractGates(wc: Record<string, unknown>): HarnessGate[] {
    const result: HarnessGate[] = [];
    for (const [key, val] of Object.entries(wc)) {
      if (key.startsWith('_') || key === 'Id' || key === 'Status') continue;
      if (typeof val === 'boolean' && (key.endsWith('_ok') || key.endsWith('_passed') || key.startsWith('has_'))) {
        const label = key
          .replace(/_ok$/, '')
          .replace(/_passed$/, '')
          .replace(/^has_/, '')
          .replace(/_/g, ' ')
          .replace(/\b\w/g, (c) => c.toUpperCase());
        result.push({ key, label, passed: !!val });
      }
    }
    return result;
  }

  let harnessGates = $derived(activeWorkCycle ? extractGates(activeWorkCycle) : []);

  /** Build the state flow steps. Split gates into level 1 (before Testing) and level 2 (before Reviewing). */
  interface FlowStep {
    type: 'state' | 'gates';
    label: string;
    active: boolean;
    gates?: HarnessGate[];
  }

  let harnessFlow = $derived.by((): FlowStep[] => {
    if (!activeWorkCycle) return [];
    const currentStatus = field(activeWorkCycle, 'Status');
    const statusIdx = KNOWN_STATE_ORDER.indexOf(currentStatus);

    // Split gates into two levels based on position
    const midpoint = Math.ceil(harnessGates.length / 2);
    const level1Gates = harnessGates.slice(0, midpoint);
    const level2Gates = harnessGates.slice(midpoint);

    const steps: FlowStep[] = [];
    const flowStates = ['Planning', 'Planned', 'InProgress'];

    for (const s of flowStates) {
      steps.push({ type: 'state', label: s, active: currentStatus === s });
    }

    if (level1Gates.length > 0) {
      steps.push({ type: 'gates', label: 'Level 1 Gates', active: false, gates: level1Gates });
    }

    steps.push({ type: 'state', label: 'Testing', active: currentStatus === 'Testing' });

    if (level2Gates.length > 0) {
      steps.push({ type: 'gates', label: 'Level 2 Gates', active: false, gates: level2Gates });
    }

    steps.push({ type: 'state', label: 'Reviewing', active: currentStatus === 'Reviewing' });
    steps.push({ type: 'state', label: 'Complete', active: currentStatus === 'Complete' || currentStatus === 'Completed' });

    return steps;
  });

  // Conventions text from the harness
  let conventionsText = $derived.by((): string => {
    if (harnesses.length === 0) return '';
    return field(harnesses[0], 'conventions') || '';
  });
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

    <!-- HARNESS FLOW DIAGRAM -->
    {#if activeWorkCycle}
      <section class="section">
        <h2 class="section-title">Harness Flow</h2>
        <p class="section-desc">Current work cycle state progression with gate enforcement</p>

        <div class="harness-flow-wrapper">
          <div class="harness-flow">
            {#each harnessFlow as step, i}
              {#if step.type === 'state'}
                <div class="flow-state" class:flow-state--active={step.active}>
                  <span class="flow-state__label">{step.label}</span>
                </div>
              {:else if step.type === 'gates' && step.gates}
                <div class="flow-gates">
                  <span class="flow-gates__label">{step.label}</span>
                  <div class="flow-gates__badges">
                    {#each step.gates as gate}
                      <span class="gate-badge" class:gate-badge--passed={gate.passed} class:gate-badge--pending={!gate.passed}>
                        {#if gate.passed}
                          <span class="gate-icon gate-icon--check">&#10003;</span>
                        {:else}
                          <span class="gate-icon gate-icon--pending">&#9679;</span>
                        {/if}
                        {gate.label}
                      </span>
                    {/each}
                  </div>
                </div>
              {/if}
              {#if i < harnessFlow.length - 1}
                <span class="flow-arrow">&rarr;</span>
              {/if}
            {/each}
          </div>
        </div>

        <div class="harness-meta">
          <span class="harness-meta__item">
            <strong>Cycle:</strong> {field(activeWorkCycle, 'task_summary') || shortId(field(activeWorkCycle, 'Id'))}
          </span>
          <StatusBadge status={field(activeWorkCycle, 'Status')} />
        </div>

        {#if conventionsText}
          <button class="expand-toggle" onclick={() => harnessConventionsExpanded = !harnessConventionsExpanded}>
            {harnessConventionsExpanded ? 'Hide' : 'Show'} harness conventions
          </button>
          {#if harnessConventionsExpanded}
            <pre class="conventions-pre" transition:slide={{ duration: 150 }}>{conventionsText}</pre>
          {/if}
        {/if}
      </section>
    {/if}

    <!-- TEAM COMPOSITION -->
    <section class="section">
      <h2 class="section-title">Team Composition</h2>
      <p class="section-desc">Roles derived from souls and their associated skills</p>
      {#if teamRoles.length === 0 && sharedSkills.length === 0}
        <p class="empty-text">No team roles configured</p>
      {:else}
        <div class="role-grid">
          {#each teamRoles as role}
            <div class="role-card">
              <div class="role-card__header">
                <span class="role-card__name">{role.name}</span>
                {#if role.soul}
                  <span class="role-badge role-badge--soul">Soul</span>
                {:else}
                  <span class="role-badge role-badge--skill">Skill-only</span>
                {/if}
              </div>
              {#if role.description}
                <p class="role-card__desc">{role.description}</p>
              {/if}

              {#if role.skills.length > 0}
                <div class="role-card__skills">
                  {#each role.skills as sk}
                    <span class="skill-tag">{field(sk, 'Name') || field(sk, 'name') || field(sk, 'Id')}</span>
                  {/each}
                </div>
              {/if}

              <div class="role-card__actions">
                {#if role.soulFileId}
                  <button class="view-btn" onclick={async () => {
                    const soulId = field(role.soul!, 'Id');
                    if (expandedRoleSoul === soulId) { expandedRoleSoul = null; return; }
                    expandedRoleSoul = soulId;
                    await loadFileContent(soulId, role.soulFileId);
                  }}>
                    {expandedRoleSoul === (role.soul ? field(role.soul, 'Id') : '') ? 'Hide soul' : 'View soul'}
                  </button>
                {/if}
                {#each role.skills as sk}
                  {@const skId = field(sk, 'Id')}
                  {@const skFileId = field(sk, 'content_file_id') || field(sk, 'ContentFileId')}
                  {#if skFileId}
                    <button class="view-btn" onclick={async () => {
                      if (expandedRoleSkill === skId) { expandedRoleSkill = null; return; }
                      expandedRoleSkill = skId;
                      await loadFileContent(skId, skFileId);
                    }}>
                      {expandedRoleSkill === skId ? `Hide ${field(sk, 'name') || field(sk, 'Name') || 'skill'}` : `View ${field(sk, 'name') || field(sk, 'Name') || 'skill'}`}
                    </button>
                  {/if}
                {/each}
              </div>

              {#if role.soul && expandedRoleSoul === field(role.soul, 'Id') && fileContentCache[field(role.soul, 'Id')]}
                <pre class="file-content" transition:slide={{ duration: 150 }}>{fileContentCache[field(role.soul, 'Id')]}</pre>
              {/if}
              {#each role.skills as sk}
                {@const skId = field(sk, 'Id')}
                {#if expandedRoleSkill === skId && fileContentCache[skId]}
                  <pre class="file-content" transition:slide={{ duration: 150 }}>{fileContentCache[skId]}</pre>
                {/if}
              {/each}
            </div>
          {/each}
        </div>

        {#if sharedSkills.length > 0}
          <div class="shared-skills-section">
            <h3 class="shared-skills-title">Shared Skills</h3>
            <p class="section-desc">Available to all agents (no agent_filter)</p>
            <div class="shared-skills-grid">
              {#each sharedSkills as sk}
                {@const skId = field(sk, 'Id')}
                {@const skFileId = field(sk, 'content_file_id') || field(sk, 'ContentFileId')}
                <div class="shared-skill-card">
                  <div class="shared-skill-card__header">
                    <span class="shared-skill-card__name">{field(sk, 'Name') || field(sk, 'name') || field(sk, 'Id')}</span>
                    {#if field(sk, 'scope')}
                      <span class="scope-badge">{field(sk, 'scope')}</span>
                    {/if}
                  </div>
                  {#if field(sk, 'Description') || field(sk, 'description')}
                    <p class="shared-skill-card__desc">{field(sk, 'Description') || field(sk, 'description')}</p>
                  {/if}
                  {#if skFileId}
                    <button class="view-btn" onclick={async () => {
                      if (expandedRoleSkill === skId) { expandedRoleSkill = null; return; }
                      expandedRoleSkill = skId;
                      await loadFileContent(skId, skFileId);
                    }}>
                      {expandedRoleSkill === skId ? 'Hide content' : 'View content'}
                    </button>
                  {/if}
                  {#if expandedRoleSkill === skId && fileContentCache[skId]}
                    <pre class="file-content" transition:slide={{ duration: 150 }}>{fileContentCache[skId]}</pre>
                  {/if}
                </div>
              {/each}
            </div>
          </div>
        {/if}
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

  /* ---- Role Grid (Team Composition) ---- */
  .role-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: var(--space-2);
  }

  .role-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: var(--space-2);
    background: var(--surface-raised);
    border-radius: var(--radius-md);
  }

  .role-card__header {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  .role-card__name {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-primary);
  }

  .role-badge {
    font-size: 0.625rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    padding: 1px 6px;
    border-radius: var(--radius-sm);
  }

  .role-badge--soul {
    color: var(--status-active);
    background: var(--brand-subtle);
  }

  .role-badge--skill {
    color: var(--text-tertiary);
    background: var(--surface-overlay);
  }

  .role-card__desc {
    font-size: var(--text-xs);
    color: var(--text-secondary);
    line-height: 1.4;
  }

  .role-card__skills {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .skill-tag {
    font-family: var(--font-mono);
    font-size: 0.625rem;
    color: var(--text-secondary);
    background: var(--surface-overlay);
    padding: 2px 8px;
    border-radius: var(--radius-sm);
  }

  .role-card__actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  /* ---- Shared Skills ---- */
  .shared-skills-section {
    margin-top: var(--space-2);
    padding-top: var(--space-2);
    border-top: 1px solid var(--border);
  }

  .shared-skills-title {
    font-size: var(--text-base);
    color: var(--text-secondary);
    margin-bottom: 4px;
  }

  .shared-skills-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: var(--space-2);
  }

  .shared-skill-card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: var(--space-2);
    background: var(--surface-raised);
    border-radius: var(--radius-sm);
  }

  .shared-skill-card__header {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  .shared-skill-card__name {
    font-size: var(--text-sm);
    font-weight: 500;
    color: var(--text-primary);
  }

  .scope-badge {
    font-family: var(--font-mono);
    font-size: 0.625rem;
    color: var(--text-secondary);
    background: var(--surface-overlay);
    padding: 1px 6px;
    border-radius: var(--radius-sm);
  }

  .shared-skill-card__desc {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }

  /* ---- Harness Flow Diagram ---- */
  .harness-flow-wrapper {
    overflow-x: auto;
    padding: var(--space-2) 0;
  }

  .harness-flow {
    display: flex;
    align-items: center;
    gap: 0;
    min-width: max-content;
  }

  .flow-state {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 8px 16px;
    background: var(--surface-raised);
    border: 1.5px solid var(--border);
    border-radius: var(--radius-md);
    min-width: 90px;
    text-align: center;
  }

  .flow-state--active {
    background: var(--surface-overlay);
    border-color: var(--status-active);
    box-shadow: 0 0 0 2px var(--brand-subtle);
  }

  .flow-state__label {
    font-size: var(--text-xs);
    font-weight: 500;
    color: var(--text-primary);
  }

  .flow-arrow {
    color: var(--text-tertiary);
    font-size: var(--text-sm);
    padding: 0 6px;
    flex-shrink: 0;
  }

  .flow-gates {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 6px 10px;
    background: var(--surface-overlay);
    border-radius: var(--radius-md);
    border: 1px dashed var(--border);
  }

  .flow-gates__label {
    font-size: 0.625rem;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .flow-gates__badges {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }

  .gate-badge {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 0.625rem;
    padding: 2px 6px;
    border-radius: var(--radius-sm);
    white-space: nowrap;
  }

  .gate-badge--passed {
    color: #4ade80;
    background: rgba(74, 222, 128, 0.1);
  }

  .gate-badge--pending {
    color: #6b7280;
    background: rgba(107, 114, 128, 0.1);
  }

  .gate-icon {
    font-size: 0.5rem;
    line-height: 1;
  }

  .gate-icon--check { color: #4ade80; }
  .gate-icon--pending { color: #6b7280; }

  .harness-meta {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }

  .harness-meta__item strong {
    color: var(--text-tertiary);
    font-weight: 500;
    text-transform: uppercase;
    font-size: 0.625rem;
    letter-spacing: 0.03em;
  }

  .expand-toggle {
    font-size: var(--text-xs);
    color: var(--text-secondary);
    padding: 2px 0;
    text-align: left;
    width: fit-content;
    cursor: pointer;
    background: none;
    border: none;
  }

  .expand-toggle:hover { color: var(--text-primary); }

  .conventions-pre {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-secondary);
    background: var(--surface-overlay);
    padding: var(--space-2);
    border-radius: var(--radius-sm);
    white-space: pre-wrap;
    overflow-x: auto;
    max-height: 400px;
    overflow-y: auto;
  }

  /* ---- Reused card / list styles ---- */
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
    .role-grid { grid-template-columns: 1fr; }
    .shared-skills-grid { grid-template-columns: 1fr; }
    .detail-grid { grid-template-columns: 1fr; }
    .harness-flow { flex-wrap: wrap; }
  }
</style>
