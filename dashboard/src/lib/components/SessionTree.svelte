<script lang="ts">
  import { base } from '$app/paths';
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
      <a href="{base}/sessions/{parentId}" class="node parent">{shortParent}</a>
      <span class="separator">&gt;</span>
    {/if}
    <span class="node current">{shortCurrent}</span>
    {#if childIds.length > 0}
      <span class="separator">&gt;</span>
      <span class="children">
        {#each childIds as childId, i}
          {#if i > 0}<span class="comma">,</span>{/if}
          <a href="{base}/sessions/{childId}" class="node child">{childId}</a>
        {/each}
      </span>
    {/if}
  </nav>
{/if}

<style>
  .session-tree {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    flex-wrap: wrap;
  }

  .node {
    color: var(--text-3);
    text-decoration: none;
  }

  a.node:hover {
    color: var(--text-1);
    text-decoration: underline;
  }

  .current {
    color: var(--text-1);
    font-weight: 700;
  }

  .separator {
    color: var(--text-3);
  }

  .children {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-1);
  }

  .comma {
    color: var(--text-3);
  }
</style>
