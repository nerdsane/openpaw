<script lang="ts">
  import { onMount } from 'svelte';
  import { fade, slide } from 'svelte/transition';
  import { fetchDecisions, fetchPolicies, queryEntities, type PendingDecision, type PolicyEntry } from '$lib/api';
  import StatusBadge from '$lib/components/StatusBadge.svelte';

  let decisions = $state<PendingDecision[]>([]);
  let policies = $state<PolicyEntry[]>([]);
  let toolHooks = $state<Record<string, unknown>[]>([]);
  let loaded = $state(false);

  // Tab state
  type Tab = 'decisions' | 'policies' | 'hooks';
  let activeTab = $state<Tab>('decisions');

  // Decision filter
  let statusFilter = $state<string | null>(null);
  const statusOptions = ['All', 'pending', 'approved', 'denied', 'expired'] as const;

  let filteredDecisions = $derived.by(() => {
    if (!statusFilter || statusFilter === 'All') return decisions;
    return decisions.filter((d) => d.status === statusFilter);
  });

  // Counts
  let pendingCount = $derived(decisions.filter((d) => d.status === 'pending').length);
  let approvedCount = $derived(decisions.filter((d) => d.status === 'approved').length);
  let deniedCount = $derived(decisions.filter((d) => d.status === 'denied').length);

  // Expanded rows
  let expandedDecision = $state<string | null>(null);
  let expandedPolicy = $state<string | null>(null);

  onMount(async () => {
    const [decRes, polRes, hookRes] = await Promise.all([
      fetchDecisions().catch(() => ({ decisions: [], total: 0, pending_count: 0, approved_count: 0, denied_count: 0 })),
      fetchPolicies().catch(() => []),
      queryEntities('ToolHooks').catch(() => []),
    ]);
    decisions = decRes.decisions;
    policies = polRes;
    toolHooks = hookRes;
    loaded = true;
  });

  function statusColor(status: string): string {
    switch (status) {
      case 'approved': return 'var(--status-success)';
      case 'denied': case 'expired': return 'var(--status-error)';
      case 'pending': return 'var(--status-warning)';
      default: return 'var(--status-idle)';
    }
  }

  function relativeTime(iso: string): string {
    const d = new Date(iso);
    if (isNaN(d.getTime())) return iso;
    const diff = Date.now() - d.getTime();
    const mins = Math.floor(diff / 60000);
    if (mins < 1) return 'just now';
    if (mins < 60) return `${mins}m ago`;
    const hrs = Math.floor(mins / 60);
    if (hrs < 24) return `${hrs}h ago`;
    return `${Math.floor(hrs / 24)}d ago`;
  }

  function shortId(id: string): string {
    return id?.slice(0, 12) ?? '';
  }
</script>

