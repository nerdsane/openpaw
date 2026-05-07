<script lang="ts">
  import { base } from '$app/paths';
  import {
    asArray,
    asRecord,
    entityId,
    entityStatus,
    parseJsonString,
    readField,
    textValue,
    truncateMiddle
  } from '$lib/entity-format';

  let {
    rows = {}
  } = $props<{
    rows: Record<string, Record<string, unknown>[]>;
  }>();

  const patrolRuns = $derived(rows.PatrolRuns ?? []);
  const proofs = $derived(rows.ProofPackets ?? []);
  const findings = $derived(rows.ObservabilityFindings ?? []);
  const workCycles = $derived(rows.WorkCycles ?? []);

  const latestPatrol = $derived(patrolRuns[0] ?? null);
  const latestProof = $derived(
    proofs.find((proof: Record<string, unknown>) => entityStatus(proof) === 'Ready') ?? proofs[0] ?? null
  );
  const proofData = $derived(asRecord(parseJsonString(readField(latestProof, 'proof_json'))));
  const created = $derived(asRecord(proofData.created));
  const activeMonitors = $derived(asArray(proofData.active_monitors).map(asRecord));
  const triageMemo = $derived(String(proofData.codex_analysis ?? '').trim());
  const gatedCycles = $derived(
    workCycles.filter((cycle: Record<string, unknown>) =>
      ['AwaitingHumanStartApproval', 'Planning', 'Scoped'].includes(entityStatus(cycle))
    )
  );

  function link(entitySet: string, id: unknown): string {
    return `${base}/entities/${entitySet}/${String(id)}`;
  }
</script>

