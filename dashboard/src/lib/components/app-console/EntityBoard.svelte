<script lang="ts">
  import { base } from '$app/paths';

  let {
    title,
    entitySet,
    rows = [],
    columns = []
  } = $props<{
    title: string;
    entitySet: string;
    rows: Record<string, unknown>[];
    columns: string[];
  }>();

  function entityId(row: Record<string, unknown>): string {
    return String(row.Id ?? row._entity_id ?? '');
  }

  function text(value: unknown): string {
    if (value === null || value === undefined || value === '') return '-';
    if (typeof value === 'object') return JSON.stringify(value);
    return String(value);
  }
</script>

<section class="board">
  <div class="board-head">
    <h2>{title}</h2>
    <span>{rows.length}</span>
  </div>

  {#if rows.length === 0}
    <div class="empty">No entities yet</div>
  {:else}
    <div class="table" style={`--cols:${columns.length}`}>
      <div class="th">Entity</div>
      {#each columns as column}
        <div class="th">{column}</div>
      {/each}

      {#each rows as row (entityId(row))}
        <a class="cell entity" href={`${base}/entities/${entitySet}/${entityId(row)}`}>
          {entityId(row)}
        </a>
        {#each columns as column}
          <div class="cell" title={text(row[column])}>{text(row[column])}</div>
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
    align-items: baseline;
    justify-content: space-between;
    gap: var(--sp-3);
    margin-bottom: var(--sp-2);
  }

  h2 {
    font-size: var(--text-base);
    margin: 0;
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

  .table {
    display: grid;
    grid-template-columns: minmax(120px, 0.9fr) repeat(var(--cols), minmax(96px, 1fr));
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
</style>
