<script lang="ts">
  import { base } from '$app/paths';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import {
    asArray,
    entityId,
    fieldLabel,
    parseJsonString,
    readField,
    snakeCaseKey,
    textValue,
    truncateMiddle
  } from '$lib/entity-format';
  import { formatDateTimeMs, lastActivityMs, shortText } from '$lib/dashboard-format';

  let {
    title,
    entitySet,
    rows = [],
    columns = [],
    loadError = ''
  } = $props<{
    title: string;
    entitySet: string;
    rows: Record<string, unknown>[];
    columns: string[];
    loadError?: string;
  }>();

  function cellValue(row: Record<string, unknown>, column: string): string {
    return textValue(readField(row, column));
  }

  function linkedSetForColumn(column: string): string | null {
    const key = snakeCaseKey(column);
    if (!key.endsWith('_id')) return null;
    const baseName = key.slice(0, -3);
    const map: Record<string, string> = {
      factory_case: 'FactoryCases',
      signal: 'Signals',
      patrol_run: 'PatrolRuns',
      repo_graph_snapshot: 'RepoGraphSnapshots',
      work_cycle: 'WorkCycles',
      implementer_worker_run: 'WorkerRuns',
      worker_run: 'WorkerRuns',
      review_run: 'ReviewRuns',
      reviewer_run: 'ReviewRuns',
      evaluation_run: 'EvaluationRuns',
      proof_packet: 'ProofPackets',
      observability_finding: 'ObservabilityFindings',
      quality_finding: 'QualityFindings',
      security_finding: 'SecurityFindings',
      worker_provider: 'WorkerProviders',
      worker_agent: 'WorkerAgents',
      pm_issue: 'Issues',
      session: 'Sessions',
      assessment_session: 'Sessions'
    };
    return map[baseName] ?? null;
  }

  function jsonIdList(value: unknown): string[] {
    const parsed = parseJsonString(value);
    return asArray(parsed).map(String).filter(Boolean);
  }
</script>

<section class="board">
  <div class="board-head">
    <div>
      <h2>{title}</h2>
      <p>{entitySet}</p>
    </div>
    <span>{rows.length}</span>
  </div>

  {#if loadError}
    <div class="empty empty-error">{loadError}</div>
  {:else if rows.length === 0}
    <div class="empty">No entities yet</div>
  {:else}
    <div class="table" style={`--cols:${columns.length}`}>
      <div class="th">Entity</div>
      <div class="th">Activity</div>
      {#each columns as column}
        <div class="th">{fieldLabel(column)}</div>
      {/each}

      {#each rows as row (entityId(row))}
        <a class="cell entity" href={`${base}/entities/${entitySet}/${entityId(row)}`}>
          {truncateMiddle(entityId(row))}
        </a>
        <div class="cell muted">{formatDateTimeMs(lastActivityMs(row))}</div>
        {#each columns as column}
          {@const value = readField(row, column)}
          {@const linkedSet = linkedSetForColumn(column)}
          {@const ids = jsonIdList(value)}
          <div class="cell" title={cellValue(row, column)}>
            {#if column.toLowerCase() === 'status'}
              <StatusBadge status={cellValue(row, column)} />
            {:else if linkedSet && cellValue(row, column) !== '-'}
              <a class="inline-link" href={`${base}/entities/${linkedSet}/${cellValue(row, column)}`}>
                {truncateMiddle(cellValue(row, column))}
              </a>
            {:else if column.toLowerCase().endsWith('ids') && ids.length > 0}
              <span class="id-list">{ids.slice(0, 3).map((id) => truncateMiddle(id, 8, 5)).join(', ')}{ids.length > 3 ? ` +${ids.length - 3}` : ''}</span>
            {:else}
              {shortText(value, 90)}
            {/if}
          </div>
        {/each}
      {/each}
    </div>
  {/if}
</section>

<style>
  .board {
    border-top: 1px solid var(--border);
    padding: var(--sp-4) 0;
    min-width: 0;
  }

  .board-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-3);
    margin-bottom: var(--sp-2);
  }

  h2 {
    font-size: var(--text-base);
    margin: 0;
  }

  .board-head p {
    margin-top: 2px;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .board-head span,
  .empty,
  .th,
  .cell {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .board-head span,
  .empty,
  .th {
    color: var(--text-3);
  }

  .empty {
    border: 1px dashed var(--border);
    padding: var(--sp-3);
  }

  .empty-error {
    border-color: var(--authz-denied-border);
    color: var(--status-error);
    background: var(--authz-denied-bg);
  }

  .table {
    display: grid;
    grid-template-columns: minmax(132px, 0.9fr) minmax(112px, 0.55fr) repeat(var(--cols), minmax(118px, 1fr));
    overflow-x: auto;
  }

  .th,
  .cell {
    min-width: 0;
    padding: 7px var(--sp-2);
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .th {
    color: var(--text-3);
  }

  .cell {
    color: var(--text-2);
  }

  .entity {
    color: var(--text-1);
  }

  .muted,
  .id-list {
    color: var(--text-3);
  }

  .inline-link {
    color: var(--accent);
    text-decoration: none;
  }

  .inline-link:hover {
    text-decoration: underline;
  }
</style>
