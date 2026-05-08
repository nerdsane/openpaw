<script lang="ts">
  import { base } from '$app/paths';
  import { onMount } from 'svelte';
  import { queryEntities } from '$lib/api';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import { entityId, readField, textValue, truncateMiddle } from '$lib/entity-format';
  import {
    TERMINAL_STATUSES,
    field,
    firstActivityMs,
    formatCount,
    formatDateTimeMs,
    jsonArrayCount,
    lastActivityMs,
    sessionTitle,
    shortText
  } from '$lib/dashboard-format';

  type SessionRow = Record<string, unknown>;
  type SessionFilter = 'all' | 'active' | 'failed' | 'completed';
  type SortMode = 'newest' | 'oldest';

  let sessions = $state<SessionRow[]>([]);
  let loaded = $state(false);
  let loadError = $state('');
  let filter = $state<SessionFilter>('all');
  let sortMode = $state<SortMode>('newest');

  function status(row: SessionRow): string {
    return textValue(readField(row, 'Status'));
  }

  function isTerminal(row: SessionRow): boolean {
    return TERMINAL_STATUSES.has(status(row));
  }

  function sessionKind(row: SessionRow): string {
    const agent = textValue(field(row, ['agent_id', 'AgentId']));
    const soul = textValue(field(row, ['soul_id', 'SoulId']));
    const provider = textValue(field(row, ['provider', 'Provider']));
    const model = textValue(field(row, ['model', 'Model']));
    const owner = agent !== '-' ? agent : soul;
    return [owner, provider !== '-' ? provider : '', model !== '-' ? model : '']
      .filter(Boolean)
      .join(' / ') || '-';
  }

  function tokenSummary(row: SessionRow): string {
    const input = Number(field(row, ['input_tokens', 'InputTokens']) ?? 0);
    const output = Number(field(row, ['output_tokens', 'OutputTokens']) ?? 0);
    const cost = textValue(field(row, ['cost_cents', 'CostCents']));
    const pieces = [`in ${formatCount(input)}`, `out ${formatCount(output)}`];
    if (cost !== '-') pieces.push(`${cost}c`);
    return pieces.join(' / ');
  }

  function relationSummary(row: SessionRow): string {
    const parent = textValue(field(row, ['parent_session_id', 'ParentSessionId']));
    const children = jsonArrayCount(field(row, ['child_session_ids', 'ChildSessionIds']));
    const depth = textValue(field(row, ['session_depth', 'SessionDepth']));
    const parts = [];
    if (parent !== '-') parts.push(`parent ${truncateMiddle(parent, 8, 5)}`);
    if (children > 0) parts.push(`${children} child${children === 1 ? '' : 'ren'}`);
    if (depth !== '-') parts.push(`depth ${depth}`);
    return parts.join(' / ') || '-';
  }

  function matchesFilter(row: SessionRow): boolean {
    if (filter === 'all') return true;
    if (filter === 'active') return !isTerminal(row);
    if (filter === 'failed') return status(row) === 'Failed';
    if (filter === 'completed') return ['Completed', 'Complete'].includes(status(row));
    return true;
  }

  const visibleSessions = $derived.by(() => {
    return sessions
      .filter(matchesFilter)
      .sort((left, right) => {
        const delta = lastActivityMs(right) - lastActivityMs(left);
        const fallback = entityId(right).localeCompare(entityId(left));
        const newest = delta !== 0 ? delta : fallback;
        return sortMode === 'newest' ? newest : -newest;
      });
  });

  const summary = $derived.by(() => {
    const active = sessions.filter((row) => !isTerminal(row)).length;
    const failed = sessions.filter((row) => status(row) === 'Failed').length;
    const completed = sessions.filter((row) => ['Completed', 'Complete'].includes(status(row))).length;
    const newest = Math.max(...sessions.map(lastActivityMs), 0);
    const oldest = Math.min(...sessions.map(firstActivityMs).filter((value) => value > 0), newest || 0);
    return { active, failed, completed, newest, oldest };
  });

  async function loadSessions() {
    loaded = false;
    loadError = '';
    try {
      const data = await queryEntities('Sessions', undefined, 'Id desc', 200);
      sessions = data;
    } catch (err) {
      loadError = err instanceof Error ? err.message : 'Could not load Sessions';
      sessions = [];
    }
    loaded = true;
  }

  onMount(() => {
    void loadSessions();
  });
</script>

