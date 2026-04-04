<script lang="ts">
  interface Decision {
    id: string;
    agent_id: string;
    action: string;
    resource_type: string;
    resource_id: string;
    denial_reason: string;
    created_at: string;
    status: string;
    generated_policy: string | null;
    decided_by: string | null;
    decided_at: string | null;
  }

  let { decision }: { decision: Decision } = $props();

  let policyVisible = $state(false);

  let formattedCreated = $derived.by(() => {
    const d = new Date(decision.created_at);
    if (isNaN(d.getTime())) return decision.created_at;
    return d.toLocaleString('en-US', {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  });

  let formattedDecided = $derived.by(() => {
    if (!decision.decided_at) return null;
    const d = new Date(decision.decided_at);
    if (isNaN(d.getTime())) return decision.decided_at;
    return d.toLocaleString('en-US', {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  });
</script>

<div class="decision-card">
  <div class="header">
    <span class="agent">{decision.agent_id}</span>
    <span class="status-badge">{decision.status}</span>
  </div>
  <div class="body">
    <div class="row">
      <span class="label">ACTION:</span>
      <span class="value">{decision.action}</span>
    </div>
    <div class="row">
      <span class="label">RESOURCE:</span>
      <span class="value">{decision.resource_type}/{decision.resource_id}</span>
    </div>
    <div class="row">
      <span class="label">REASON:</span>
      <span class="value">{decision.denial_reason}</span>
    </div>
    <div class="row">
      <span class="label">CREATED:</span>
      <span class="value">{formattedCreated}</span>
    </div>
    {#if decision.generated_policy}
      <div class="policy-section">
        <button class="policy-btn" onclick={() => (policyVisible = !policyVisible)}>
          {policyVisible ? '[-] POLICY' : '[+] POLICY'}
        </button>
        {#if policyVisible}
          <pre class="policy-text">{decision.generated_policy}</pre>
        {/if}
      </div>
    {/if}
    {#if decision.decided_by}
      <div class="decided">
        <div class="row">
          <span class="label">DECIDED BY:</span>
          <span class="value">{decision.decided_by}</span>
        </div>
        {#if formattedDecided}
          <div class="row">
            <span class="label">DECIDED AT:</span>
            <span class="value">{formattedDecided}</span>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .decision-card {
    background: var(--terminal-bg);
    border: 1px solid var(--terminal-border);
    border-left: 2px solid var(--status-warning);
    border-radius: var(--radius-md);
    padding: var(--space-md);
    display: flex;
    flex-direction: column;
    gap: var(--space-sm);
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .agent {
    font-family: var(--font-mono);
    font-size: var(--caption);
    font-weight: 700;
    color: var(--terminal-text);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .status-badge {
    font-family: var(--font-mono);
    font-size: var(--label);
    color: var(--status-warning);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-xs);
  }

  .row {
    display: flex;
    gap: var(--space-sm);
    align-items: baseline;
  }

  .label {
    font-family: var(--font-mono);
    font-size: var(--label);
    color: var(--terminal-dim);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    flex-shrink: 0;
  }

  .value {
    font-family: var(--font-mono);
    font-size: var(--caption);
    color: var(--terminal-text);
    word-break: break-all;
  }

  .policy-section {
    margin-top: var(--space-xs);
  }

  .policy-btn {
    background: none;
    border: none;
    color: var(--terminal-dim);
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    cursor: pointer;
    padding: 0;
    transition: color var(--duration-fast) var(--ease);
  }

  .policy-btn:hover {
    color: var(--terminal-text);
  }

  .policy-text {
    margin: var(--space-xs) 0 0 0;
    padding: var(--space-sm);
    background: var(--terminal-bg);
    border: 1px solid var(--terminal-border);
    border-radius: var(--radius-sm);
    font-family: var(--font-mono);
    font-size: var(--label);
    color: var(--terminal-text);
    white-space: pre-wrap;
    overflow-x: auto;
  }

  .decided {
    margin-top: var(--space-xs);
    padding-top: var(--space-xs);
    border-top: 1px solid var(--terminal-border);
    display: flex;
    flex-direction: column;
    gap: var(--space-xs);
  }
</style>
