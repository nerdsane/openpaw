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

  function jsonField(row: Record<string, unknown> | null | undefined, key: string): Record<string, unknown> {
    return asRecord(parseJsonString(readField(row, key)));
  }

  function isDatadogMcpProof(proof: Record<string, unknown>): boolean {
    const data = jsonField(proof, 'proof_json');
    return ['datadog_observability', 'github_repository'].includes(String(data.kind ?? ''))
      || ['codex_datadog_mcp_agent', 'codex_github_agent'].includes(String(data.evidence_source ?? ''));
  }

  function isDatadogMcpPatrol(run: Record<string, unknown>): boolean {
    const data = jsonField(run, 'evidence_json');
    return ['codex_datadog_mcp_agent', 'codex_github_agent'].includes(String(data.evidence_source ?? ''))
      || ['datadog_observability', 'github_repository'].includes(String(readField(run, 'patrol_kind') ?? ''));
  }

  const latestPatrol = $derived(
    patrolRuns.find((run: Record<string, unknown>) => entityStatus(run) === 'Complete' && isDatadogMcpPatrol(run))
      ?? patrolRuns.find((run: Record<string, unknown>) => isDatadogMcpPatrol(run))
      ?? patrolRuns[0]
      ?? null
  );
  const latestProof = $derived(
    proofs.find((proof: Record<string, unknown>) => entityStatus(proof) === 'Ready' && isDatadogMcpProof(proof))
      ?? proofs.find((proof: Record<string, unknown>) => isDatadogMcpProof(proof))
      ?? proofs.find((proof: Record<string, unknown>) => entityStatus(proof) === 'Ready')
      ?? proofs[0]
      ?? null
  );
  const proofData = $derived(jsonField(latestProof, 'proof_json'));
  const created = $derived(asRecord(proofData.created));
  const evidenceScope = $derived(asArray(proofData.evidence_scope).map(asRecord));
  const patrolFindings = $derived(asArray(proofData.findings).map(asRecord));
  const patrolSummary = $derived(String(proofData.summary ?? '').trim());
  const residualRisks = $derived(asArray(proofData.residual_risks).map(String));
  const queuedImplementers = $derived(asArray(created.implementer_worker_runs).length);
  const createdFindingIds = $derived(asArray(created.observability_findings).map(String));
  const createdSignalIds = $derived(asArray(created.signals).map(String));
  const proofKind = $derived(String(proofData.kind ?? readField(latestPatrol, 'patrol_kind') ?? 'patrol').trim());
  const proofTitle = $derived(proofKind === 'github_repository' ? 'GitHub Repository Patrol' : 'Datadog MCP Patrol');
  const openedFindings = $derived(
    createdFindingIds.length > 0
      ? createdFindingIds
          .map((id: string) => findings.find((finding: Record<string, unknown>) => entityId(finding) === id))
          .filter((finding): finding is Record<string, unknown> => Boolean(finding))
      : findings
  );
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
      <h2>{proofTitle}</h2>
      <p>
        Local Codex investigates the external surface with authenticated tools, then the worker
        records the agent's Signals, Findings or Cases, gated WorkCycles, and ProofPacket back into Temper.
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
      <span>Surfaces checked</span>
      <strong>{String(evidenceScope.length || '-')}</strong>
    </div>
    <div>
      <span>Agent findings</span>
      <strong>{String(patrolFindings.length || '-')}</strong>
    </div>
    <div>
      <span>Signals / findings opened</span>
      <strong>{String(createdSignalIds.length || createdFindingIds.length || findings.length || '-')}</strong>
    </div>
    <div>
      <span>Queued / gated work</span>
      <strong>{queuedImplementers}/{String(asArray(created.work_cycles).length || gatedCycles.length || '-')}</strong>
    </div>
  </div>

  <div class="evidence-grid">
    <article class="panel panel-main">
      <h3>Agent Summary</h3>
      {#if patrolSummary}
        <p>{patrolSummary}</p>
      {:else}
        <p>No Datadog MCP patrol summary has been recorded yet.</p>
      {/if}
    </article>

    <article class="panel">
      <h3>Opened Work</h3>
      {#if openedFindings.length > 0 || patrolFindings.length > 0}
        <ul class="finding-list">
          {#each openedFindings.slice(0, 5) as finding}
            <li>
              <a href={link('ObservabilityFindings', entityId(finding))}>
                {textValue(readField(finding, 'title'))}
              </a>
              <span>{textValue(readField(finding, 'risk_lane'))} · {textValue(readField(finding, 'severity'))}</span>
            </li>
          {/each}
          {#if openedFindings.length === 0}
            {#each patrolFindings.slice(0, 5) as finding}
              <li>
                <span>{textValue(readField(finding, 'title'))}</span>
                <span>{textValue(readField(finding, 'risk_lane'))} · {textValue(readField(finding, 'severity'))}</span>
              </li>
            {/each}
          {/if}
        </ul>
      {:else}
        <p>No ObservabilityFindings loaded yet.</p>
      {/if}
    </article>

    <article class="panel">
      <h3>Control Posture</h3>
      <dl>
        <div>
          <dt>Investigator</dt>
          <dd>{textValue(proofData.evidence_source ?? 'codex_patrol_agent')}</dd>
        </div>
        <div>
          <dt>Evidence Areas</dt>
          <dd>{evidenceScope.map((scope) => textValue(readField(scope, 'surface'))).join(', ') || 'Awaiting agent evidence.'}</dd>
        </div>
        <div>
          <dt>Safety Gate</dt>
          <dd>Production-impacting fixes stop at WorkCycles until explicit approval.</dd>
        </div>
      </dl>
    </article>
  </div>

  {#if evidenceScope.length > 0}
    <div class="monitor-table">
      <div class="table-head">
        <span>Evidence Surface</span>
        <span>Agent Query</span>
        <span>Result</span>
      </div>
      {#each evidenceScope as scope}
        <div class="table-row">
          <span>{textValue(readField(scope, 'surface'))}</span>
          <span>{textValue(readField(scope, 'query'))}</span>
          <span>{textValue(readField(scope, 'result_summary'))}</span>
        </div>
      {/each}
    </div>
  {/if}

  {#if residualRisks.length > 0}
    <div class="risk-strip">
      <span>Residual risks</span>
      <p>{residualRisks.join(' | ')}</p>
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

  .panel-main p {
    font-size: var(--text-sm);
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

  .risk-strip {
    display: grid;
    grid-template-columns: 160px minmax(0, 1fr);
    gap: var(--sp-3);
    border: 1px solid var(--border);
    margin-top: var(--sp-4);
    padding: var(--sp-3);
  }

  .risk-strip span {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    text-transform: uppercase;
  }

  .risk-strip p {
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

    .risk-strip {
      grid-template-columns: 1fr;
    }
  }
</style>
