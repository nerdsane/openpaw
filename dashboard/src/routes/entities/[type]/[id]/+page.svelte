<script lang="ts">
  import { base } from '$app/paths';
  import { fade } from 'svelte/transition';
  import { page } from '$app/stores';
  import { fetchEntityHistory, getEntity, type SessionHistoryEntry } from '$lib/api';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import { asArray, fieldLabel, parseJsonString, readField, snakeCaseKey, textValue, truncateMiddle } from '$lib/entity-format';
  import { asEventArray, formatDateTimeMs, lastActivityMs, shortText } from '$lib/dashboard-format';

  let entitySetParam = $derived($page.params.type ?? '');
  let entityId = $derived($page.params.id ?? '');
  let entitySetName = $derived(entitySetParam);

  let entity = $state<Record<string, unknown> | null>(null);
  let history = $state<SessionHistoryEntry[]>([]);
  let loaded = $state(false);
  let error = $state<string | null>(null);

  let expandedFields = $state<Set<string>>(new Set());

  interface TimelineRow {
    timestamp: string;
    action: string;
    from_status: string;
    to_status: string;
    authz_denied: boolean;
    error: string | null;
    source: 'audit' | 'entity';
  }

  const entityLinks: Record<string, string> = {
    factory_case_id: 'FactoryCases',
    signal_id: 'Signals',
    patrol_run_id: 'PatrolRuns',
    repo_graph_snapshot_id: 'RepoGraphSnapshots',
    work_cycle_id: 'WorkCycles',
    implementer_worker_run_id: 'WorkerRuns',
    worker_run_id: 'WorkerRuns',
    review_run_id: 'ReviewRuns',
    reviewer_run_id: 'ReviewRuns',
    evaluation_run_id: 'EvaluationRuns',
    proof_packet_id: 'ProofPackets',
    proof_packet_ids: 'ProofPackets',
    observability_finding_id: 'ObservabilityFindings',
    observability_finding_ids: 'ObservabilityFindings',
    quality_finding_id: 'QualityFindings',
    quality_finding_ids: 'QualityFindings',
    security_finding_id: 'SecurityFindings',
    security_finding_ids: 'SecurityFindings',
    worker_provider_id: 'WorkerProviders',
    worker_agent_id: 'WorkerAgents',
    daily_brief_id: 'DailyBriefs',
    patrol_schedule_id: 'PatrolSchedules',
    session_id: 'Sessions',
    assessment_session_id: 'Sessions',
    parent_session_id: 'Sessions',
    pm_issue_id: 'Issues'
  };

  const primaryFields = new Set([
    'id',
    'status',
    'title',
    'summary',
    'task_summary',
    'plan_summary',
    'plan_revision_count',
    'request_text',
    'source',
    'severity',
    'risk_lane',
    'minimum_risk_lane',
    'factory_case_id',
    'work_cycle_id',
    'worker_run_id',
    'reviewer_run_id',
    'evaluation_run_id',
    'proof_packet_id',
    'session_id'
  ]);

  function toggleExpand(key: string) {
    const next = new Set(expandedFields);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    expandedFields = next;
  }

  function isJsonLike(value: unknown): boolean {
    if (typeof value !== 'string') return false;
    const trimmed = value.trim();
    return (trimmed.startsWith('{') && trimmed.endsWith('}')) ||
           (trimmed.startsWith('[') && trimmed.endsWith(']'));
  }

  function formatJson(value: string): string {
    try {
      return JSON.stringify(JSON.parse(value), null, 2);
    } catch {
      return value;
    }
  }

  function isLongString(value: unknown): boolean {
    return typeof value === 'string' && value.length > 120;
  }

  function linkedEntitySet(key: string, value: unknown): string | null {
    if (typeof value !== 'string' || !value.trim() || isJsonLike(value)) return null;
    return entityLinks[snakeCaseKey(key)] ?? null;
  }

  function linkedEntitySetForList(key: string): string | null {
    return entityLinks[snakeCaseKey(key)] ?? null;
  }

  function idList(value: unknown): string[] {
    const parsed = parseJsonString(value);
    return asArray(parsed).map(String).filter(Boolean);
  }

  function fieldRank(key: string): number {
    const normalized = snakeCaseKey(key);
    if (normalized === 'id') return 0;
    if (normalized === 'status') return 1;
    if (primaryFields.has(normalized)) return 2;
    if (normalized.startsWith('_')) return 4;
    return 3;
  }

  function backHref(entitySet: string): string {
    if (entitySet === 'Sessions') return `${base}/sessions`;
    if ([
      'WorkRequests',
      'PatrolRequests',
      'Signals',
      'PatrolRuns',
      'ObservabilityFindings',
      'RepoGraphSnapshots',
      'QualityFindings',
      'SecurityFindings',
      'FactoryCases',
      'WorkCycles',
      'WorkerRuns',
      'ReviewRuns',
      'EvaluationRuns',
      'ProofPackets',
      'DailyBriefs',
      'PatrolSchedules',
      'WorkerAgents',
      'WorkerProviders',
      'RiskRules'
    ].includes(entitySet)) return `${base}/apps/paw-patrol`;
    return `${base}/apps`;
  }

  async function loadEntityDetail(type: string, id: string) {
    entity = null;
    history = [];
    error = null;
    loaded = false;
    expandedFields = new Set();

    if (!id) {
      error = `Could not load ${type}`;
      loaded = true;
      return;
    }

    try {
      entity = await getEntity(type, id);
      history = await fetchEntityHistory(type, id, 200).catch(() => []);
    } catch (err) {
      error = err instanceof Error ? err.message : `Could not load ${type} ${id}`;
    }
    loaded = true;
  }

  $effect(() => {
    void loadEntityDetail(entitySetName, entityId);
  });

  let status = $derived(String(readField(entity, 'Status') ?? ''));
  let fields = $derived.by((): Array<[string, unknown]> => {
    if (!entity) return [];
    return Object.entries(entity)
      .filter(([k]) => !k.startsWith('@odata') && !k.startsWith('odata'))
      .sort(([left], [right]) => fieldRank(left) - fieldRank(right) || fieldLabel(left).localeCompare(fieldLabel(right)));
  });
  let timeline = $derived.by((): TimelineRow[] => {
    const auditRows = history.map((item) => ({ ...item, source: 'audit' as const }));
    const entityRows = asEventArray(entity).map((event) => ({
      timestamp: event.timestamp,
      action: event.action,
      from_status: event.from_status,
      to_status: event.to_status,
      authz_denied: false,
      error: null,
      source: 'entity' as const
    }));
    return (auditRows.length > 0 ? auditRows : entityRows)
      .sort((left, right) => Date.parse(right.timestamp) - Date.parse(left.timestamp));
  });
