<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { slide, fade } from 'svelte/transition';
  import { page } from '$app/stores';
  import type { Agent, WorkCycle, EntityEvent, Skill } from '$lib/types';
  import { getEntity, queryEntities, fetchAgentHistory, fetchFileContent } from '$lib/api';
  import { connectSSE, disconnectSSE, events, type StateChangeEvent } from '$lib/sse';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import GatePipeline from '$lib/components/GatePipeline.svelte';
  import StateTransition from '$lib/components/StateTransition.svelte';

  let agentId = $derived($page.params.id);

  let agent = $state<Agent | null>(null);
  let soulName = $state<string | null>(null);
  let soulEntity = $state<Record<string, unknown> | null>(null);
  let soulContentExpanded = $state(false);
  let soulContent = $state<string | null>(null);
  let agentSkills = $state<Skill[]>([]);
  let expandedSkillId = $state<string | null>(null);
  let skillContentCache = $state<Record<string, string>>({});
  let workcycle = $state<WorkCycle | null>(null);
  let loaded = $state(false);
  let error = $state<string | null>(null);
  let taskExpanded = $state(false);

  let transitions = $state<Array<{ event: StateChangeEvent; snapshot?: Agent; timestamp?: string; fromStatus?: string; authz?: { allowed: boolean; denied_resource?: string } }>>([]);

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

  let totalEventCount = $derived(agent?._total_event_count ?? 0);
  let displayedEventCount = $derived(transitions.length);
  let rawEventCount = $derived(agent?._events?.length ?? 0);

  let isTerminal = $derived(
    agent ? ['Completed', 'Failed', 'Cancelled'].includes(agent.Status) : false
  );

  async function fetchAgent() {
    const data = await getEntity('Agents', agentId);
    agent = data as unknown as Agent;
  }

  onMount(async () => {
    try {
      await fetchAgent();

      // Fetch soul name and entity
      if (agent?.soul_id) {
        try {
          const soul = await getEntity('Souls', agent.soul_id);
          soulEntity = soul;
          soulName = (soul as { name?: string; Name?: string }).name ?? (soul as { Name?: string }).Name ?? null;
        } catch {
          // soul_id might be a name string (e.g. "SWE"), not an entity ID
          soulName = agent.soul_id;
        }
      }

      // Fetch skills relevant to this agent
      try {
        const allSkills = await queryEntities('Skills');
        // Show skills that match: global scope, or scope matching agent's project harness
        agentSkills = allSkills.filter((s: Record<string, unknown>) => {
          const scope = ((s as { scope?: string }).scope ?? '') as string;
          const filter = ((s as { agent_filter?: string }).agent_filter ?? '') as string;
          // Show all skills; if agent_filter is set, only show if it matches agent's soul
          if (filter && agent?.soul_id && !filter.includes(agent.soul_id) && !filter.includes(soulName ?? '')) {
            return false;
          }
          return true;
        }) as unknown as Skill[];
      } catch { /* skills may not exist */ }

      // Try to find linked work cycle
      try {
        const wcs = await queryEntities('WorkCycles', `planner_id eq '${agentId}'`, undefined, 1);
        if (wcs.length > 0) {
          workcycle = wcs[0] as unknown as WorkCycle;
        }
      } catch { /* ignore */ }

      // Populate transitions from historical events
      if (agent?._events && agent._events.length > 0) {
        // Filter out noisy Heartbeat events, show meaningful transitions
        const meaningful = agent._events.filter(
          (e: EntityEvent) => e.action !== 'Heartbeat'
        );
        transitions = meaningful
          .reverse()
          .map((e: EntityEvent) => ({
            event: {
              seq: 0,
              entity_type: 'Agent',
              entity_id: agentId,
              action: e.action,
              status: e.to_status,
              tenant: 'default',
            } as StateChangeEvent,
            snapshot: undefined,
            timestamp: e.timestamp,
            fromStatus: e.from_status,
          }));
      }
      // Fetch trajectory history for authorization context
      const history = await fetchAgentHistory(agentId, 'Agent', 200);

      // Merge authz data into transitions by matching action + timestamp proximity
      for (const t of transitions) {
        const eventAction = t.event.action;
        const eventTime = t.timestamp ? new Date(t.timestamp).getTime() : 0;

        // Find matching trajectory entry (same action, within 5 seconds)
        const match = history.find(h =>
          h.action === eventAction &&
          Math.abs(new Date(h.timestamp).getTime() - eventTime) < 5000
        );

        if (match) {
          t.authz = {
            allowed: !match.authz_denied,
            denied_resource: match.denied_resource ?? undefined,
          };
        }
      }
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
      ].slice(0, 500);
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

        {#if isTerminal && agent.result}
          <div class="context-section context-section--result">
            <span class="context-label">Result</span>
            <p class="context-result">{agent.result}</p>
          </div>
        {/if}

        {#if isTerminal && agent.error_message}
          <div class="context-section context-section--error">
            <span class="context-label">Error</span>
            <p class="context-error">{agent.error_message}</p>
          </div>
        {/if}

        {#if workcycle}
          <div class="context-section context-section--workcycle">
            <span class="context-label">Work Cycle</span>
            <a href="/entities/WorkCycle/{workcycle.Id}" class="workcycle-card">
              <span class="workcycle-card__task">{workcycle.task_summary || 'Untitled'}</span>
              <div class="workcycle-card__status">
                <StatusBadge status={workcycle.Status} />
              </div>
              <div class="workcycle-card__gates">
                <GatePipeline {workcycle} />
              </div>
              {#if workcycle.pr_url}
                <code class="workcycle-card__pr">{workcycle.pr_url}</code>
              {/if}
              {#if workcycle.plan_summary}
                <p class="workcycle-card__plan">{workcycle.plan_summary}</p>
              {/if}
            </a>
          </div>
        {/if}

        {#if soulEntity}
          {@const soulFileId = (soulEntity as Record<string, unknown>).ContentFileId ?? (soulEntity as Record<string, unknown>).content_file_id}
          <div class="context-section">
            <span class="context-label">Soul</span>
            <span class="context-value">{soulName ?? agent.soul_id}</span>
            {#if soulFileId}
              <button class="expand-btn" onclick={async () => {
                soulContentExpanded = !soulContentExpanded;
                if (soulContentExpanded && !soulContent) {
                  soulContent = await fetchFileContent(soulFileId as string);
                }
              }}>
                {soulContentExpanded ? 'Hide soul content' : 'View soul content'}
              </button>
              {#if soulContentExpanded && soulContent}
                <pre class="context-pre" transition:slide={{ duration: 150 }}>{soulContent}</pre>
              {/if}
            {/if}
          </div>
        {:else if agent.soul_id}
          <div class="context-section">
            <span class="context-label">Soul</span>
            <span class="context-value">{soulName ?? agent.soul_id}</span>
          </div>
        {/if}

        {#if agentSkills.length > 0}
          <div class="context-section">
            <span class="context-label">Skills ({agentSkills.length})</span>
            <div class="agent-skills">
              {#each agentSkills as skill (skill.Id)}
                <div class="agent-skill">
                  <div class="agent-skill__header">
                    <span class="agent-skill__name">{skill.name || skill.Name || skill.Id}</span>
                    {#if skill.scope}
                      <span class="agent-skill__scope">{skill.scope}</span>
                    {/if}
                  </div>
                  {#if skill.description || skill.Description}
                    <span class="agent-skill__desc">{skill.description || skill.Description}</span>
                  {/if}
                  {#if skill.content_file_id}
                    <button class="expand-btn" onclick={async () => {
                      if (expandedSkillId === skill.Id) { expandedSkillId = null; return; }
                      expandedSkillId = skill.Id;
                      if (!skillContentCache[skill.Id]) {
                        const content = await fetchFileContent(skill.content_file_id);
                        skillContentCache = { ...skillContentCache, [skill.Id]: content };
                      }
                    }}>
                      {expandedSkillId === skill.Id ? 'Hide' : 'View content'}
                    </button>
                    {#if expandedSkillId === skill.Id && skillContentCache[skill.Id]}
                      <pre class="context-pre" transition:slide={{ duration: 150 }}>{skillContentCache[skill.Id]}</pre>
                    {/if}
                  {/if}
                </div>
              {/each}
            </div>
          </div>
        {/if}

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
            {#if agent.user_message.length > 200}
              <p class="context-task">
                {taskExpanded ? agent.user_message : agent.user_message.slice(0, 200) + '...'}
              </p>
              <button class="expand-btn" onclick={() => taskExpanded = !taskExpanded}>
                {taskExpanded ? 'Show less' : 'Show full task'}
              </button>
            {:else}
              <p class="context-task">{agent.user_message}</p>
            {/if}
          </div>
        {/if}
      </aside>

      <!-- Right Panel: Event History -->
      <section class="desk-stream">
        <div class="stream-header">
          <h2 class="stream-title">Event History</h2>
          {#if totalEventCount > 0}
            <span class="stream-count">
              Showing {displayedEventCount} of {totalEventCount} total events (heartbeats filtered)
            </span>
          {:else if rawEventCount > 0}
            <span class="stream-count">
              {displayedEventCount} events (heartbeats filtered)
            </span>
          {/if}
        </div>

        {#if transitions.length === 0}
          <div class="stream-empty">
            <p class="stream-empty-text">Waiting for activity...</p>
          </div>
        {:else}
          <div class="stream-list">
            {#each transitions as t, i (t.timestamp ?? t.event.seq ?? i)}
              <StateTransition event={t.event} agentSnapshot={t.snapshot} timestamp={t.timestamp} fromStatus={t.fromStatus} authz={t.authz} />
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

  .context-section--result {
    background: var(--surface-raised);
    padding: var(--space-2);
    border-radius: var(--radius-md);
    border-left: 3px solid var(--status-success);
  }

  .context-section--error {
    background: var(--surface-raised);
    padding: var(--space-2);
    border-radius: var(--radius-md);
    border-left: 3px solid var(--status-error);
  }

  .context-section--workcycle {
    padding-bottom: var(--space-2);
    border-bottom: 1px solid var(--border);
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

  .context-result {
    font-size: var(--text-sm);
    color: var(--text-primary);
    line-height: 1.5;
    white-space: pre-wrap;
  }

  .context-error {
    font-size: var(--text-sm);
    color: var(--status-error);
    line-height: 1.5;
    white-space: pre-wrap;
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
    white-space: pre-wrap;
  }

  .expand-btn {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    padding: 2px 0;
    text-align: left;
    width: fit-content;
    transition: color var(--duration-fast) var(--ease);
  }

  .expand-btn:hover {
    color: var(--text-primary);
  }

  .workcycle-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: var(--space-1) var(--space-2);
    background: var(--surface-raised);
    border-radius: var(--radius-sm);
    text-decoration: none;
    color: inherit;
    transition: background var(--duration-fast) var(--ease);
  }

  .workcycle-card:hover {
    text-decoration: none;
    background: var(--surface-overlay);
  }

  .workcycle-card__task {
    font-size: var(--text-sm);
    color: var(--text-primary);
  }

  .workcycle-card__status {
    margin-top: 2px;
  }

  .workcycle-card__gates {
    margin-top: 2px;
  }

  .workcycle-card__pr {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    word-break: break-all;
  }

  .workcycle-card__plan {
    font-size: var(--text-xs);
    color: var(--text-secondary);
    line-height: 1.4;
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

  .stream-header {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .stream-title {
    font-family: var(--font-sans);
    font-size: var(--text-base);
    font-weight: 500;
    color: var(--text-secondary);
  }

  .stream-count {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
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

  .agent-skills {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .agent-skill {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: var(--space-1) var(--space-2);
    background: var(--surface-raised);
    border-radius: var(--radius-sm);
  }

  .agent-skill__header {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  .agent-skill__name {
    font-size: var(--text-xs);
    font-weight: 500;
    color: var(--text-primary);
  }

  .agent-skill__scope {
    font-family: var(--font-mono);
    font-size: 0.625rem;
    color: var(--text-secondary);
    background: var(--surface-overlay);
    padding: 1px 5px;
    border-radius: var(--radius-sm);
  }

  .agent-skill__desc {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
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
