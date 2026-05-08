<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import {
    createEntity,
    fetchAppViewManifest,
    postEntityAction,
    queryEntities
  } from '$lib/api';
  import { entityId } from '$lib/entity-format';
  import EntityBoard from '$lib/components/app-console/EntityBoard.svelte';
  import PatrolOverview from '$lib/components/app-console/PatrolOverview.svelte';
  import ProofViewer from '$lib/components/app-console/ProofViewer.svelte';
  import RelationTimeline from '$lib/components/app-console/RelationTimeline.svelte';
  import type { AppViewManifest } from '$lib/app-views/paw-patrol';

  let manifest = $state<AppViewManifest | null>(null);
  let rows = $state<Record<string, Record<string, unknown>[]>>({});
  let loading = $state(true);
  let actionBusy = $state(false);
  let error = $state('');
  let actionMessage = $state('');
  let workSource = $state('dashboard');
  let workRequestText = $state('');
  let workSubmitting = $state(false);

  const patrolActions = $derived(manifest?.actions.filter((action) => action.kind === 'patrol-run') ?? []);
  const workRequestAction = $derived(
    manifest?.actions.find((action) => action.kind === 'work-request') ?? null
  );

  function newestFirst(rows: Record<string, unknown>[]): Record<string, unknown>[] {
    return [...rows].sort((left, right) => entityId(right).localeCompare(entityId(left)));
  }

  async function load() {
    loading = true;
    error = '';
    const name = $page.params.name ?? '';
    manifest = await fetchAppViewManifest(name);
    if (!manifest) {
      rows = {};
      loading = false;
      return;
    }

    const loaded: Record<string, Record<string, unknown>[]> = {};
    await Promise.all(
      manifest.entitySets.map(async (set) => {
        const result = await queryEntities(set.name, undefined, set.orderby, set.top).catch(() => []);
        loaded[set.name] = newestFirst(result);
      })
    );
    rows = loaded;
    loading = false;
  }

  async function runPatrol(action: AppViewManifest['actions'][number]) {
    if (action.kind !== 'patrol-run') return;
    actionBusy = true;
    actionMessage = '';
    error = '';
    try {
      const run = await createEntity('PatrolRuns');
      const id = String(run.Id ?? run._entity_id ?? '');
      if (!id) throw new Error('PatrolRun create response did not include an id');
      await postEntityAction('PatrolRuns', id, 'Configure', {
        patrol_kind: action.patrolKind,
        summary: `${action.label} from dashboard`,
        requested_by: 'dashboard',
        required_capabilities: action.requiredCapabilities
      });
      await postEntityAction('PatrolRuns', id, 'Start', {});
      actionMessage = `Started PatrolRun ${id}`;
      await load();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Patrol action failed';
    } finally {
      actionBusy = false;
    }
  }

  async function submitWorkRequest(action: AppViewManifest['actions'][number]) {
    if (action.kind !== 'work-request') return;
    const requestText = workRequestText.trim();
    if (!requestText) {
      error = 'Work request text is required';
      return;
    }

    workSubmitting = true;
    actionMessage = '';
    error = '';
    try {
      const request = await createEntity('WorkRequests');
      const id = String(request.Id ?? request._entity_id ?? '');
      if (!id) throw new Error('WorkRequest create response did not include an id');
      await postEntityAction('WorkRequests', id, 'Submit', {
        source: workSource.trim() || action.source,
        request_text: requestText,
        requester_id: 'dashboard'
      });
      actionMessage = `Submitted WorkRequest ${id}`;
      workRequestText = '';
      await load();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Work request submission failed';
    } finally {
      workSubmitting = false;
    }
  }

  onMount(load);
</script>

