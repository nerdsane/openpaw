<script lang="ts">
  import { base } from '$app/paths';
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import { decisions, policies, authzHistory, loadDecisions, loadPolicies, loadAuthzHistory } from '$lib/stores/permissions';
  import type { PendingDecision, PolicyEntry, SessionHistoryEntry } from '$lib/api';

  let loaded = $state(false);
  let expandedDecision = $state<string | null>(null);
  let expandedPolicy = $state<string | null>(null);
  let auditFilter = $state<'all' | 'denied'>('all');

  let pendingDecisions = $derived(
    ($decisions ?? []).filter(d => d.status === 'pending')
  );

  let filteredHistory = $derived(
    auditFilter === 'denied'
      ? ($authzHistory ?? []).slice().sort((a, b) => b.timestamp.localeCompare(a.timestamp)).filter(e => e.authz_denied)
      : ($authzHistory ?? []).slice().sort((a, b) => b.timestamp.localeCompare(a.timestamp))
  );

  function toggleDecision(id: string) {
    expandedDecision = expandedDecision === id ? null : id;
  }

  function togglePolicy(id: string) {
    expandedPolicy = expandedPolicy === id ? null : id;
  }

  function formatTime(ts: string): string {
    if (!ts) return '--:--:--';
    const d = new Date(ts);
    return d.toTimeString().slice(0, 8);
  }

  function formatTimestamp(ts: string): string {
    if (!ts) return '--';
    const d = new Date(ts);
    return d.toLocaleString();
  }

  function highlightCedar(text: string): string {
    if (!text) return '';
    return text
      .split('\n')
      .map(line => {
        if (line.trimStart().startsWith('//')) {
          return `<span class="cedar-comment">${escapeHtml(line)}</span>`;
        }
        let result = escapeHtml(line);
        result = result.replace(/\b(permit)\b/g, '<span class="cedar-permit">$1</span>');
        result = result.replace(/\b(forbid)\b/g, '<span class="cedar-forbid">$1</span>');
        return result;
      })
      .join('\n');
  }

  function escapeHtml(s: string): string {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  onMount(async () => {
    await Promise.all([loadDecisions(), loadPolicies(), loadAuthzHistory()]);
    loaded = true;
  });
</script>

<div class="page">
  <div class="page-label">PERMISSIONS</div>

  {#if !loaded}
    <div class="empty">LOADING...</div>
  {:else}

    <!-- SECTION A: PENDING DECISIONS -->
    <div class="section-block">
      <div class="section-title">PENDING DECISIONS</div>
      {#if pendingDecisions.length === 0}
        <div class="empty-msg">NO PENDING DECISIONS</div>
      {:else}
        <div class="decision-list">
          {#each pendingDecisions as dec (dec.id)}
            <div class="decision-card">
              <div class="decision-header">
                <div class="decision-row">
                  <span class="field-label">AGENT</span>
                  <span class="field-value bold">{dec.agent_id}</span>
                </div>
                <div class="decision-row">
                  <span class="field-label">ACTION</span>
                  <span class="field-value">{dec.action}</span>
                </div>
                <div class="decision-row">
                  <span class="field-label">RESOURCE</span>
                  <span class="field-value">{dec.resource_type}/{dec.resource_id}</span>
                </div>
                <div class="decision-row">
                  <span class="field-label">REASON</span>
                  <span class="field-value">{dec.denial_reason || '--'}</span>
                </div>
                <div class="decision-row">
                  <span class="field-label">CREATED</span>
                  <span class="field-value">{formatTimestamp(dec.created_at)}</span>
                </div>
              </div>
              {#if dec.generated_policy}
                <button class="raw-toggle" onclick={() => toggleDecision(dec.id)}>
                  {expandedDecision === dec.id ? '[-]' : '[+]'} POLICY
                </button>
                {#if expandedDecision === dec.id}
                  <pre class="cedar-text" transition:slide={{ duration: 150 }}>{dec.generated_policy}</pre>
                {/if}
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <!-- SECTION B: POLICY REGISTRY -->
    <div class="section-block">
      <div class="section-title">POLICY REGISTRY</div>
      {#if ($policies ?? []).length === 0}
        <div class="empty-msg">NO POLICIES</div>
      {:else}
        <div class="policy-list">
          {#each $policies as pol (pol.policy_id)}
            <div class="policy-row">
              <button class="policy-header" onclick={() => togglePolicy(pol.policy_id)}>
                <span class="status-dot" class:active={pol.enabled} class:idle={!pol.enabled}></span>
                <span class="policy-source tag">{pol.source ?? 'manual'}</span>
                <span class="policy-id">{pol.policy_id ?? '--'}</span>
                <span class="expand-icon">{expandedPolicy === pol.policy_id ? '[-]' : '[+]'}</span>
              </button>
              {#if expandedPolicy === pol.policy_id}
                <div class="policy-body" transition:slide={{ duration: 150 }}>
                  <pre class="cedar-text highlighted">{@html highlightCedar(pol.cedar_text)}</pre>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <!-- SECTION C: AUTHORIZATION AUDIT -->
    <div class="section-block">
      <div class="section-title">AUTHORIZATION AUDIT</div>
      <div class="filter-bar">
        <button class="filter-btn" class:filter-active={auditFilter === 'all'} onclick={() => auditFilter = 'all'}>ALL</button>
        <button class="filter-btn" class:filter-active={auditFilter === 'denied'} onclick={() => auditFilter = 'denied'}>DENIED ONLY</button>
      </div>
      {#if filteredHistory.length === 0}
        <div class="empty-msg">NO ENTRIES</div>
      {:else}
        <div class="audit-list">
          {#each filteredHistory as entry}
            <div class="audit-row" class:denied-row={entry.authz_denied}>
              <span class="audit-time">{formatTime(entry.timestamp)}</span>
              <span class="tag">{entry.entity_type}</span>
              <a href="{base}/entities/{entry.entity_type}/{entry.entity_id}" class="audit-id">{entry.entity_id ?? ''}</a>
              <span class="audit-action">{entry.action}</span>
              {#if entry.authz_denied}
                <span class="badge-denied">DENIED: {entry.denied_resource ?? 'unknown'}</span>
              {:else}
                <span class="badge-allowed">ALLOWED</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>

  {/if}
</div>

<style>
  .page {
    background: var(--bg);
    min-height: 100vh;
    padding: var(--sp-6);
    color: var(--text-1);
  }

  .page-label {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-2);
    margin-bottom: var(--sp-6);
  }

  .empty {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text-3);
    padding: var(--sp-8);
    text-align: center;
  }

  .empty-msg {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    padding: var(--sp-4);
  }

  .section-block {
    margin-bottom: var(--sp-8);
  }

  .section-title {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-3);
    margin-bottom: var(--sp-4);
    padding-bottom: var(--sp-1);
    border-bottom: 1px solid var(--border);
  }

  /* DECISIONS */
  .decision-list {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .decision-card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-left: 2px solid var(--status-warning);
    border-radius: var(--radius);
    padding: var(--sp-4);
  }

  .decision-header {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }

  .decision-row {
    display: flex;
    gap: var(--sp-2);
    align-items: baseline;
  }

  .field-label {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-3);
    min-width: 80px;
  }

  .field-value {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-1);
  }

  .field-value.bold {
    font-weight: 700;
    color: var(--text-1);
  }

  .raw-toggle {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    background: none;
    border: none;
    cursor: pointer;
    padding: var(--sp-1) 0;
    margin-top: var(--sp-2);
  }

  .raw-toggle:hover {
    color: var(--accent);
  }

  .cedar-text {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-2);
    background: var(--surface-raised);
    padding: var(--sp-2);
    border-radius: var(--radius);
    border: 1px solid var(--border);
    overflow-x: auto;
    white-space: pre-wrap;
    margin-top: var(--sp-1);
  }

  /* POLICIES */
  .policy-list {
    display: flex;
    flex-direction: column;
  }

  .policy-row {
    border-bottom: 1px solid var(--border);
  }

  .policy-header {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    width: 100%;
    padding: var(--sp-2);
    background: none;
    border: none;
    color: inherit;
    cursor: pointer;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-align: left;
  }

  .policy-header:hover {
    background: var(--surface-raised);
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .status-dot.active {
    background: var(--accent);
  }

  .status-dot.idle {
    background: var(--status-idle);
  }

  .tag {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-2);
    background: var(--accent-subtle);
    padding: 2px var(--sp-1);
    border-radius: var(--radius);
  }

  .policy-id {
    color: var(--text-1);
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .expand-icon {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    margin-left: auto;
  }

  .policy-body {
    padding: 0 var(--sp-2) var(--sp-2);
  }

  .highlighted :global(.cedar-permit) {
    color: var(--accent);
  }

  .highlighted :global(.cedar-forbid) {
    color: var(--status-error);
  }

  .highlighted :global(.cedar-comment) {
    color: var(--text-3);
  }

  /* AUDIT */
  .filter-bar {
    display: flex;
    gap: var(--sp-1);
    margin-bottom: var(--sp-4);
  }

  .filter-btn {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-2);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--sp-1) var(--sp-2);
    cursor: pointer;
  }

  .filter-btn:hover {
    border-color: var(--border-strong);
  }

  .filter-btn.filter-active {
    color: var(--accent);
    border-color: var(--accent);
  }

  .audit-list {
    display: flex;
    flex-direction: column;
  }

  .audit-row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-1) var(--sp-2);
    border-bottom: 1px solid var(--border);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .denied-row {
    background: var(--authz-denied-bg);
  }

  .audit-time {
    color: var(--text-2);
    min-width: 72px;
  }

  .audit-id {
    color: var(--accent);
    text-decoration: none;
    min-width: 96px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .audit-id:hover {
    text-decoration: underline;
  }

  .audit-action {
    color: var(--text-1);
    flex: 1;
  }

  .badge-allowed {
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .badge-denied {
    color: var(--status-error);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .policy-source {
    min-width: 60px;
    text-align: center;
  }
</style>
