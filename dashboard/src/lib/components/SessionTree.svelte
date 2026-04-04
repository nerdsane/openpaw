<script lang="ts">
  let { parentId = null, currentId, childIds }: {
    parentId?: string | null;
    currentId: string;
    childIds: string[];
  } = $props();

  let shortCurrent = $derived(currentId ?? '');
  let shortParent = $derived(parentId ?? '');
  let hasTree = $derived(!!parentId || childIds.length > 0);
</script>

{#if hasTree}
  <nav class="session-tree" aria-label="Session hierarchy">
    {#if parentId}
      <a href="/sessions/{parentId}" class="node parent">{shortParent}</a>
      <span class="separator">&gt;</span>
    {/if}
    <span class="node current">{shortCurrent}</span>
    {#if childIds.length > 0}
      <span class="separator">&gt;</span>
      <span class="children">
        {#each childIds as childId, i}
          {#if i > 0}<span class="comma">,</span>{/if}
          <a href="/sessions/{childId}" class="node child">{childId}</a>
        {/each}
      </span>
    {/if}
  </nav>
{/if}

<style>
  .session-tree {
    display: flex;
    align-items: center;
    gap: var(--space-xs);
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    flex-wrap: wrap;
  }

  .node {
    color: var(--terminal-dim);
    text-decoration: none;
  }

  a.node:hover {
    color: var(--terminal-text);
    text-decoration: underline;
  }

  .current {
    color: var(--terminal-text);
    font-weight: 700;
  }

  .separator {
    color: var(--terminal-dim);
  }

  .children {
    display: inline-flex;
    align-items: center;
    gap: var(--space-xs);
  }

  .comma {
    color: var(--terminal-dim);
  }
</style>
