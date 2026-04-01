<script lang="ts">
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import { fetchDecisions, fetchPolicies, queryEntities, queryWorkCyclesForHarness, fetchFileContent, type PendingDecision, type PolicyEntry } from '$lib/api';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import GatePipeline from '$lib/components/GatePipeline.svelte';
  import type { WorkCycle, Soul, Skill } from '$lib/types';

  let loaded = $state(false);

  // Data
  let harnesses = $state<Record<string, unknown>[]>([]);
  let souls = $state<Record<string, unknown>[]>([]);
  let skills = $state<Record<string, unknown>[]>([]);
  /** Map of harnessId -> work cycles loaded from the harness's work_cycle_type entity set */
  let workCyclesByHarness = $state<Record<string, Record<string, unknown>[]>>({});
  let decisions = $state<PendingDecision[]>([]);
  let policies = $state<PolicyEntry[]>([]);

  // Expandable
  let expandedHarness = $state<string | null>(null);
  let expandedPolicy = $state<string | null>(null);
  let expandedRoleSoul = $state<string | null>(null);
  let expandedRoleSkill = $state<string | null>(null);
  let conventionsExpandedMap = $state<Record<string, boolean>>({});

  // File content cache for souls and skills
  let fileContentCache = $state<Record<string, string>>({});

  async function loadFileContent(id: string, fileId: string): Promise<void> {
    if (fileId && !fileContentCache[id]) {
      const content = await fetchFileContent(fileId);
      fileContentCache = { ...fileContentCache, [id]: content };
    }
  }

  onMount(async () => {
    const [ha, so, sk, dec, pol] = await Promise.all([
      queryEntities('Harnesses').catch(() => queryEntities('ProjectHarnesses').catch(() => [])),
      queryEntities('Souls').catch(() => []),
      queryEntities('Skills').catch(() => []),
      fetchDecisions().then(r => r.decisions).catch(() => []),
      fetchPolicies().catch(() => []),
    ]);
    harnesses = ha;
    souls = so;
    skills = sk;
    decisions = dec;
    policies = pol;

    // Load work cycles per harness using work_cycle_type
    const wcMap: Record<string, Record<string, unknown>[]> = {};
    await Promise.all(harnesses.map(async (h) => {
      const hId = field(h, 'Id');
      try {
        wcMap[hId] = await queryWorkCyclesForHarness(h, 'SequenceNr desc');
      } catch {
        // Fallback: try generic WorkCycles filtered by harness_id
        try {
          wcMap[hId] = await queryEntities('WorkCycles', `harness_id eq '${hId}'`);
        } catch {
          wcMap[hId] = [];
        }
      }
    }));
    workCyclesByHarness = wcMap;

    loaded = true;
  });

  /** All work cycles across all harnesses (for team role matching etc) */
  let allWorkCycles = $derived(
    Object.values(workCyclesByHarness).flat()
  );

  function field(entity: Record<string, unknown>, key: string): string {
    const val = entity[key] ?? entity[key.charAt(0).toUpperCase() + key.slice(1)] ?? '';
    if (typeof val === 'object' && val !== null) {
      try { return JSON.stringify(val); } catch { return String(val); }
    }
    return String(val);
  }

  function safeDisplay(value: unknown): string {
    if (value === null || value === undefined) return '';
    if (typeof value === 'object') {
      try { return JSON.stringify(value); } catch { return String(value); }
    }
    return String(value);
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

  // Filter skills to only project-scoped ones (exclude platform bootstrap skills like "Project Lead Schema")
  let projectSkills = $derived(
    skills.filter((sk) => {
      const scope = field(sk, 'scope');
      // Include skills scoped to a project (non-empty, not a soul name like "Paw")
      // Exclude platform-level skills (scope = soul name or empty)
      return scope && !souls.some(s => field(s, 'Name') === scope);
    })
  );

  let teamRoles = $derived.by((): TeamRole[] => {
    const roles: TeamRole[] = [];
    const matchedSkillIds = new Set<string>();

    // First: find skills with agent_filter → these define roles
    // Group by agent_filter to discover roles
    const rolesByFilter = new Map<string, Record<string, unknown>[]>();
    for (const sk of projectSkills) {
      const filter = field(sk, 'agent_filter');
      if (filter) {
        if (!rolesByFilter.has(filter)) rolesByFilter.set(filter, []);
        rolesByFilter.get(filter)!.push(sk);
        matchedSkillIds.add(field(sk, 'Id'));
      }
    }

    // For each role, check if there's a matching Soul
    for (const [roleName, roleSkills] of rolesByFilter) {
      const matchingSoul = souls.find(s => {
        const name = field(s, 'Name') || field(s, 'name');
        return name === roleName;
      });
      roles.push({
        name: roleName,
        description: matchingSoul
          ? (field(matchingSoul, 'Description') || field(matchingSoul, 'description') || '')
          : (field(roleSkills[0], 'Description') || field(roleSkills[0], 'description') || ''),
        soul: matchingSoul ?? null,
        skills: roleSkills,
        soulFileId: matchingSoul ? (field(matchingSoul, 'ContentFileId') || field(matchingSoul, 'content_file_id')) : '',
      });
    }

    // Check for souls that have project skills but aren't matched by agent_filter
    // (e.g., Ren has skills scoped to deep-sci-fi but with empty agent_filter)
    for (const soul of souls) {
      const soulName = field(soul, 'Name') || field(soul, 'name');
      // Skip if already a role, or if it's a platform soul without project skills
      if (roles.some(r => r.name === soulName)) continue;
      // Check if this soul has any project-scoped skills assigned to it
      const soulSkills = projectSkills.filter(sk => {
        const filter = field(sk, 'agent_filter');
        return filter && (filter.includes(soulName) || filter === soulName);
      });
      // Also check if the soul itself appears to be project-relevant
      // (e.g., Ren is project-relevant, Paw is not)
      const hasProjectSkills = soulSkills.length > 0;
      const isProjectSoul = projectSkills.some(sk => field(sk, 'scope') && soulName !== 'Paw');
      // Only include if the soul has a bespoke description (not generic bootstrap)
      const desc = field(soul, 'Description') || field(soul, 'description') || '';
      const isCustomSoul = desc.length > 30; // Generic bootstrap souls have short descriptions
      if (hasProjectSkills || isCustomSoul) {
        for (const sk of soulSkills) matchedSkillIds.add(field(sk, 'Id'));
        if (!roles.some(r => r.name === soulName)) {
          roles.push({
            name: soulName,
            description: desc,
            soul,
            skills: soulSkills,
            soulFileId: field(soul, 'ContentFileId') || field(soul, 'content_file_id'),
          });
        }
      }
    }

    return roles;
  });

  let sharedSkills = $derived(
    projectSkills.filter((sk) => !field(sk, 'agent_filter'))
  );

  // ---------- Harness Flow Diagram (per-harness, generic) ----------

  const KNOWN_STATE_ORDER = ['Planning', 'Planned', 'InProgress', 'Testing', 'Reviewing', 'Complete', 'Completed', 'Failed', 'Cancelled'];

  function getActiveWorkCycle(harnessId: string): Record<string, unknown> | null {
    const wcs = workCyclesByHarness[harnessId] ?? [];
    const active = wcs.find(
      (wc) => !['Completed', 'Complete', 'Failed', 'Cancelled'].includes(field(wc, 'Status'))
    );
    return active ?? (wcs.length > 0 ? wcs[0] : null);
  }

  interface HarnessGate {
    key: string;
    label: string;
    passed: boolean;
  }

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

  interface FlowStep {
    type: 'state' | 'gates';
    label: string;
    active: boolean;
    gates?: HarnessGate[];
  }

  function buildHarnessFlow(harnessId: string): FlowStep[] {
    const activeWc = getActiveWorkCycle(harnessId);
    if (!activeWc) return [];
    const currentStatus = field(activeWc, 'Status');
    const gates = extractGates(activeWc);

    const midpoint = Math.ceil(gates.length / 2);
    const level1Gates = gates.slice(0, midpoint);
    const level2Gates = gates.slice(midpoint);

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
  }

  // Skills grouped by agent_filter for display
  let skillsByFilter = $derived.by((): Map<string, Record<string, unknown>[]> => {
    const map = new Map<string, Record<string, unknown>[]>();
    for (const sk of skills) {
      const filter = field(sk, 'agent_filter') || 'shared';
      if (!map.has(filter)) map.set(filter, []);
      map.get(filter)!.push(sk);
    }
    return map;
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

    {#if harnesses.length === 0}
      <p class="empty-text">No harnesses configured</p>
    {:else}
      {#each harnesses as harness (field(harness, 'Id'))}
        {@const harnessId = field(harness, 'Id')}
        {@const harnessWcs = workCyclesByHarness[harnessId] ?? []}
        {@const activeWc = getActiveWorkCycle(harnessId)}
        {@const flow = buildHarnessFlow(harnessId)}
        {@const conventionsText = field(harness, 'conventions')}
        {@const workCycleType = field(harness, 'work_cycle_type') || 'WorkCycles'}

        <section class="harness-section">
          <!-- HARNESS HEADER -->
          <div class="harness-header">
            <div class="harness-header__top">
              <h2 class="harness-header__name">{field(harness, 'project_name') || field(harness, 'repo_url').replace('https://github.com/', '') || 'Unnamed Project'}</h2>
              <StatusBadge status={field(harness, 'Status')} />
            </div>
            <div class="harness-header__meta">
              {#if field(harness, 'repo_url')}
                <code class="harness-header__url">{field(harness, 'repo_url')}</code>
              {/if}
              {#if field(harness, 'tech_stack')}
                <span class="harness-header__stack">{safeDisplay(harness.tech_stack)}</span>
              {/if}
              <span class="harness-header__type">Cycles: {workCycleType}</span>
            </div>
          </div>

          <!-- TEAM COMPOSITION -->
          {#if teamRoles.length > 0 || sharedSkills.length > 0}
            <div class="harness-block">
              <h3 class="harness-block__title">Team</h3>
              <p class="harness-block__desc">Roles derived from souls and their associated skills</p>
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
            </div>
          {/if}

          <!-- HARNESS FLOW DIAGRAM -->
          {#if activeWc && flow.length > 0}
            <div class="harness-block">
              <h3 class="harness-block__title">Harness Flow</h3>
              <p class="harness-block__desc">Current work cycle state progression with gate enforcement</p>

              <div class="harness-flow-wrapper">
                <div class="harness-flow">
                  {#each flow as step, i}
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
                    {#if i < flow.length - 1}
                      <span class="flow-arrow">&rarr;</span>
                    {/if}
                  {/each}
                </div>
              </div>

              <div class="harness-meta">
                <span class="harness-meta__item">
                  <strong>Cycle:</strong> {field(activeWc, 'task_summary') || shortId(field(activeWc, 'Id'))}
                </span>
                <StatusBadge status={field(activeWc, 'Status')} />
              </div>

              {#if conventionsText}
                <button class="expand-toggle" onclick={() => conventionsExpandedMap = { ...conventionsExpandedMap, [harnessId]: !conventionsExpandedMap[harnessId] }}>
                  {conventionsExpandedMap[harnessId] ? 'Hide' : 'Show'} harness conventions
                </button>
                {#if conventionsExpandedMap[harnessId]}
                  <pre class="conventions-pre" transition:slide={{ duration: 150 }}>{conventionsText}</pre>
                {/if}
              {/if}
            </div>
          {/if}

          <!-- ACTIVE WORK CYCLES -->
          <div class="harness-block">
            <h3 class="harness-block__title">Work Cycles</h3>
            <p class="harness-block__desc">Governed implementation loops with gate enforcement ({workCycleType})</p>
            {#if harnessWcs.length === 0}
              <p class="empty-text">No work cycles for this harness</p>
            {:else}
              <div class="list">
                {#each harnessWcs as wc (field(wc, 'Id'))}
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
          </div>

          <!-- SKILLS (grouped by agent_filter) -->
          {#if skills.length > 0}
            <div class="harness-block">
              <h3 class="harness-block__title">Skills</h3>
              <p class="harness-block__desc">Grouped by agent filter</p>
              {#each [...skillsByFilter.entries()] as [filterLabel, filterSkills]}
                <div class="skills-scope-group">
                  <span class="skills-scope-label">{filterLabel}</span>
                  <div class="shared-skills-grid">
                    {#each filterSkills as sk}
                      {@const skId = field(sk, 'Id')}
                      {@const skFileId = field(sk, 'content_file_id') || field(sk, 'ContentFileId')}
                      <div class="shared-skill-card">
                        <div class="shared-skill-card__header">
                          <span class="shared-skill-card__name">{field(sk, 'Name') || field(sk, 'name') || field(sk, 'Id')}</span>
                          {#if field(sk, 'scope')}
                            <span class="scope-badge">{safeDisplay(sk.scope)}</span>
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
              {/each}
            </div>
          {/if}

        </section>
      {/each}
    {/if}

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
                {#if policy.source}<span class="tool-tag">{safeDisplay(policy.source)}</span>{/if}
                {#if policy.created_by}<span class="list-meta">by {safeDisplay(policy.created_by)}</span>{/if}
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
                <span class="list-meta">{safeDisplay(decision.status)}</span>
                <span class="list-name">{safeDisplay(decision.action)}</span>
                <span class="list-meta">on {safeDisplay(decision.resource_type)}:{shortId(safeDisplay(decision.resource_id))}</span>
                <span class="list-meta" style="margin-left:auto">by <code>{shortId(safeDisplay(decision.agent_id))}</code></span>
                {#if decision.decided_by}
                  <span class="list-meta">decided by {safeDisplay(decision.decided_by)}</span>
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

  /* ---- Harness Section ---- */
  .harness-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-3);
    background: var(--surface-base);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
  }

  .harness-header__top {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .harness-header__name {
    font-size: var(--text-lg);
    font-weight: 600;
    color: var(--text-primary);
  }

  .harness-header__meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    margin-top: 4px;
  }

  .harness-header__url {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    word-break: break-all;
  }

  .harness-header__stack {
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }

  .harness-header__type {
    font-family: var(--font-mono);
    font-size: 0.625rem;
    color: var(--text-tertiary);
    background: var(--surface-overlay);
    padding: 1px 6px;
    border-radius: var(--radius-sm);
  }

  .harness-block {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding-top: var(--space-2);
    border-top: 1px solid var(--border);
  }

  .harness-block__title {
    font-size: var(--text-base);
    font-weight: 500;
    color: var(--text-secondary);
  }

  .harness-block__desc {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    margin-bottom: var(--space-1);
  }

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

  /* ---- Skills grouped ---- */
  .skills-scope-group {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    margin-top: var(--space-1);
  }

  .skills-scope-label {
    font-family: var(--font-mono);
    font-size: 0.625rem;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding-left: 2px;
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
    .harness-flow { flex-wrap: wrap; }
  }
</style>
