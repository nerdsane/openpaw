<script lang="ts">
  import { base } from '$app/paths';
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import { allAgents, loadAllAgents, fetchAgentSessions } from '$lib/stores/agents';
  import { policies, loadPolicies } from '$lib/stores/permissions';
  import { summarizePermissions } from '$lib/utils/cedar-parse';
  import { queryEntities, fetchSessionHistory } from '$lib/api';
  import type { Agent, Session, EntityEvent } from '$lib/types';
  import type { SessionHistoryEntry } from '$lib/api';

  let loaded = $state(false);
  let expandedAgent = $state<string | null>(null);
  let expandedSection = $state<Record<string, Set<string>>>({});
  let sessionsCache = $state<Record<string, Session[]>>({});
  let historyCache = $state<Record<string, SessionHistoryEntry[]>>({});
  let showRawPolicies = $state(false);

  function toggleAgent(id: string) {
    expandedAgent = expandedAgent === id ? null : id;
  }

  function toggleSection(agentId: string, section: string) {
    const current = expandedSection[agentId] ?? new Set();
    if (current.has(section)) {
      current.delete(section);
    } else {
      current.add(section);
    }
    expandedSection = { ...expandedSection, [agentId]: new Set(current) };

    if (section === 'sessions' && !sessionsCache[agentId]) {
      loadSessions(agentId);
    }
    if (section === 'history' && !historyCache[agentId]) {
      loadHistory(agentId);
    }
  }

  function isSectionOpen(agentId: string, section: string): boolean {
    return expandedSection[agentId]?.has(section) ?? false;
  }

  async function loadSessions(agentId: string) {
    try {
      const data = await queryEntities('Sessions', `agent_id eq '${agentId}'`, 'SequenceNr desc', 20);
      sessionsCache = { ...sessionsCache, [agentId]: data as unknown as Session[] };
    } catch {
      sessionsCache = { ...sessionsCache, [agentId]: [] };
    }
  }

  async function loadHistory(agentId: string) {
    try {
      const sessions = sessionsCache[agentId] ?? [];
      const allHistory: SessionHistoryEntry[] = [];
      if (sessions.length > 0) {
        const results = await Promise.all(
          sessions.slice(0, 10).map(s => fetchSessionHistory(s.Id, 'Session', 200).catch(() => []))
        );
        results.forEach(r => allHistory.push(...r));
      }
      // Also try fetching history for the agent entity itself
      const agentHistory = await fetchSessionHistory(agentId, 'Agent', 200).catch(() => []);
      allHistory.push(...agentHistory);
      allHistory.sort((a, b) => b.timestamp.localeCompare(a.timestamp));
      historyCache = { ...historyCache, [agentId]: allHistory };
    } catch {
      historyCache = { ...historyCache, [agentId]: [] };
    }
  }

  function parseJsonArray(val: string | undefined | null): string[] {
    if (!val) return [];
    try {
      const parsed = JSON.parse(val);
      return Array.isArray(parsed) ? parsed : [];
    } catch {
      return val.split(',').map(s => s.trim()).filter(Boolean);
    }
  }

  function relativeTime(ts: string): string {
    if (!ts) return '--';
    const diff = Date.now() - new Date(ts).getTime();
    const mins = Math.floor(diff / 60000);
    if (mins < 1) return 'just now';
    if (mins < 60) return `${mins}m ago`;
    const hrs = Math.floor(mins / 60);
    if (hrs < 24) return `${hrs}h ago`;
    const days = Math.floor(hrs / 24);
    return `${days}d ago`;
  }

  function formatTime(ts: string): string {
    if (!ts) return '--:--:--';
    const d = new Date(ts);
    return d.toTimeString().slice(0, 8);
  }

  function truncate(s: string | undefined | null, len: number): string {
    if (!s) return '--';
    return s.length > len ? s.slice(0, len) + '...' : s;
  }

  let permissionsSummary = $derived(
    summarizePermissions(($policies ?? []).map(p => p.cedar_text))
  );

  onMount(async () => {
    await Promise.all([loadAllAgents(), loadPolicies()]);
    loaded = true;
  });
