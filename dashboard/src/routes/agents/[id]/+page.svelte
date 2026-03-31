<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { slide, fade } from 'svelte/transition';
  import { page } from '$app/stores';
  import type { Agent, WorkCycle } from '$lib/types';
  import { getEntity, queryEntities } from '$lib/api';
  import { connectSSE, disconnectSSE, events, type StateChangeEvent } from '$lib/sse';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import GatePipeline from '$lib/components/GatePipeline.svelte';
  import StateTransition from '$lib/components/StateTransition.svelte';

  let agentId = $derived($page.params.id);

  let agent = $state<Agent | null>(null);
  let soulName = $state<string | null>(null);
  let workcycle = $state<WorkCycle | null>(null);
  let loaded = $state(false);
  let error = $state<string | null>(null);

  let transitions = $state<Array<{ event: StateChangeEvent; snapshot?: Agent }>>([]);

  let childIds = $derived.by((): string[] => {
    if (!agent?.child_agent_ids) return [];
    try {
      return JSON.parse(agent.child_agent_ids);
    } catch {
      return [];
    }
  });

  let maxTurns = $derived(parseInt(agent?.max_turns || '0', 10) || 0);
  let turnProgress = $derived.by(() => {
    if (!agent || maxTurns === 0) return 0;
    return Math.min(100, (agent.turn_count / maxTurns) * 100);
  });

  let totalTokens = $derived((agent?.input_tokens ?? 0) + (agent?.output_tokens ?? 0));
  let costDisplay = $derived.by(() => {
    const c = parseFloat(agent?.cost_cents || '0');
    if (c === 0) return '--';
    return `$${(c / 100).toFixed(4)}`;
  });

  async function fetchAgent() {
    const data = await getEntity('Agents', agentId);
    agent = data as unknown as Agent;
  }

  onMount(async () => {
    try {
      await fetchAgent();

      // Fetch soul name
      if (agent?.soul_id) {
        try {
          const soul = await getEntity('Souls', agent.soul_id);
          soulName = (soul as { name?: string }).name ?? null;
        } catch {
          // soul_id might be a name string (e.g. "SWE"), not an entity ID
          soulName = agent.soul_id;
        }
      }

      // Try to find linked work cycle
      try {
        const wcs = await queryEntities('WorkCycles', `planner_id eq '${agentId}'`, undefined, 1);
        if (wcs.length > 0) {
          workcycle = wcs[0] as unknown as WorkCycle;
        }
      } catch { /* ignore */ }
    } catch {
      error = 'Could not load agent';
    }
    loaded = true;

    connectSSE('Agent', agentId);
  });

  onDestroy(() => {
    disconnectSSE();
  });

  // SSE reactivity
  let lastSeq = $state(0);
  $effect(() => {
    const evts = $events;
    if (evts.length === 0) return;
    const latest = evts[0];
    if (latest.seq <= lastSeq) return;
    lastSeq = latest.seq;

    // Re-fetch agent on any event
    fetchAgent().then(() => {
      transitions = [
        { event: latest, snapshot: agent ?? undefined },
        ...transitions,
      ].slice(0, 200);
    });
  });
</script>