<div class="app-console">
  {#if loading}
    <div class="empty">Loading app console...</div>
  {:else if !manifest}
    <div class="empty">No generic console manifest is available for this app yet.</div>
  {:else}
    <header class="console-head">
      <div>
        <p class="eyebrow">{manifest.name}</p>
        <h1>{manifest.title}</h1>
        <p>{manifest.summary}</p>
      </div>
      <div class="actions">
        {#each patrolActions as action}
          <button type="button" disabled={actionBusy} onclick={() => runPatrol(action)}>
            {actionBusy ? 'Running...' : action.label}
          </button>
        {/each}
      </div>
    </header>

    {#if error}
      <div class="notice notice-error">{error}</div>
    {/if}
    {#if actionMessage}
      <div class="notice">{actionMessage}</div>
    {/if}

    {#if workRequestAction}
      <section class="work-intake">
        <div>
          <p class="eyebrow">Work Intake</p>
          <h2>{workRequestAction.label}</h2>
        </div>
        <form onsubmit={(event) => { event.preventDefault(); void submitWorkRequest(workRequestAction); }}>
          <label>
            <span>Source</span>
            <input bind:value={workSource} />
          </label>
          <label class="request-text">
            <span>Request</span>
            <textarea bind:value={workRequestText} rows="4"></textarea>
          </label>
          <button type="submit" disabled={workSubmitting}>
            {workSubmitting ? 'Submitting...' : workRequestAction.label}
          </button>
        </form>
      </section>
    {/if}

    <RelationTimeline links={manifest.timeline} />
    <PatrolOverview {rows} />
    <ProofViewer proofs={rows[manifest.proofEntitySet] ?? []} />

    <div class="boards">
      {#each manifest.entitySets as set}
        <EntityBoard
          title={set.label}
          entitySet={set.name}
          rows={rows[set.name] ?? []}
          columns={set.columns}
        />
      {/each}
    </div>
  {/if}
</div>

<style>
  .app-console {
    width: min(1440px, 100%);
    padding: var(--sp-6);
  }

  .console-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--sp-6);
    padding-bottom: var(--sp-4);
  }

  .eyebrow {
    margin: 0 0 var(--sp-1) 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  h1 {
    margin: 0 0 var(--sp-2) 0;
  }

  .console-head p:not(.eyebrow) {
    margin: 0;
    color: var(--text-2);
    max-width: 680px;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--sp-2);
  }

  button {
    min-height: 32px;
    border: 1px solid var(--border-strong);
    padding: 0 var(--sp-3);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-1);
    background: var(--surface);
  }

  button:hover:not(:disabled) {
    border-color: var(--accent);
  }

  button:disabled {
    cursor: default;
    color: var(--text-3);
  }

  .notice,
  .empty {
    border: 1px solid var(--border);
    padding: var(--sp-3);
    margin-bottom: var(--sp-3);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-2);
  }

  .notice-error {
    border-color: var(--authz-denied-border);
    background: var(--authz-denied-bg);
    color: var(--status-error);
  }

  .work-intake {
    display: grid;
    grid-template-columns: minmax(160px, 0.35fr) minmax(0, 1fr);
    gap: var(--sp-4);
    border-top: 1px solid var(--border);
    padding: var(--sp-4) 0;
  }

  .work-intake h2 {
    margin: 0;
    font-size: var(--text-base);
  }

  .work-intake form {
    display: grid;
    grid-template-columns: minmax(160px, 0.3fr) minmax(0, 1fr) auto;
    align-items: end;
    gap: var(--sp-2);
  }

  .work-intake label {
    display: grid;
    gap: var(--sp-1);
    min-width: 0;
  }

  .work-intake label span {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .work-intake input,
  .work-intake textarea {
    width: 100%;
    min-width: 0;
    border: 1px solid var(--border);
    padding: var(--sp-2);
    color: var(--text-1);
    background: var(--surface);
    font: inherit;
  }

  .work-intake textarea {
    resize: vertical;
    min-height: 84px;
  }

  .boards {
    display: grid;
    grid-template-columns: 1fr;
  }

  @media (max-width: 720px) {
    .app-console {
      padding: var(--sp-4);
    }

    .console-head {
      display: block;
    }

    .actions {
      justify-content: flex-start;
      margin-top: var(--sp-3);
    }

    .work-intake,
    .work-intake form {
      grid-template-columns: 1fr;
    }
  }
</style>