</script>

<div class="page">
  <div class="page-label">AGENTS</div>

  <div class="header-row">
    <a href="{base}/agents/new" class="new-agent-btn">+ NEW AGENT</a>
  </div>

  {#if !loaded}
    <div class="empty">LOADING...</div>
  {:else if $allAgents.length === 0}
    <div class="empty">NO AGENTS FOUND</div>
  {:else}
    <div class="agent-list">
      {#each $allAgents as agent (agent.Id)}
        {@const isExpanded = expandedAgent === agent.Id}
        <div class="agent-row" class:expanded={isExpanded}>
          <button class="agent-header" onclick={() => toggleAgent(agent.Id)}>
            <span class="status-dot" class:active={agent.Status === 'Active'} class:idle={agent.Status !== 'Active'}></span>
            <span class="agent-name">{agent.name || agent.Id}</span>
            <span class="agent-role">{agent.role || '--'}</span>
            <span class="tag">{agent.model || '--'}</span>
            <span class="tag">{agent.provider || '--'}</span>
            <span class="expand-icon">{isExpanded ? '[-]' : '[+]'}</span>
          </button>

          {#if isExpanded}
            <div class="agent-detail" transition:slide={{ duration: 200 }}>

              <!-- IDENTITY -->
              <div class="section">
                <button class="section-label" onclick={() => toggleSection(agent.Id, 'identity')}>
                  IDENTITY {isSectionOpen(agent.Id, 'identity') ? '[-]' : '[+]'}
                </button>
                {#if isSectionOpen(agent.Id, 'identity')}
                  <div class="section-body" transition:slide={{ duration: 150 }}>
                    <div class="field-grid">
                      <div class="field-label">NAME</div>
                      <div class="field-value">{agent.name || '--'}</div>
                      <div class="field-label">ROLE</div>
                      <div class="field-value">{agent.role || '--'}</div>
                      <div class="field-label">MODEL</div>
                      <div class="field-value">{agent.model || '--'}</div>
                      <div class="field-label">PROVIDER</div>
                      <div class="field-value">{agent.provider || '--'}</div>
                      <div class="field-label">SOUL ID</div>
                      <div class="field-value">
                        {#if agent.soul_id}
                          <a href="{base}/entities/Souls/{agent.soul_id}" class="entity-link">{agent.soul_id}</a>
                        {:else}
                          --
                        {/if}
                      </div>
                      <div class="field-label">SKILLS</div>
                      <div class="field-value">
                        {#each parseJsonArray(agent.skill_ids) as skillId}
                          <span class="tag">{skillId}</span>
                        {:else}
                          --
                        {/each}
                      </div>
                      <div class="field-label">TOOLS</div>
                      <div class="field-value">
                        {#each parseJsonArray(agent.tools_enabled) as tool}
                          <span class="tag">{tool}</span>
                        {:else}
                          --
                        {/each}
                      </div>
                    </div>
                  </div>
                {/if}
              </div>

              <!-- PERMISSIONS -->
              <div class="section">
                <button class="section-label" onclick={() => toggleSection(agent.Id, 'permissions')}>
                  PERMISSIONS {isSectionOpen(agent.Id, 'permissions') ? '[-]' : '[+]'}
                </button>
                {#if isSectionOpen(agent.Id, 'permissions')}
                  <div class="section-body" transition:slide={{ duration: 150 }}>
                    {#if permissionsSummary.length === 0}
                      <div class="empty-sm">NO POLICIES</div>
                    {:else}
                      <table class="perm-table">
                        <thead>
                          <tr>
                            <th>ACTION</th>
                            <th>RESOURCE</th>
                            <th>ACCESS</th>
                            <th>CONDITION</th>
                          </tr>
                        </thead>
                        <tbody>
                          {#each permissionsSummary as perm}
                            <tr class:denied-row={perm.verdict === 'forbid'}>
                              <td>{perm.actionDisplay}</td>
                              <td>{perm.resource}</td>
                              <td>
                                {#if perm.verdict === 'permit'}
                                  <span class="badge-allowed">ALLOWED</span>
                                {:else}
                                  <span class="badge-denied">DENIED</span>
                                {/if}
                              </td>
                              <td>{perm.condition ?? '--'}</td>
                            </tr>
                          {/each}
                        </tbody>
                      </table>
                    {/if}
                    <button class="raw-toggle" onclick={() => showRawPolicies = !showRawPolicies}>
                      {showRawPolicies ? '[-]' : '[+]'} RAW POLICIES
                    </button>
                    {#if showRawPolicies}
                      <div class="raw-policies" transition:slide={{ duration: 150 }}>
                        {#each $policies as pol}
                          <pre class="cedar-text">{pol.cedar_text}</pre>
                        {/each}
                      </div>
                    {/if}
                  </div>
                {/if}
              </div>

              <!-- RECENT SESSIONS -->
              <div class="section">
                <button class="section-label" onclick={() => toggleSection(agent.Id, 'sessions')}>
                  RECENT SESSIONS {isSectionOpen(agent.Id, 'sessions') ? '[-]' : '[+]'}
                </button>
                {#if isSectionOpen(agent.Id, 'sessions')}
                  <div class="section-body" transition:slide={{ duration: 150 }}>
                    {#if !sessionsCache[agent.Id]}
                      <div class="empty-sm">LOADING...</div>
                    {:else if sessionsCache[agent.Id].length === 0}
                      <div class="empty-sm">NO SESSIONS</div>
                    {:else}
                      <div class="session-list">
                        {#each sessionsCache[agent.Id] as session}
                          <a href="{base}/sessions/{session.Id}" class="session-row">
                            <span class="status-dot" class:active={session.Status === 'Running' || session.Status === 'Active'} class:idle={session.Status !== 'Running' && session.Status !== 'Active'}></span>
                            <span class="session-id">{session.Id ?? ''}</span>
                            <span class="session-task">{truncate(session.user_message || session.result, 60)}</span>
                            <span class="session-meta">{session.turn_count ?? 0} turns</span>
                            <span class="session-meta">{(session.input_tokens ?? 0) + (session.output_tokens ?? 0)} tok</span>
                            <span class="session-meta">{relativeTime(session.last_heartbeat_at)}</span>
                          </a>
                        {/each}
                      </div>
                    {/if}
                  </div>
                {/if}
              </div>

              <!-- AUTHORIZATION HISTORY -->
              <div class="section">
                <button class="section-label" onclick={() => toggleSection(agent.Id, 'history')}>
                  AUTHORIZATION HISTORY {isSectionOpen(agent.Id, 'history') ? '[-]' : '[+]'}
                </button>
                {#if isSectionOpen(agent.Id, 'history')}
                  <div class="section-body" transition:slide={{ duration: 150 }}>
                    {#if !historyCache[agent.Id]}
                      <div class="empty-sm">LOADING...</div>
                    {:else if historyCache[agent.Id].length === 0}
                      <div class="empty-sm">NO HISTORY</div>
                    {:else}
                      <div class="history-list">
                        {#each historyCache[agent.Id] as entry}
                          <div class="history-row" class:denied-row={entry.authz_denied}>
                            <span class="history-time">{formatTime(entry.timestamp)}</span>
                            <span class="history-action">{entry.action}</span>
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

            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .header-row {
    display: flex;
    justify-content: flex-end;
    margin-bottom: var(--sp-4);
  }

  .new-agent-btn {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    letter-spacing: 0.06em;
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--radius);
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
    text-decoration: none;
  }

  .new-agent-btn:hover { opacity: 0.9; text-decoration: none; }

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
    color: var(--text-3);
    margin-bottom: var(--sp-6);
  }

  .empty {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text-3);
    padding: var(--sp-8);
    text-align: center;
  }

  .empty-sm {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text-3);
    padding: var(--sp-4);
  }

  .agent-list {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .agent-row {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }

  .agent-row.expanded {
    border-color: var(--border-strong);
  }

  .agent-header {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    width: 100%;
    padding: var(--sp-3);
    background: none;
    border: none;
    color: inherit;
    cursor: pointer;
    font-family: var(--font-mono);
    font-size: var(--text-base);
    text-align: left;
  }

  .agent-header:hover {
    background: var(--surface);
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

  .agent-name {
    font-weight: 600;
    color: var(--text-1);
  }

  .agent-role {
    color: var(--text-3);
    flex: 1;
  }

  .tag {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-2);
    background: var(--accent-subtle);
    padding: var(--sp-1) var(--sp-2);
    border-radius: var(--radius);
  }

  .expand-icon {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text-3);
    margin-left: auto;
  }

  .agent-detail {
    border-top: 1px solid var(--border);
    padding: var(--sp-4);
  }

  .section {
    margin-bottom: var(--sp-2);
  }

  .section-label {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-3);
    background: none;
    border: none;
    cursor: pointer;
    padding: var(--sp-1) 0;
    display: block;
    width: 100%;
    text-align: left;
  }

  .section-label:hover {
    color: var(--accent);
  }

  .section-body {
    padding: var(--sp-2) 0;
  }

  .field-grid {
    display: grid;
    grid-template-columns: 100px 1fr;
    gap: var(--sp-1) var(--sp-4);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }

  .field-label {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-3);
  }

  .field-value {
    color: var(--text-1);
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-1);
    align-items: center;
  }

  .entity-link {
    color: var(--accent);
    text-decoration: none;
  }

  .entity-link:hover {
    text-decoration: underline;
  }

  .perm-table {
    width: 100%;
    border-collapse: collapse;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }

  .perm-table th {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-3);
    text-align: left;
    padding: var(--sp-1) var(--sp-2);
    border-bottom: 1px solid var(--border);
  }

  .perm-table td {
    padding: var(--sp-1) var(--sp-2);
    border-bottom: 1px solid var(--border);
    color: var(--text-1);
  }

  .denied-row {
    background: var(--authz-denied-bg);
  }

  .badge-allowed {
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .badge-denied {
    color: var(--status-error);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .raw-toggle {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
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

  .raw-policies {
    margin-top: var(--sp-2);
  }

  .cedar-text {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text-2);
    background: var(--surface);
    padding: var(--sp-2);
    border-radius: var(--radius);
    border: 1px solid var(--border);
    overflow-x: auto;
    white-space: pre-wrap;
    margin-bottom: var(--sp-1);
  }

  .session-list {
    display: flex;
    flex-direction: column;
  }

  .session-row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-1) var(--sp-2);
    border-bottom: 1px solid var(--border);
    text-decoration: none;
    color: inherit;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }

  .session-row:hover {
    background: var(--surface);
  }

  .session-id {
    color: var(--text-1);
    font-family: var(--font-mono);
    min-width: 72px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-task {
    flex: 1;
    color: var(--text-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-meta {
    color: var(--text-3);
    white-space: nowrap;
  }

  .history-list {
    display: flex;
    flex-direction: column;
  }

  .history-row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-1) var(--sp-2);
    border-bottom: 1px solid var(--border);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }

  .history-time {
    color: var(--text-3);
    min-width: 72px;
  }

  .history-action {
    color: var(--text-1);
    flex: 1;
  }

  @media (max-width: 640px) {
    .field-grid {
      grid-template-columns: 1fr;
      gap: var(--sp-1);
    }
    .session-row, .history-row {
      flex-wrap: wrap;
      gap: var(--sp-1);
    }
    .session-id, .history-time {
      min-width: 0;
      width: auto;
    }
    .session-task {
      width: 100%;
      order: -1;
    }
  }
</style>