<div class="permissions">
  <header class="permissions-header">
    <h1>Permissions</h1>
    <p class="permissions-subtitle">Authorization decisions, Cedar policies, and tool governance</p>
  </header>

  <!-- Summary cards -->
  <div class="summary-row">
    <div class="summary-card">
      <span class="summary-value" style:color="var(--status-warning)">{pendingCount}</span>
      <span class="summary-label">Pending</span>
    </div>
    <div class="summary-card">
      <span class="summary-value" style:color="var(--status-success)">{approvedCount}</span>
      <span class="summary-label">Approved</span>
    </div>
    <div class="summary-card">
      <span class="summary-value" style:color="var(--status-error)">{deniedCount}</span>
      <span class="summary-label">Denied</span>
    </div>
    <div class="summary-card">
      <span class="summary-value">{policies.length}</span>
      <span class="summary-label">Policies</span>
    </div>
    <div class="summary-card">
      <span class="summary-value">{toolHooks.length}</span>
      <span class="summary-label">Tool Hooks</span>
    </div>
  </div>

  <!-- Tab bar -->
  <div class="tab-bar">
    <button class="tab" class:tab--active={activeTab === 'decisions'} onclick={() => activeTab = 'decisions'}>
      Decisions {decisions.length > 0 ? `(${decisions.length})` : ''}
    </button>
    <button class="tab" class:tab--active={activeTab === 'policies'} onclick={() => activeTab = 'policies'}>
      Policies {policies.length > 0 ? `(${policies.length})` : ''}
    </button>
    <button class="tab" class:tab--active={activeTab === 'hooks'} onclick={() => activeTab = 'hooks'}>
      Tool Hooks {toolHooks.length > 0 ? `(${toolHooks.length})` : ''}
    </button>
  </div>

  {#if !loaded}
    <div class="empty" transition:fade={{ duration: 200 }}>
      <p class="empty-text">Loading...</p>
    </div>
  {:else if activeTab === 'decisions'}
    <!-- Decision filter -->
    <div class="filter-bar">
      {#each statusOptions as opt}
        <button
          class="filter-btn"
          class:filter-btn--active={statusFilter === opt || (opt === 'All' && !statusFilter)}
          onclick={() => statusFilter = opt === 'All' ? null : opt}
        >
          {opt}
        </button>
      {/each}
    </div>

    {#if filteredDecisions.length === 0}
      <div class="empty" transition:fade={{ duration: 200 }}>
        <p class="empty-text">No authorization decisions recorded yet</p>
        <p class="empty-detail">Decisions appear when Cedar denies an agent action and creates an approval request</p>
      </div>
    {:else}
      <div class="decision-list">
        {#each filteredDecisions as decision (decision.id)}
          <div class="decision-row" transition:slide={{ duration: 150 }}>
            <div class="decision-header" onclick={() => expandedDecision = expandedDecision === decision.id ? null : decision.id}>
              <span class="decision-dot" style:background={statusColor(decision.status)}></span>
              <span class="decision-status">{decision.status}</span>
              <span class="decision-action">{decision.action}</span>
              <span class="decision-arrow">on</span>
              <span class="decision-resource">{decision.resource_type}:{shortId(decision.resource_id)}</span>
              <span class="decision-agent">by <code>{shortId(decision.agent_id)}</code></span>
              <span class="decision-time">{relativeTime(decision.created_at)}</span>
            </div>

            {#if expandedDecision === decision.id}
              <div class="decision-detail" transition:slide={{ duration: 150 }}>
                <div class="detail-grid">
                  <span class="detail-label">Decision ID</span>
                  <code class="detail-value">{decision.id}</code>

                  <span class="detail-label">Agent</span>
                  <code class="detail-value">{decision.agent_id}</code>

                  <span class="detail-label">Action</span>
                  <span class="detail-value">{decision.action}</span>

                  <span class="detail-label">Resource</span>
                  <span class="detail-value">{decision.resource_type} : {decision.resource_id}</span>

                  <span class="detail-label">Denial Reason</span>
                  <span class="detail-value detail-value--reason">{decision.denial_reason}</span>

                  {#if decision.module_name}
                    <span class="detail-label">Module</span>
                    <code class="detail-value">{decision.module_name}</code>
                  {/if}

                  <span class="detail-label">Created</span>
                  <span class="detail-value">{decision.created_at}</span>

                  {#if decision.decided_by}
                    <span class="detail-label">Decided By</span>
                    <span class="detail-value">{decision.decided_by}</span>
                  {/if}

                  {#if decision.decided_at}
                    <span class="detail-label">Decided At</span>
                    <span class="detail-value">{decision.decided_at}</span>
                  {/if}

                  {#if decision.generated_policy}
                    <span class="detail-label">Generated Policy</span>
                    <pre class="detail-cedar">{decision.generated_policy}</pre>
                  {/if}
                </div>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

  {:else if activeTab === 'policies'}
    {#if policies.length === 0}
      <div class="empty" transition:fade={{ duration: 200 }}>
        <p class="empty-text">No Cedar policies loaded</p>
        <p class="empty-detail">Policies are created from os-app bundles, manual authoring, or approved decisions</p>
      </div>
    {:else}
      <div class="policy-list">
        {#each policies as policy (policy.policy_id)}
          <div class="policy-row" transition:slide={{ duration: 150 }}>
            <div class="policy-header" onclick={() => expandedPolicy = expandedPolicy === policy.policy_id ? null : policy.policy_id}>
              <span class="policy-dot" style:background={policy.enabled ? 'var(--status-success)' : 'var(--status-idle)'}></span>
              <code class="policy-id">{policy.policy_id}</code>
              {#if policy.source}
                <span class="policy-source">{policy.source}</span>
              {/if}
              {#if policy.created_by}
                <span class="policy-by">by {policy.created_by}</span>
              {/if}
              <span class="policy-enabled">{policy.enabled ? 'active' : 'disabled'}</span>
            </div>

            {#if expandedPolicy === policy.policy_id}
              <div class="policy-detail" transition:slide={{ duration: 150 }}>
                {#if policy.created_at}
                  <div class="detail-row">
                    <span class="detail-label">Created</span>
                    <span class="detail-value">{policy.created_at}</span>
                  </div>
                {/if}
                <pre class="detail-cedar">{policy.cedar_text}</pre>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

  {:else if activeTab === 'hooks'}
    {#if toolHooks.length === 0}
      <div class="empty" transition:fade={{ duration: 200 }}>
        <p class="empty-text">No tool hooks configured</p>
        <p class="empty-detail">Tool hooks intercept tool calls before/after execution to block, log, or modify behavior</p>
      </div>
    {:else}
      <div class="hook-list">
        {#each toolHooks as hook, i (hook.Id ?? i)}
          <div class="hook-row">
            <span class="hook-dot" style:background={hook.hook_action === 'block' ? 'var(--status-error)' : hook.hook_action === 'log' ? 'var(--status-warning)' : 'var(--status-success)'}></span>
            <span class="hook-name">{hook.name ?? 'Unnamed'}</span>
            <code class="hook-pattern">{hook.tool_pattern ?? '*'}</code>
            <span class="hook-action">{hook.hook_action ?? 'log'}</span>
            <span class="hook-type">{hook.hook_type ?? 'before'}</span>
            {#if hook.soul_id}
              <span class="hook-soul">soul: {hook.soul_id}</span>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .permissions {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .permissions-subtitle {
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  /* Summary */
  .summary-row {
    display: flex;
    gap: var(--space-2);
  }

  .summary-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: var(--space-2) var(--space-3);
    background: var(--surface-raised);
    border-radius: var(--radius-md);
    min-width: 90px;
  }

  .summary-value {
    font-family: var(--font-mono);
    font-size: var(--text-xl);
    font-weight: 600;
    color: var(--text-primary);
  }

  .summary-label {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  /* Tabs */
  .tab-bar {
    display: flex;
    gap: 2px;
    border-bottom: 1px solid var(--border);
    padding-bottom: 0;
  }

  .tab {
    padding: var(--space-1) var(--space-2);
    font-size: var(--text-sm);
    color: var(--text-secondary);
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    transition: color var(--duration-fast) var(--ease),
                border-color var(--duration-fast) var(--ease);
  }

  .tab:hover {
    color: var(--text-primary);
  }

  .tab--active {
    color: var(--text-primary);
    border-bottom-color: var(--accent);
  }

  /* Filters */
  .filter-bar {
    display: flex;
    gap: 4px;
  }

  .filter-btn {
    padding: 4px var(--space-1);
    font-size: var(--text-xs);
    color: var(--text-secondary);
    border-radius: var(--radius-sm);
    transition: color var(--duration-fast) var(--ease),
                background var(--duration-fast) var(--ease);
  }

  .filter-btn:hover { color: var(--text-primary); background: var(--brand-subtle); }
  .filter-btn--active { color: var(--text-primary); background: var(--surface-raised); }

  /* Empty */
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-1);
    padding: var(--space-8) 0;
  }

  .empty-text { color: var(--text-tertiary); font-size: var(--text-sm); }
  .empty-detail { color: var(--text-tertiary); font-size: var(--text-xs); opacity: 0.7; }

  /* Decision rows */
  .decision-list, .policy-list, .hook-list {
    display: flex;
    flex-direction: column;
  }

  .decision-row, .policy-row {
    border-bottom: 1px solid var(--border);
  }

  .decision-header, .policy-header {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: var(--space-2) 0;
    cursor: pointer;
    transition: background var(--duration-fast) var(--ease);
  }

  .decision-header:hover, .policy-header:hover {
    background: var(--brand-subtle);
  }

  .decision-dot, .policy-dot, .hook-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .decision-status {
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.03em;
    color: var(--text-secondary);
    min-width: 64px;
  }

  .decision-action {
    font-size: var(--text-sm);
    font-weight: 500;
    color: var(--text-primary);
  }

  .decision-arrow {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }

  .decision-resource {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }

  .decision-agent {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    margin-left: auto;
  }

  .decision-agent code {
    font-family: var(--font-mono);
  }

  .decision-time {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    min-width: 60px;
    text-align: right;
  }

  /* Detail panel */
  .decision-detail, .policy-detail {
    padding: var(--space-2) var(--space-3);
    background: var(--surface-raised);
    border-radius: var(--radius-md);
    margin-bottom: var(--space-1);
  }

  .detail-grid {
    display: grid;
    grid-template-columns: 120px 1fr;
    gap: 6px var(--space-2);
    align-items: baseline;
  }

  .detail-row {
    display: flex;
    gap: var(--space-2);
    margin-bottom: 6px;
  }

  .detail-label {
    font-size: 0.625rem;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .detail-value {
    font-size: var(--text-sm);
    color: var(--text-primary);
    word-break: break-all;
  }

  .detail-value--reason {
    color: var(--status-error);
  }

  .detail-cedar {
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
    margin-top: var(--space-1);
  }

  /* Policy rows */
  .policy-id {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-primary);
  }

  .policy-source {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    background: var(--surface-overlay);
    padding: 1px 6px;
    border-radius: var(--radius-sm);
  }

  .policy-by {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }

  .policy-enabled {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    margin-left: auto;
  }

  /* Hook rows */
  .hook-row {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--border);
  }

  .hook-name {
    font-size: var(--text-sm);
    font-weight: 500;
    color: var(--text-primary);
  }

  .hook-pattern {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-secondary);
    background: var(--surface-overlay);
    padding: 1px 6px;
    border-radius: var(--radius-sm);
  }

  .hook-action {
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.03em;
    color: var(--text-secondary);
  }

  .hook-type {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }

  .hook-soul {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    margin-left: auto;
  }

  @media (max-width: 800px) {
    .summary-row { flex-wrap: wrap; }
    .decision-header { flex-wrap: wrap; }
    .detail-grid { grid-template-columns: 1fr; }
  }
</style>
