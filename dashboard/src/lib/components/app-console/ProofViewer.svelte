<script lang="ts">
  let {
    proofs = []
  } = $props<{
    proofs: Record<string, unknown>[];
  }>();

  const latest = $derived(proofs[0] ?? null);

  function value(key: string): string {
    const raw = latest?.[key];
    if (raw === null || raw === undefined || raw === '') return '-';
    return String(raw);
  }
</script>

<section class="proof">
  <div class="proof-head">
    <h2>Proof</h2>
    <span>{proofs.length}</span>
  </div>

  {#if latest}
    <div class="proof-grid">
      <div>
        <span>Latest</span>
        <strong>{value('Id') || value('_entity_id')}</strong>
      </div>
      <div>
        <span>Status</span>
        <strong>{value('Status')}</strong>
      </div>
      <div>
        <span>Reviewer</span>
        <strong>{value('ReviewerVerdict')}</strong>
      </div>
    </div>
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
    grid-template-columns: repeat(3, minmax(0, 1fr));
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

  @media (max-width: 720px) {
    .proof-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