<div class="page">
  <header class="page-head">
    <div>
      <p class="eyebrow">Temporal session ledger</p>
      <h1>Sessions</h1>
      <p>All loaded Sessions, ordered by actual activity timestamps with entity-id fallback.</p>
    </div>
    <button type="button" onclick={loadSessions} disabled={!loaded}>Refresh</button>
  </header>

  {#if loadError}
    <div class="notice notice-error">{loadError}</div>
  {/if}

  <section class="summary" aria-label="Session summary">
    <div>
      <span>Total loaded</span>
      <strong>{sessions.length}</strong>
    </div>
    <div>
      <span>Active</span>
      <strong>{summary.active}</strong>
    </div>
    <div>
      <span>Failed</span>
      <strong>{summary.failed}</strong>
    </div>
    <div>
      <span>Completed</span>
      <strong>{summary.completed}</strong>
    </div>
    <div>
      <span>Oldest</span>
      <strong>{formatDateTimeMs(summary.oldest)}</strong>
    </div>
    <div>
      <span>Newest</span>
      <strong>{formatDateTimeMs(summary.newest)}</strong>
    </div>
  </section>

  <div class="controls" aria-label="Session list controls">
    <div class="segmented">
      {#each ['all', 'active', 'failed', 'completed'] as option}
        <button
          type="button"
          class:active={filter === option}
          onclick={() => filter = option as SessionFilter}
        >
          {option}
        </button>
      {/each}
    </div>
    <div class="segmented">
      <button type="button" class:active={sortMode === 'newest'} onclick={() => sortMode = 'newest'}>newest</button>
      <button type="button" class:active={sortMode === 'oldest'} onclick={() => sortMode = 'oldest'}>oldest</button>
    </div>
  </div>

  {#if !loaded}
    <p class="empty">Loading sessions...</p>
  {:else if visibleSessions.length === 0}
    <p class="empty">No sessions match this view.</p>
  {:else}
    <div class="session-table">
      <div class="thead">
        <span>Status</span>
        <span>Activity</span>
        <span>Task / result</span>
        <span>Identity</span>
        <span>Turns</span>
        <span>Tokens / cost</span>
        <span>Relations</span>
        <span>Entity</span>
      </div>
      {#each visibleSessions as session (entityId(session))}
        <div class="row">
          <a class="status-cell" href={`${base}/sessions/${entityId(session)}`}>
            <StatusBadge status={status(session)} />
          </a>
          <a class="mono muted" href={`${base}/sessions/${entityId(session)}`}>
            {formatDateTimeMs(lastActivityMs(session))}
          </a>
          <a class="task" href={`${base}/sessions/${entityId(session)}`}>
            <strong>{shortText(sessionTitle(session), 160)}</strong>
            {#if textValue(field(session, ['error_message', 'ErrorMessage'])) !== '-'}
              <span class="error-text">{shortText(field(session, ['error_message', 'ErrorMessage']), 120)}</span>
            {:else if textValue(field(session, ['result', 'Result'])) !== '-'}
              <span>{shortText(field(session, ['result', 'Result']), 120)}</span>
            {/if}
          </a>
          <span class="mono">{shortText(sessionKind(session), 96)}</span>
          <span class="mono">{textValue(field(session, ['turn_count', 'TurnCount']))} / {textValue(field(session, ['max_turns', 'MaxTurns']))}</span>
          <span class="mono muted">{tokenSummary(session)}</span>
          <span class="mono muted">{relationSummary(session)}</span>
          <a class="entity-link" href={`${base}/entities/Sessions/${entityId(session)}`}>
            {truncateMiddle(entityId(session))}
          </a>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .page {
    width: min(1480px, 100%);
    padding: var(--sp-6);
  }

  .page-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-6);
    margin-bottom: var(--sp-4);
  }

  .eyebrow {
    margin: 0 0 var(--sp-1) 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  h1 {
    margin: 0 0 var(--sp-2) 0;
    letter-spacing: 0;
  }

  .page-head p:not(.eyebrow) {
    margin: 0;
    max-width: 760px;
    color: var(--text-2);
  }

  button {
    min-height: 32px;
    border: 1px solid var(--border-strong);
    padding: 0 var(--sp-3);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-1);
    background: var(--surface);
  }

  button:disabled {
    color: var(--text-3);
    cursor: default;
  }

  .notice,
  .empty {
    border: 1px solid var(--border);
    padding: var(--sp-3);
    margin-bottom: var(--sp-3);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-2);
  }

  .notice-error {
    border-color: var(--authz-denied-border);
    background: var(--authz-denied-bg);
    color: var(--status-error);
  }

  .summary {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    margin-bottom: var(--sp-4);
  }

  .summary div {
    min-width: 0;
    padding: var(--sp-3);
    border-right: 1px solid var(--border);
  }

  .summary div:last-child {
    border-right: 0;
  }

  .summary span,
  .summary strong,
  .mono,
  .thead,
  .entity-link {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .summary span,
  .thead,
  .muted {
    color: var(--text-3);
  }

  .summary strong {
    display: block;
    margin-top: 2px;
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-3);
    margin-bottom: var(--sp-3);
  }

  .segmented {
    display: flex;
    gap: 1px;
    border: 1px solid var(--border);
    background: var(--border);
  }

  .segmented button {
    border: 0;
    background: var(--bg);
    color: var(--text-2);
  }

  .segmented button.active {
    background: var(--surface-raised);
    color: var(--text-1);
  }

  .session-table {
    overflow-x: auto;
    border-top: 1px solid var(--border);
  }

  .thead,
  .row {
    display: grid;
    grid-template-columns:
      minmax(92px, 0.55fr)
      minmax(124px, 0.7fr)
      minmax(300px, 1.7fr)
      minmax(180px, 1fr)
      minmax(80px, 0.45fr)
      minmax(132px, 0.75fr)
      minmax(150px, 0.85fr)
      minmax(132px, 0.7fr);
    gap: var(--sp-3);
    align-items: center;
    min-width: 1220px;
    padding: var(--sp-2) 0;
    border-bottom: 1px solid var(--border);
  }

  .thead {
    color: var(--text-3);
  }

  .row a {
    text-decoration: none;
  }

  .status-cell {
    width: fit-content;
  }

  .task {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .task strong {
    font-size: var(--text-sm);
    font-weight: 500;
    color: var(--text-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .task span {
    font-size: var(--text-xs);
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .error-text {
    color: var(--status-error) !important;
  }

  .mono,
  .entity-link {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-2);
  }

  .entity-link {
    color: var(--accent);
  }

  @media (max-width: 900px) {
    .page {
      padding: var(--sp-4);
    }

    .page-head,
    .controls {
      display: block;
    }

    .page-head button,
    .controls .segmented {
      margin-top: var(--sp-3);
    }

    .summary {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .summary div:nth-child(2n) {
      border-right: 0;
    }
  }
</style>