</script>

<div class="entity-detail">
  <a href={backHref(entitySetName)} class="entity-back">&larr; BACK</a>

  {#if !loaded}
    <div class="entity-empty" transition:fade={{ duration: 200 }}>
      <span class="entity-loading">[LOADING...]</span>
    </div>
  {:else if error || !entity}
    <div class="entity-empty" transition:fade={{ duration: 200 }}>
      <span class="entity-loading">[ERROR: {error ?? 'Entity not found'}]</span>
    </div>
  {:else}
    <header class="entity-header">
      <div class="entity-title">
        <code class="entity-type-label">{entitySetParam}</code>
        <code class="entity-id-label">{entityId}</code>
      </div>
      {#if status}
        <StatusBadge {status} />
      {/if}
    </header>

    <section class="entity-summary" aria-label="Entity summary">
      <div>
        <span>Last activity</span>
        <strong>{formatDateTimeMs(lastActivityMs(entity))}</strong>
      </div>
      <div>
        <span>Timeline events</span>
        <strong>{timeline.length}</strong>
      </div>
      <div>
        <span>Sequence</span>
        <strong>{textValue(readField(entity, '_sequence_nr'))}</strong>
      </div>
      <div>
        <span>Total events</span>
        <strong>{textValue(readField(entity, '_total_event_count'))}</strong>
      </div>
    </section>

    <div class="field-table">
      {#each fields as [key, value] (key)}
        <div class="field-row">
          <span class="field-key">{fieldLabel(key)}</span>
          <div class="field-value">
            {#if linkedEntitySetForList(key) && idList(value).length > 0}
              <div class="field-links">
                {#each idList(value) as linkedId}
                  <a class="field-link" href={`${base}/entities/${linkedEntitySetForList(key)}/${linkedId}`}>
                    {truncateMiddle(linkedId)}
                  </a>
                {/each}
              </div>
            {:else if isJsonLike(value)}
              <pre class="field-json">{formatJson(value as string)}</pre>
            {:else if isLongString(value) && !expandedFields.has(key)}
              <span class="field-text">{(value as string).slice(0, 120)}...</span>
              <button class="field-expand" onclick={() => toggleExpand(key)}>[+] MORE</button>
            {:else if isLongString(value) && expandedFields.has(key)}
              <span class="field-text">{value}</span>
              <button class="field-expand" onclick={() => toggleExpand(key)}>[-] LESS</button>
            {:else if value === null || value === undefined}
              <span class="field-null">--</span>
            {:else if typeof value === 'boolean'}
              <span class="field-bool">{value ? 'TRUE' : 'FALSE'}</span>
            {:else if linkedEntitySet(key, value)}
              <a class="field-link" href={`${base}/entities/${linkedEntitySet(key, value)}/${value}`}>
                {truncateMiddle(String(value))}
              </a>
            {:else}
              <span class="field-text">{shortText(value, 600)}</span>
            {/if}
          </div>
        </div>
      {/each}
    </div>

    <div class="entity-history">
      <span class="history-label">HISTORY</span>
      {#if timeline.length === 0}
        <span class="history-text">No history or entity event rows were returned for this entity.</span>
      {:else}
        <div class="history-table">
          {#each timeline as item}
            <div class="history-row">
              <span class="history-time">{formatDateTimeMs(Date.parse(item.timestamp))}</span>
              <span class="history-action">{item.action}</span>
              <span class="history-state">{item.from_status || '-'} -> {item.to_status || '-'}</span>
              <span class={item.authz_denied ? 'history-denied' : ''}>{item.authz_denied ? 'denied' : 'allowed'}</span>
              {#if item.error}
                <span class="history-error">{item.error}</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .entity-detail {
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
  }

  .entity-back {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.08em;
    color: var(--text-2);
    text-decoration: none;
    width: fit-content;
  }

  .entity-back:hover {
    color: var(--text-1);
  }

  .entity-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--sp-8) 0;
  }

  .entity-loading {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    letter-spacing: 0.06em;
  }

  .entity-header {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    flex-wrap: wrap;
  }

  .entity-summary {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
  }

  .entity-summary div {
    min-width: 0;
    padding: var(--sp-3);
    border-right: 1px solid var(--border);
  }

  .entity-summary div:last-child {
    border-right: 0;
  }

  .entity-summary span,
  .entity-summary strong {
    display: block;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .entity-summary span {
    color: var(--text-3);
  }

  .entity-summary strong {
    margin-top: 2px;
    color: var(--text-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entity-title {
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
  }

  .entity-type-label {
    font-family: var(--font-mono);
    font-size: var(--text-lg);
    color: var(--text-1);
    letter-spacing: -0.01em;
  }

  .entity-id-label {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .field-table {
    display: flex;
    flex-direction: column;
  }

  .field-row {
    display: flex;
    gap: var(--sp-6);
    padding: var(--sp-2) 0;
    border-bottom: 1px solid var(--border);
    align-items: flex-start;
  }

  .field-key {
    width: 180px;
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-2);
  }

  .field-value {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }

  .field-text {
    font-size: var(--text-sm);
    color: var(--text-1);
    word-break: break-word;
  }

  .field-link {
    width: fit-content;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--accent);
    text-decoration: none;
    overflow-wrap: anywhere;
  }

  .field-links {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }

  .field-link:hover {
    text-decoration: underline;
  }

  .field-null {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .field-bool {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-1);
    letter-spacing: 0.04em;
  }

  .field-json {
    font-size: var(--text-xs);
    max-height: 200px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-all;
    background: var(--surface);
    border: 1px solid var(--border);
    padding: var(--sp-2);
    border-radius: var(--radius);
  }

  .field-expand {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.06em;
    color: var(--text-3);
    padding: 0;
    text-align: left;
    width: fit-content;
  }

  .field-expand:hover {
    color: var(--text-1);
  }

  .entity-history {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin-top: var(--sp-8);
    padding-top: var(--sp-6);
    border-top: 1px solid var(--border);
  }

  .history-label {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.08em;
    color: var(--text-2);
  }

  .history-text {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .history-table {
    display: grid;
    border-top: 1px solid var(--border);
  }

  .history-row {
    display: grid;
    grid-template-columns: 132px minmax(120px, 0.8fr) minmax(120px, 0.8fr) 72px minmax(0, 1fr);
    gap: var(--sp-3);
    align-items: baseline;
    padding: var(--sp-2) 0;
    border-bottom: 1px solid var(--border);
    min-width: 0;
  }

  .history-row span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-2);
  }

  .history-time,
  .history-state {
    color: var(--text-3) !important;
  }

  .history-denied,
  .history-error {
    color: var(--status-error) !important;
  }

  @media (max-width: 760px) {
    .entity-summary {
      grid-template-columns: 1fr 1fr;
    }

    .entity-summary div:nth-child(2n) {
      border-right: 0;
    }

    .history-row {
      grid-template-columns: 1fr;
      gap: 2px;
    }
  }
</style>
