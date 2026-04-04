<script lang="ts">
  interface Policy {
    policy_id: string;
    cedar_text: string;
    enabled: boolean;
    source: string;
    created_by: string;
    created_at: string;
  }

  let { policy }: { policy: Policy } = $props();

  let expanded = $state(false);

  let shortId = $derived(policy.policy_id);

  let highlightedCedar = $derived.by(() => {
    const text = policy.cedar_text;
    const lines = text.split('\n');
    return lines
      .map((line) => {
        // Comments
        if (line.trimStart().startsWith('//')) {
          return `<span class="hl-comment">${escapeHtml(line)}</span>`;
        }
        let escaped = escapeHtml(line);
        // Keywords: permit / forbid
        escaped = escaped.replace(
          /\b(permit)\b/g,
          '<span class="hl-permit">$1</span>'
        );
        escaped = escaped.replace(
          /\b(forbid)\b/g,
          '<span class="hl-forbid">$1</span>'
        );
        return escaped;
      })
      .join('\n');
  });

  function escapeHtml(str: string): string {
    return str
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }
</script>

<div class="policy-row">
  <button class="row-header" onclick={() => (expanded = !expanded)}>
    <span class="enabled-dot" class:on={policy.enabled}></span>
    <code class="policy-id">{shortId}</code>
    <span class="source-badge">{policy.source}</span>
    <span class="expand-toggle">{expanded ? '[-]' : '[+]'}</span>
  </button>
  {#if expanded}
    <div class="cedar-block">
      <pre class="cedar-text">{@html highlightedCedar}</pre>
    </div>
  {/if}
</div>

<style>
  .policy-row {
    border-bottom: 1px solid var(--terminal-border);
  }

  .row-header {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    width: 100%;
    padding: var(--space-sm) 0;
    background: none;
    border: none;
    cursor: pointer;
    font-family: var(--font-mono);
    font-size: var(--label);
    color: var(--terminal-text);
    text-align: left;
    transition: color var(--duration-fast) var(--ease);
  }

  .row-header:hover {
    color: var(--accent);
  }

  .enabled-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--status-idle);
    flex-shrink: 0;
  }

  .enabled-dot.on {
    background: var(--accent);
  }

  .policy-id {
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.02em;
    color: var(--terminal-text);
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .source-badge {
    font-family: var(--font-mono);
    font-size: var(--label);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--terminal-dim);
    padding: 1px 6px;
    border: 1px solid var(--terminal-border);
    border-radius: var(--radius-sm);
    flex-shrink: 0;
  }

  .expand-toggle {
    font-family: var(--font-mono);
    font-size: var(--label);
    color: var(--terminal-dim);
    flex-shrink: 0;
  }

  .cedar-block {
    padding: var(--space-sm) 0 var(--space-md) 0;
  }

  .cedar-text {
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
    line-height: 1.6;
  }

  .cedar-text :global(.hl-permit) {
    color: var(--accent);
    font-weight: 700;
  }

  .cedar-text :global(.hl-forbid) {
    color: var(--status-error);
    font-weight: 700;
  }

  .cedar-text :global(.hl-comment) {
    color: var(--terminal-dim);
  }
</style>