<div class="desk">
  <a href="/" class="desk-back">&larr; Floor</a>

  {#if !loaded}
    <div class="desk-empty" transition:fade={{ duration: 200 }}>
      <p class="desk-empty-text">Loading...</p>
    </div>
  {:else if error || !agent}
    <div class="desk-empty" transition:fade={{ duration: 200 }}>
      <p class="desk-empty-text">{error ?? 'Agent not found'}</p>
    </div>
  {:else}
    <div class="desk-layout">
      <!-- Left Panel: Context -->
      <aside class="desk-context">
        <div class="context-header">
          <h1 class="context-name">{soulName ?? 'Agent'}</h1>
          <code class="context-id">{agent.Id?.slice(0, 12)}</code>
          <div class="context-status">
            <StatusBadge status={agent.Status} />
          </div>
        </div>

        <div class="context-section">
          <span class="context-label">Model</span>
          <span class="context-value">{agent.model || '--'}</span>
        </div>

        {#if agent.provider}
          <div class="context-section">
            <span class="context-label">Provider</span>
            <span class="context-value">{agent.provider}</span>
          </div>
        {/if}

        <div class="context-section">
          <span class="context-label">Turns</span>
          <div class="progress-bar">
            <div class="progress-fill" style:width="{turnProgress}%"></div>
          </div>
          <span class="context-detail">{agent.turn_count ?? 0}{maxTurns > 0 ? ` / ${maxTurns}` : ''}</span>
        </div>

        <div class="context-section">
          <span class="context-label">Tokens</span>
          <span class="context-value mono">{totalTokens.toLocaleString()}</span>
          <span class="context-detail">in: {(agent.input_tokens ?? 0).toLocaleString()} / out: {(agent.output_tokens ?? 0).toLocaleString()}</span>
        </div>

        <div class="context-section">
          <span class="context-label">Cost</span>
          <span class="context-value mono">{costDisplay}</span>
        </div>

        {#if workcycle}
          <div class="context-section">
            <span class="context-label">Work Cycle</span>
            <GatePipeline {workcycle} />
          </div>
        {/if}

        {#if agent.parent_agent_id}
          <div class="context-section">
            <span class="context-label">Parent</span>
            <a href="/agents/{agent.parent_agent_id}" class="context-link mono">
              {agent.parent_agent_id.slice(0, 8)}
            </a>
          </div>
        {/if}

        {#if childIds.length > 0}
          <div class="context-section">
            <span class="context-label">Children</span>
            <div class="context-children">
              {#each childIds as childId}
                <a href="/agents/{childId}" class="context-link mono">{childId.slice(0, 8)}</a>
              {/each}
            </div>
          </div>
        {/if}

        {#if agent.Status === 'WaitingForApproval' && agent.pending_tool_context}
          <div class="context-section context-section--approval">
            <span class="context-label">Pending Approval</span>
            <pre class="context-pre">{agent.pending_tool_context}</pre>
          </div>
        {/if}

        {#if agent.user_message}
          <div class="context-section">
            <span class="context-label">Task</span>
            <p class="context-task">{agent.user_message}</p>
          </div>
        {/if}
      </aside>

      <!-- Right Panel: Live Stream -->
      <section class="desk-stream">
        <h2 class="stream-title">Live Stream</h2>

        {#if transitions.length === 0}
          <div class="stream-empty">
            <p class="stream-empty-text">Waiting for activity...</p>
          </div>
        {:else}
          <div class="stream-list">
            {#each transitions as t (t.event.seq)}
              <StateTransition event={t.event} agentSnapshot={t.snapshot} />
            {/each}
          </div>
        {/if}
      </section>
    </div>
  {/if}
</div>

<style>
  .desk {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .desk-back {
    font-size: var(--text-sm);
    color: var(--text-secondary);
    text-decoration: none;
    width: fit-content;
  }

  .desk-back:hover {
    color: var(--text-primary);
  }

  .desk-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-8) 0;
  }

  .desk-empty-text {
    color: var(--text-tertiary);
    font-size: var(--text-sm);
  }

  .desk-layout {
    display: flex;
    gap: var(--space-4);
  }

  .desk-context {
    width: 320px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .context-header {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-bottom: var(--space-2);
    border-bottom: 1px solid var(--border);
  }

  .context-name {
    font-size: var(--text-xl);
  }

  .context-id {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }

  .context-status {
    margin-top: 4px;
  }

  .context-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .context-section--approval {
    background: var(--surface-raised);
    padding: var(--space-2);
    border-radius: var(--radius-md);
  }

  .context-label {
    font-size: 0.625rem;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .context-value {
    font-size: var(--text-sm);
    color: var(--text-primary);
  }

  .context-detail {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }

  .context-link {
    font-size: var(--text-xs);
    color: var(--text-secondary);
    text-decoration: none;
  }

  .context-link:hover {
    color: var(--text-primary);
  }

  .context-children {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .context-pre {
    font-size: var(--text-xs);
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 150px;
    overflow: auto;
  }

  .context-task {
    font-size: var(--text-sm);
    color: var(--text-secondary);
    line-height: 1.5;
  }

  .progress-bar {
    height: 4px;
    background: var(--surface-overlay);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--status-active);
    border-radius: 2px;
    transition: width var(--duration-base) var(--ease);
  }

  .desk-stream {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }

  .stream-title {
    font-family: var(--font-sans);
    font-size: var(--text-base);
    font-weight: 500;
    color: var(--text-secondary);
  }

  .stream-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-6) 0;
  }

  .stream-empty-text {
    color: var(--text-tertiary);
    font-size: var(--text-sm);
  }

  .stream-list {
    display: flex;
    flex-direction: column;
  }

  .mono {
    font-family: var(--font-mono);
  }

  @media (max-width: 800px) {
    .desk-layout {
      flex-direction: column;
    }

    .desk-context {
      width: 100%;
    }
  }
</style>