<section class="overview">
  <div class="overview-head">
    <div>
      <p class="eyebrow">Latest Patrol Evidence</p>
      <h2>Datadog monitor coverage looks degraded</h2>
      <p>
        This run used read-only Datadog monitor evidence, then the local Mac mini Codex worker turned
        the evidence into Signals, Findings, Cases, gated WorkCycles, and a ProofPacket.
      </p>
    </div>
    {#if latestPatrol}
      <a class="run-link" href={link('PatrolRuns', entityId(latestPatrol))}>
        {truncateMiddle(entityId(latestPatrol))}
      </a>
    {/if}
  </div>

  <div class="metrics" aria-label="Latest Patrol metrics">
    <div>
      <span>Monitors scanned</span>
      <strong>{String(proofData.monitor_count ?? '-')}</strong>
    </div>
    <div>
      <span>Active / No Data</span>
      <strong>{String(proofData.active_monitor_count ?? '-')}</strong>
    </div>
    <div>
      <span>Findings opened</span>
      <strong>{String(asArray(created.observability_findings).length || findings.length || '-')}</strong>
    </div>
    <div>
      <span>Approval-gated work</span>
      <strong>{String(asArray(created.work_cycles).length || gatedCycles.length || '-')}</strong>
    </div>
  </div>

  <div class="evidence-grid">
    <article class="panel panel-main">
      <h3>What It Found</h3>
      {#if triageMemo}
        <pre>{triageMemo}</pre>
      {:else}
        <p>No agent triage memo has been recorded yet.</p>
      {/if}
    </article>

    <article class="panel">
      <h3>Opened Findings</h3>
      {#if findings.length > 0}
        <ul class="finding-list">
          {#each findings.slice(0, 5) as finding}
            <li>
              <a href={link('ObservabilityFindings', entityId(finding))}>
                {textValue(readField(finding, 'title'))}
              </a>
              <span>{textValue(readField(finding, 'risk_lane'))} · {textValue(readField(finding, 'datadog_monitor_id'))}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <p>No ObservabilityFindings loaded yet.</p>
      {/if}
    </article>

    <article class="panel">
      <h3>Evidence Scope</h3>
      <dl>
        <div>
          <dt>Collected</dt>
          <dd>{textValue(proofData.datadog_endpoint ?? '/api/v1/monitor/search')}</dd>
        </div>
        <div>
          <dt>Not Yet A Full Sweep</dt>
          <dd>Logs, traces, metrics, incidents, and dashboards are named in the agent instruction, but this v1 collector only persisted monitor-search evidence.</dd>
        </div>
        <div>
          <dt>Safety Gate</dt>
          <dd>Production-impacting fixes stop at WorkCycles until explicit approval.</dd>
        </div>
      </dl>
    </article>
  </div>

  {#if activeMonitors.length > 0}
    <div class="monitor-table">
      <div class="table-head">
        <span>Active Monitor</span>
        <span>Status</span>
        <span>Tags</span>
      </div>
      {#each activeMonitors.slice(0, 10) as monitor}
        <div class="table-row">
          <span>{textValue(monitor.name)}</span>
          <span>{textValue(monitor.status)}</span>
          <span>{asArray(monitor.tags).map(String).join(', ') || '-'}</span>
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .overview {
    border-top: 1px solid var(--border);
    padding: var(--sp-5) 0;
  }

  .overview-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-5);
    margin-bottom: var(--sp-4);
  }

  .eyebrow {
    margin: 0 0 var(--sp-1) 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  h2,
  h3,
  p {
    margin: 0;
  }

  h2 {
    font-size: var(--text-xl);
    letter-spacing: 0;
  }

  h3 {
    margin-bottom: var(--sp-2);
    font-size: var(--text-base);
  }

  .overview-head p,
  .panel p,
  dd {
    color: var(--text-2);
    line-height: 1.5;
  }

  .overview-head p {
    max-width: 780px;
    margin-top: var(--sp-2);
  }

  .run-link {
    flex: 0 0 auto;
    border: 1px solid var(--border);
    padding: var(--sp-2) var(--sp-3);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-1);
    text-decoration: none;
  }

  .metrics {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    border: 1px solid var(--border);
    margin-bottom: var(--sp-4);
  }

  .metrics div {
    padding: var(--sp-3);
    border-right: 1px solid var(--border);
  }

  .metrics div:last-child {
    border-right: 0;
  }

  .metrics span,
  dt,
  .table-head,
  .finding-list span {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .metrics strong {
    display: block;
    margin-top: var(--sp-1);
    font-size: var(--text-2xl);
    color: var(--text-1);
  }

  .evidence-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.3fr) minmax(280px, 0.8fr);
    gap: var(--sp-3);
  }

  .panel {
    border: 1px solid var(--border);
    padding: var(--sp-3);
    min-width: 0;
  }

  .panel-main {
    grid-row: span 2;
  }

  pre {
    max-height: 420px;
    margin: 0;
    overflow: auto;
    white-space: pre-wrap;
    color: var(--text-2);
  }

  .finding-list {
    display: grid;
    gap: var(--sp-2);
    padding: 0;
    margin: 0;
    list-style: none;
  }

  .finding-list li {
    display: grid;
    gap: 2px;
  }

  .finding-list a {
    color: var(--text-1);
    text-decoration: none;
  }

  .finding-list a:hover,
  .run-link:hover {
    color: var(--accent);
  }

  dl {
    display: grid;
    gap: var(--sp-3);
    margin: 0;
  }

  dt,
  dd {
    margin: 0;
  }

  .monitor-table {
    margin-top: var(--sp-4);
    border-top: 1px solid var(--border);
  }

  .table-head,
  .table-row {
    display: grid;
    grid-template-columns: minmax(220px, 1fr) minmax(80px, 0.3fr) minmax(180px, 0.8fr);
    gap: var(--sp-3);
    padding: var(--sp-2) 0;
    border-bottom: 1px solid var(--border);
  }

  .table-row span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-2);
  }

  @media (max-width: 900px) {
    .overview-head,
    .evidence-grid {
      display: block;
    }

    .run-link,
    .panel {
      margin-top: var(--sp-3);
    }

    .metrics {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .metrics div {
      border-right: 0;
      border-bottom: 1px solid var(--border);
    }

    .table-head,
    .table-row {
      grid-template-columns: 1fr;
      gap: var(--sp-1);
    }
  }
</style>
