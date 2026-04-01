<script lang="ts">
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import { page } from '$app/stores';
  import { getEntity } from '$lib/api';
  import StatusBadge from '$lib/components/StatusBadge.svelte';

  let entityType = $derived($page.params.type);
  let entityId = $derived($page.params.id);
  let entitySetName = $derived(entityType + 's');

  let entity = $state<Record<string, unknown> | null>(null);
  let loaded = $state(false);
  let error = $state<string | null>(null);

  // Track which long fields are expanded
  let expandedFields = $state<Set<string>>(new Set());

  function toggleExpand(key: string) {
    const next = new Set(expandedFields);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    expandedFields = next;
  }

  function isJsonLike(value: unknown): boolean {
    if (typeof value !== 'string') return false;
    const trimmed = value.trim();
    return (trimmed.startsWith('{') && trimmed.endsWith('}')) ||
           (trimmed.startsWith('[') && trimmed.endsWith(']'));
  }

  function formatJson(value: string): string {
    try {
      return JSON.stringify(JSON.parse(value), null, 2);
    } catch {
      return value;
    }
  }

  function isLongString(value: unknown): boolean {
    return typeof value === 'string' && value.length > 120;
  }

  onMount(async () => {
    try {
      entity = await getEntity(entitySetName, entityId);
    } catch {
      error = `Could not load ${entityType} ${entityId}`;
    }
    loaded = true;
  });

  let status = $derived((entity?.Status as string) ?? '');
  let fields = $derived.by((): Array<[string, unknown]> => {
    if (!entity) return [];
    // Skip odata metadata fields
    return Object.entries(entity).filter(([k]) => !k.startsWith('@odata') && !k.startsWith('odata'));
  });
</script>

<div class="entity-detail">
  <a href="/" class="entity-back">&larr; Floor</a>

  {#if !loaded}
    <div class="entity-empty" transition:fade={{ duration: 200 }}>
      <p class="entity-empty-text">Loading...</p>
    </div>
  {:else if error || !entity}
    <div class="entity-empty" transition:fade={{ duration: 200 }}>
      <p class="entity-empty-text">{error ?? 'Entity not found'}</p>
    </div>
  {:else}
    <header class="entity-header">
      <h1>
        <code class="entity-type-label">{entityType}</code>
        <code class="entity-id-label">{entityId.slice(0, 12)}</code>
      </h1>
      {#if status}
        <StatusBadge {status} />
      {/if}
    </header>

    <div class="field-table">
      {#each fields as [key, value] (key)}
        <div class="field-row">
          <span class="field-key">{key}</span>
          <div class="field-value">
            {#if typeof value === 'object' && value !== null}
              <pre class="field-json">{JSON.stringify(value, null, 2)}</pre>
            {:else if isJsonLike(value)}
              <pre class="field-json">{formatJson(value as string)}</pre>
            {:else if isLongString(value) && !expandedFields.has(key)}
              <span class="field-text">{(value as string).slice(0, 120)}...</span>
              <button class="field-expand" onclick={() => toggleExpand(key)}>show more</button>
            {:else if isLongString(value) && expandedFields.has(key)}
              <span class="field-text">{value}</span>
              <button class="field-expand" onclick={() => toggleExpand(key)}>show less</button>
            {:else if value === null || value === undefined}
              <span class="field-null">--</span>
            {:else if typeof value === 'boolean'}
              <span class="field-bool">{value ? 'true' : 'false'}</span>
            {:else}
              <span class="field-text">{value}</span>
            {/if}
          </div>
        </div>
      {/each}
    </div>

    <div class="entity-history">
      <h2>History</h2>
      <p class="entity-history-text">Entity history coming soon</p>
    </div>
  {/if}
</div>

<style>
  .entity-detail {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .entity-back {
    font-size: var(--text-sm);
    color: var(--text-secondary);
    text-decoration: none;
    width: fit-content;
  }

  .entity-back:hover {
    color: var(--text-primary);
  }

  .entity-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-8) 0;
  }

  .entity-empty-text {
    color: var(--text-tertiary);
    font-size: var(--text-sm);
  }

  .entity-header {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex-wrap: wrap;
  }

  .entity-header h1 {
    display: flex;
    align-items: baseline;
    gap: var(--space-1);
  }

  .entity-type-label {
    font-family: var(--font-mono);
    font-size: var(--text-lg);
    color: var(--text-secondary);
  }

  .entity-id-label {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text-tertiary);
  }

  .field-table {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .field-row {
    display: flex;
    gap: var(--space-3);
    padding: var(--space-1) 0;
    border-bottom: 1px solid var(--border);
    align-items: flex-start;
  }

  .field-key {
    width: 180px;
    flex-shrink: 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }

  .field-value {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field-text {
    font-size: var(--text-sm);
    color: var(--text-primary);
    word-break: break-word;
  }

  .field-null {
    font-size: var(--text-sm);
    color: var(--text-tertiary);
  }

  .field-bool {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-primary);
  }

  .field-json {
    font-size: var(--text-xs);
    max-height: 200px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .field-expand {
    font-size: var(--text-xs);
    color: var(--text-secondary);
    padding: 0;
    text-align: left;
    width: fit-content;
  }

  .field-expand:hover {
    color: var(--text-primary);
  }

  .entity-history {
    margin-top: var(--space-4);
    padding-top: var(--space-3);
    border-top: 1px solid var(--border);
  }

  .entity-history h2 {
    font-size: var(--text-lg);
    margin-bottom: var(--space-1);
  }

  .entity-history-text {
    color: var(--text-tertiary);
    font-size: var(--text-sm);
  }
</style>
