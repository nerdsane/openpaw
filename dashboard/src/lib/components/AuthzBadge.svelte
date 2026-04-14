<script lang="ts">
  let { allowed, resource = null }: {
    allowed: boolean;
    resource?: string | null;
  } = $props();

  let label = $derived.by(() => {
    if (allowed) return '[ALLOWED]';
    if (resource) return `[DENIED: ${resource}]`;
    return '[DENIED]';
  });
</script>

<span class="authz-badge" class:allowed class:denied={!allowed}>
  {label}
</span>

<style>
  .authz-badge {
    display: inline-flex;
    align-items: center;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    line-height: 1;
  }

  .allowed {
    color: var(--accent);
  }

  .denied {
    color: var(--status-error);
    background: var(--authz-denied-bg);
    padding: 2px var(--sp-1);
    border-radius: var(--radius);
  }
</style>
