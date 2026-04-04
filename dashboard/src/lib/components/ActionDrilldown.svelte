<script lang="ts">
  import AuthzBadge from './AuthzBadge.svelte';

  let {
    action,
    resource,
    timestamp,
    allowed,
    deniedResource = null,
    denialReason = null,
    policyText = null,
    expanded = false,
  }: {
    action: string;
    resource: string;
    timestamp: string;
    allowed: boolean;
    deniedResource?: string | null;
    denialReason?: string | null;
    policyText?: string | null;
    expanded?: boolean;
  } = $props();

  let policyVisible = $state(false);
</script>

{#if expanded}
  <div class="drilldown">
    <div class="row">
      <span class="field"><span class="label">ACTION:</span> {action}</span>
      <span class="field"><span class="label">RESOURCE:</span> {resource}</span>
    </div>
    <div class="row">
      <span class="field">
        <span class="label">VERDICT:</span>
        <AuthzBadge {allowed} />
      </span>
      <span class="field"><span class="label">TIME:</span> {timestamp}</span>
    </div>
    {#if !allowed && denialReason}
      <div class="row">
        <span class="field"><span class="label">REASON:</span> {denialReason}</span>
      </div>
    {/if}
    {#if !allowed && deniedResource}
      <div class="row">
        <span class="field"><span class="label">RESOURCE:</span> {deniedResource}</span>
      </div>
    {/if}
    {#if policyText}
      <div class="policy-toggle">
        <button class="policy-btn" onclick={() => (policyVisible = !policyVisible)}>
          {policyVisible ? '[-] HIDE POLICY' : '[+] VIEW POLICY'}
        </button>
      </div>
      {#if policyVisible}
        <pre class="policy-text">{policyText}</pre>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .drilldown {
    display: flex;
    flex-direction: column;
    gap: var(--space-xs);
    padding: var(--space-sm);
    background: var(--terminal-bg);
    border: 1px solid var(--terminal-border);
    border-radius: var(--radius-sm);
    font-family: var(--font-mono);
    font-size: var(--label);
    color: var(--terminal-text);
  }

  .row {
    display: flex;
    gap: var(--space-lg);
    flex-wrap: wrap;
  }

  .field {
    display: inline-flex;
    align-items: center;
    gap: var(--space-xs);
  }

  .label {
    color: var(--terminal-dim);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .policy-toggle {
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
    margin: 0;
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
</style>
