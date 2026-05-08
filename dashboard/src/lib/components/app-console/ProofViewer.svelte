<script lang="ts">
  import { asArray, asRecord, parseJsonString, readField, textValue } from '$lib/entity-format';

  let {
    proofs = []
  } = $props<{
    proofs: Record<string, unknown>[];
  }>();

  const latest = $derived(proofs[0] ?? null);
  const readyLatest = $derived(
    proofs.find((proof: Record<string, unknown>) => textValue(readField(proof, 'Status')) === 'Ready')
      ?? latest
  );
  const proofData = $derived(asRecord(parseJsonString(readField(readyLatest, 'proof_json'))));
  const activeMonitors = $derived(asArray(proofData.active_monitors));
  const visualSummaryUrl = $derived(textValue(readField(readyLatest, 'visual_summary_url') ?? proofData.visual_summary_url));
  const stateDiagram = $derived(textValue(readField(readyLatest, 'state_diagram') ?? proofData.state_diagram));
  const changedFiles = $derived(asArray(proofData.changed_files).map(String));

  function value(key: string): string {
    return textValue(readField(readyLatest, key));
  }
</script>

<section class="proof">
  <div class="proof-head">
    <h2>Proof</h2>
    <span>{proofs.length}</span>
  </div>

  {#if readyLatest}
    <div class="proof-grid">
      <div>
        <span>Latest</span>
        <strong>{value('Id')}</strong>
      </div>
      <div>
        <span>Status</span>
        <strong>{value('Status')}</strong>
      </div>
      <div>
        <span>Reviewer</span>
        <strong>{value('ReviewerVerdict')}</strong>
      </div>
      <div>
        <span>Active monitors</span>
        <strong>{String(proofData.active_monitor_count ?? '-')}</strong>
      </div>
    </div>
    {#if activeMonitors.length > 0}
      <div class="monitor-strip" aria-label="Active monitors found by latest proof">
        {#each activeMonitors.slice(0, 6) as monitor}
          {@const item = asRecord(monitor)}
          <span>{textValue(item.name)} · {textValue(item.status)}</span>
        {/each}
      </div>
    {/if}
    {#if visualSummaryUrl !== '-'}
      <figure class="visual-proof">
        <img src={visualSummaryUrl} alt="Visual proof summary for latest Paw Patrol proof packet" />
      </figure>
    {/if}
    {#if changedFiles.length > 0}
      <div class="file-strip" aria-label="Changed files in latest proof">
        {#each changedFiles.slice(0, 8) as file}
          <span>{file}</span>
        {/each}
      </div>
    {/if}
    {#if stateDiagram !== '-'}
      <pre class="diagram">{stateDiagram}</pre>
    {/if}
    <pre>{value('SummaryMarkdown')}</pre>
  {:else}
    <div class="empty">No ProofPackets yet</div>
  {/if}
</section>

<style>
  .proof {
    border-top: 1px solid var(--border);
    padding: var(--sp-4) 0;
  }

  .proof-head {
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

  .proof-head span,
  .proof-grid span,
  .empty {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .proof-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--sp-2);
    margin-bottom: var(--sp-3);
  }

  .proof-grid div {
    border: 1px solid var(--border);
    padding: var(--sp-2);
    min-width: 0;
  }

  .proof-grid strong {
    display: block;
    margin-top: 2px;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 500;
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  pre,
  .monitor-strip,
  .file-strip,
  .visual-proof,
  .empty {
    border: 1px dashed var(--border);
    padding: var(--sp-3);
    overflow: auto;
    max-height: 220px;
  }

  pre {
    color: var(--text-2);
    white-space: pre-wrap;
  }

  .monitor-strip {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
    margin-bottom: var(--sp-3);
    border-style: solid;
  }

  .file-strip {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
    margin-bottom: var(--sp-3);
    border-style: solid;
  }

  .visual-proof {
    margin: 0 0 var(--sp-3) 0;
    border-style: solid;
    max-height: none;
  }

  .visual-proof img {
    width: 100%;
    max-height: 420px;
    object-fit: contain;
  }

  .diagram {
    max-height: 280px;
    margin-bottom: var(--sp-3);
  }

  .monitor-strip span,
  .file-strip span {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-2);
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (max-width: 720px) {
    .proof-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
