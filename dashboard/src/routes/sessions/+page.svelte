<script lang="ts">
  import { base } from '$app/paths';
  import { onMount } from 'svelte';
  import { queryEntities } from '$lib/api';
  import StatusBadge from '$lib/components/StatusBadge.svelte';

  interface SessionRow {
    Id: string;
    Status: string;
    agent_id: string;
    user_message: string;
    turn_count: number;
    model: string;
    cost_cents: string;
    last_heartbeat_at: string;
  }

  let sessions = $state<SessionRow[]>([]);
  let loaded = $state(false);

  function formatTime(ts: string): string {
    if (!ts) return '--';
    const d = new Date(ts);
    return d.toLocaleString();
  }

  function truncate(s: string, n: number): string {
    if (!s) return '--';
    return s.length > n ? s.slice(0, n) + '...' : s;
  }

  onMount(async () => {
    try {
      const data = await queryEntities('Sessions', undefined, 'SequenceNr desc', 50);
      sessions = data as unknown as SessionRow[];
    } catch {
      // ignore
    }
    loaded = true;
  });
</script>

<div class="page">
  <h1 class="page-title">SESSIONS</h1>

  {#if !loaded}
    <p class="empty">Loading...</p>
  {:else if sessions.length === 0}
    <p class="empty">No sessions yet.</p>
  {:else}
    <div class="table">
      <div class="thead">
        <span class="th th-status">STATUS</span>
        <span class="th th-task">TASK</span>
        <span class="th th-agent">AGENT</span>
        <span class="th th-turns">TURNS</span>
        <span class="th th-time">TIME</span>
      </div>
      {#each sessions as s (s.Id)}
        <a href="{base}/sessions/{s.Id}" class="row">
          <span class="td td-status"><StatusBadge status={s.Status} /></span>
          <span class="td td-task">{truncate(s.user_message, 80)}</span>
          <span class="td td-agent">{s.agent_id || '--'}</span>
          <span class="td td-turns">{s.turn_count ?? 0}</span>
          <span class="td td-time">{formatTime(s.last_heartbeat_at)}</span>
        </a>
      {/each}
    </div>
  {/if}
</div>

<style>
  .page { max-width: 960px; }

  .page-title {
    font-family: var(--font-sans);
    font-size: var(--text-xl);
    color: var(--text-1);
    margin-bottom: var(--sp-8);
    letter-spacing: 0.1em;
  }

  .empty {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .table {
    display: flex;
    flex-direction: column;
  }

  .thead {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-3);
    border-bottom: 1px solid var(--border);
  }

  .th {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    letter-spacing: 0.06em;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-3);
    border-bottom: 1px solid var(--border);
    text-decoration: none;
    color: inherit;
    transition: background var(--duration) var(--ease);
  }

  .row:hover {
    background: var(--surface);
  }

  .td {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .th-status, .td-status { width: 80px; flex-shrink: 0; }
  .th-task, .td-task { flex: 1; min-width: 0; }
  .td-task { color: var(--text-2); }
  .th-agent, .td-agent { width: 100px; flex-shrink: 0; }
  .td-agent { color: var(--text-3); }
  .th-turns, .td-turns { width: 60px; flex-shrink: 0; text-align: right; }
  .th-time, .td-time { width: 160px; flex-shrink: 0; }
  .td-time { color: var(--text-3); }

  @media (max-width: 640px) {
    .page { max-width: 100%; }
    .thead { display: none; }
    .row {
      flex-wrap: wrap;
      gap: var(--sp-1);
      padding: var(--sp-2);
    }
    .td-status { width: auto; }
    .td-task { width: 100%; order: -1; }
    .td-agent { width: auto; }
    .td-turns { width: auto; text-align: left; }
    .td-time { width: auto; }
  }
</style>
