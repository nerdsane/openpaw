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
    gap: var(--sp-1);
    padding: var(--sp-2);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-1);
  }

  .row {
    display: flex;
    gap: var(--sp-6);
    flex-wrap: wrap;
  }

  .field {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
  }

  .label {
    color: var(--text-3);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .policy-toggle {
    margin-top: var(--sp-1);
  }

  .policy-btn {
    background: none;
    border: none;
    color: var(--text-3);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    cursor: pointer;
    padding: 0;
    transition: color var(--duration) var(--ease);
  }

  .policy-btn:hover {
    color: var(--text-1);
  }

  .policy-text {
    margin: 0;
    padding: var(--sp-2);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-1);
    white-space: pre-wrap;
    overflow-x: auto;
  }
</style>
