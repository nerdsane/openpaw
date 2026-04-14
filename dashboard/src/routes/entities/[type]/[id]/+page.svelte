<script lang="ts">
  import { base } from '$app/paths';
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
    if (!entityId) {
      error = `Could not load ${entityType}`;
      loaded = true;
      return;
    }

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
    return Object.entries(entity).filter(([k]) => !k.startsWith('@odata') && !k.startsWith('odata'));
  });
</script>

<div class="entity-detail">
  <a href="{base}/" class="entity-back">&larr; FLOOR</a>

  {#if !loaded}
    <div class="entity-empty" transition:fade={{ duration: 200 }}>
      <span class="entity-loading">[LOADING...]</span>
    </div>
  {:else if error || !entity}
    <div class="entity-empty" transition:fade={{ duration: 200 }}>
      <span class="entity-loading">[ERROR: {error ?? 'Entity not found'}]</span>
    </div>
  {:else}
    <header class="entity-header">
      <div class="entity-title">
        <code class="entity-type-label">{entityType}</code>
        <code class="entity-id-label">{entityId}</code>
      </div>
      {#if status}
        <StatusBadge {status} />
      {/if}
    </header>

    <div class="field-table">
      {#each fields as [key, value] (key)}
        <div class="field-row">
          <span class="field-key">{key}</span>
          <div class="field-value">
            {#if isJsonLike(value)}
              <pre class="field-json">{formatJson(value as string)}</pre>
            {:else if isLongString(value) && !expandedFields.has(key)}
              <span class="field-text">{(value as string).slice(0, 120)}...</span>
              <button class="field-expand" onclick={() => toggleExpand(key)}>[+] MORE</button>
            {:else if isLongString(value) && expandedFields.has(key)}
              <span class="field-text">{value}</span>
              <button class="field-expand" onclick={() => toggleExpand(key)}>[-] LESS</button>
            {:else if value === null || value === undefined}
              <span class="field-null">--</span>
            {:else if typeof value === 'boolean'}
              <span class="field-bool">{value ? 'TRUE' : 'FALSE'}</span>
            {:else}
              <span class="field-text">{value}</span>
            {/if}
          </div>
        </div>
      {/each}
    </div>

    <div class="entity-history">
      <span class="history-label">HISTORY</span>
      <span class="history-text">Entity history coming soon</span>
    </div>
  {/if}
</div>

<style>
  .entity-detail {
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
  }

  .entity-back {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.08em;
    color: var(--text-2);
    text-decoration: none;
    width: fit-content;
  }

  .entity-back:hover {
    color: var(--text-1);
  }

  .entity-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--sp-8) 0;
  }

  .entity-loading {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    letter-spacing: 0.06em;
  }

  .entity-header {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    flex-wrap: wrap;
  }

  .entity-title {
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
  }

  .entity-type-label {
    font-family: var(--font-mono);
    font-size: var(--text-lg);
    color: var(--text-1);
    letter-spacing: -0.01em;
  }

  .entity-id-label {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .field-table {
    display: flex;
    flex-direction: column;
  }

  .field-row {
    display: flex;
    gap: var(--sp-6);
    padding: var(--sp-2) 0;
    border-bottom: 1px solid var(--border);
    align-items: flex-start;
  }

  .field-key {
    width: 180px;
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-2);
  }

  .field-value {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }

  .field-text {
    font-size: var(--text-sm);
    color: var(--text-1);
    word-break: break-word;
  }

  .field-null {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .field-bool {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-1);
    letter-spacing: 0.04em;
  }

  .field-json {
    font-size: var(--text-xs);
    max-height: 200px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-all;
    background: var(--surface);
    border: 1px solid var(--border);
    padding: var(--sp-2);
    border-radius: var(--radius);
  }

  .field-expand {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.06em;
    color: var(--text-3);
    padding: 0;
    text-align: left;
    width: fit-content;
  }

  .field-expand:hover {
    color: var(--text-1);
  }

  .entity-history {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin-top: var(--sp-8);
    padding-top: var(--sp-6);
    border-top: 1px solid var(--border);
  }

  .history-label {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.08em;
    color: var(--text-2);
  }

  .history-text {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }
</style>
