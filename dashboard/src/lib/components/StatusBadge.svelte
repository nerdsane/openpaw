<script lang="ts">
  let { status }: { status: string } = $props();

  const statusColor = $derived.by(() => {
    switch (status) {
      case 'Completed': case 'Complete': case 'Resolved': case 'Done':
        return 'var(--status-success)';
      case 'Failed': case 'Cancelled': case 'Escalate':
        return 'var(--status-error)';
      case 'WaitingForApproval': case 'Reviewing': case 'Testing':
        return 'var(--status-warning)';
      case 'Thinking': case 'Executing': case 'InProgress':
      case 'Triaging': case 'Planning': case 'Running':
        return 'var(--status-active)';
      default:
        return 'var(--status-idle)';
    }
  });
</script>

<span class="status-badge">
  <span class="dot" style:background={statusColor}></span>
  <span class="label">{status}</span>
</span>

<style>
  .status-badge {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
  }

  .dot {
    width: 6px; height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .label {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.04em;
    text-transform: lowercase;
    color: var(--text-3);
  }
</style>
