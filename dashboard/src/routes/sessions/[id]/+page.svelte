<script lang="ts">
  import { base } from '$app/paths';
  import { onMount, onDestroy } from 'svelte';
  import { slide, fade } from 'svelte/transition';
  import { page } from '$app/stores';
  import type { Session, WorkCycle, EntityEvent, Skill } from '$lib/types';
  import { getEntity, queryEntities, fetchSessionHistory, fetchFileContent, fetchPolicies, type PolicyEntry } from '$lib/api';
  import { connectSSE, disconnectSSE, events, type StateChangeEvent } from '$lib/sse';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import AuthzBadge from '$lib/components/AuthzBadge.svelte';
  import TerminalChrome from '$lib/components/TerminalChrome.svelte';
  import SessionTree from '$lib/components/SessionTree.svelte';
  import GatePipeline from '$lib/components/GatePipeline.svelte';
  import TerminalFeed from '$lib/components/TerminalFeed.svelte';
  import { eventsToTurns, computeMetrics, type SessionTurn, type SessionMetrics } from '$lib/parse';
  import { resolveAgentName } from '$lib/stores/agents';

  let sessionId = $derived($page.params.id);

  let session = $state<Session | null>(null);
  let agentName = $state<string>('Session');
  let soulName = $state<string | null>(null);
  let soulEntity = $state<Record<string, unknown> | null>(null);
  let soulContentExpanded = $state(false);
  let soulContent = $state<string | null>(null);
  let sessionSkills = $state<Skill[]>([]);
  let expandedSkillId = $state<string | null>(null);
  let skillContentCache = $state<Record<string, string>>({});
  let workcycle = $state<WorkCycle | null>(null);
  let loaded = $state(false);
  let error = $state<string | null>(null);
  let taskExpanded = $state(false);
  let identityExpanded = $state(false);

  let transitions = $state<Array<{ event: StateChangeEvent; snapshot?: Session; timestamp?: string; fromStatus?: string; authz?: { allowed: boolean; denied_resource?: string } }>>([]);
  let turns = $state<SessionTurn[]>([]);
  let authzAllowed = $state(0);
  let authzDenied = $state(0);
  let sessionPolicies = $state<PolicyEntry[]>([]);
  let metrics = $state<SessionMetrics>({ turns: 0, inputTokens: 0, outputTokens: 0, totalOutputTokens: 0 });

  let childIds = $derived.by((): string[] => {
    if (!session?.child_session_ids) return [];
    try { return JSON.parse(session.child_session_ids); } catch { return []; }
  });

  let maxTurns = $derived(parseInt(session?.max_turns || '0', 10) || 0);
  let turnProgress = $derived.by(() => {
    if (!session || maxTurns === 0) return 0;
    return Math.min(100, (metrics.turns / maxTurns) * 100);
  });

  let isTerminal = $derived(
    session ? ['Completed', 'Failed', 'Cancelled'].includes(session.Status) : false
  );

  let isActive = $derived(
    session ? !['Completed', 'Failed', 'Cancelled'].includes(session.Status) : false
  );

  let stateTransitions = $derived.by(() => {
    if (!transitions.length) return [];
    return transitions
      .filter(t => t.timestamp)
      .map(t => ({
        action: t.event.action,
        from: t.fromStatus ?? '',
        to: t.event.status,
        timestamp: t.timestamp!,
      }));
  });

  async function fetchSession() {
    if (!sessionId) {
      throw new Error('Missing session id');
    }
    const data = await getEntity('Sessions', sessionId);
    session = data as unknown as Session;
  }

  onMount(async () => {
    if (!sessionId) {
      error = 'Missing session id';
      loaded = true;
      return;
    }

    try {
      await fetchSession();

      // Resolve agent name
      if (session) {
        agentName = await resolveAgentName(session);
      }

      if (session?.soul_id) {
        try {
          const soul = await getEntity('Souls', session.soul_id);
          soulEntity = soul;
          soulName = (soul as { name?: string; Name?: string }).name ?? (soul as { Name?: string }).Name ?? null;
        } catch { soulName = session.soul_id; }
      }

      try {
        const allSkills = await queryEntities('Skills');
        sessionSkills = allSkills.filter((s: Record<string, unknown>) => {
          const filter = ((s as { agent_filter?: string }).agent_filter ?? '') as string;
          if (filter && session?.soul_id && !filter.includes(session.soul_id) && !filter.includes(soulName ?? '')) return false;
          return true;
        }) as unknown as Skill[];
      } catch {}

      try {
        const wcs = await queryEntities('WorkCycles', `planner_id eq '${sessionId}'`, undefined, 1);
        if (wcs.length > 0) workcycle = wcs[0] as unknown as WorkCycle;
      } catch {}

      if (session?._events && session._events.length > 0) {
        const meaningful = session._events.filter((e: EntityEvent) => e.action !== 'Heartbeat');
        transitions = meaningful.reverse().map((e: EntityEvent) => ({
          event: { seq: 0, entity_type: 'Session', entity_id: sessionId, action: e.action, status: e.to_status, tenant: 'default' } as StateChangeEvent,
          snapshot: undefined,
          timestamp: e.timestamp,
          fromStatus: e.from_status,
        }));
      }
      if (session?._events && session._events.length > 0) {
        turns = eventsToTurns(session._events).reverse(); // latest first
        metrics = computeMetrics(session._events);
      }

      const history = await fetchSessionHistory(sessionId, 'Session', 200);

      // Count authz stats
      authzAllowed = history.filter(h => !h.authz_denied).length;
      authzDenied = history.filter(h => h.authz_denied).length;

      for (const turn of turns) {
        const match = history.find(h =>
          h.action === 'ProcessToolCalls' &&
          Math.abs(new Date(h.timestamp).getTime() - new Date(turn.timestamp).getTime()) < 5000
        );
        if (match) {
          turn.authz = { allowed: !match.authz_denied, denied_resource: match.denied_resource ?? undefined };
        }
      }

      for (const t of transitions) {
        const eventAction = t.event.action;
        const eventTime = t.timestamp ? new Date(t.timestamp).getTime() : 0;
        const match = history.find(h => h.action === eventAction && Math.abs(new Date(h.timestamp).getTime() - eventTime) < 5000);
        if (match) {
          t.authz = { allowed: !match.authz_denied, denied_resource: match.denied_resource ?? undefined };
        }
      }
      // Load policies for authz drill-down
      try { sessionPolicies = await fetchPolicies(); } catch {}
    } catch { error = 'Could not load session'; }
    loaded = true;
    connectSSE('Session', sessionId);
  });

  onDestroy(() => { disconnectSSE(); });

  let lastSeq = $state(0);
  $effect(() => {
    const evts = $events;
    if (evts.length === 0) return;
    const latest = evts[0];
    if (latest.seq <= lastSeq) return;
    lastSeq = latest.seq;
    fetchSession().then(() => {
      transitions = [{ event: latest, snapshot: session ?? undefined }, ...transitions].slice(0, 500);
    });
  });
</script>

<div class="desk">
  <a href="{base}/" class="desk-back">&larr; FLOOR</a>

  {#if !loaded}
    <div class="desk-empty" transition:fade={{ duration: 200 }}>
      <span class="desk-loading">[LOADING...]</span>
    </div>
  {:else if error || !session}
    <div class="desk-empty" transition:fade={{ duration: 200 }}>
      <span class="desk-loading">[ERROR: {error ?? 'Session not found'}]</span>
    </div>
  {:else}
    <div class="desk-layout">
      <!-- Left Panel: Context -->
      <aside class="desk-context">
        <!-- Agent link -->
        <div class="context-section">
          <span class="context-label">AGENT</span>
          <a href="{base}/agents" class="agent-link">{agentName}</a>
          {#if session.model}
            <span class="context-meta">{session.model}</span>
          {/if}
        </div>

        <!-- Session tree -->
        <SessionTree
          parentId={session.parent_session_id || null}
          currentId={session.Id}
          childIds={childIds}
        />

        <!-- Status -->
        <div class="context-section">
          <span class="context-label">STATUS</span>
          <StatusBadge status={session.Status} />
        </div>

        {#if isTerminal && session.result}
          <div class="context-section context-section--result">
            <span class="context-label">RESULT</span>
            <p class="context-result">{session.result}</p>
          </div>
        {/if}

        {#if isTerminal && session.error_message}
          <div class="context-section context-section--error">
            <span class="context-label">ERROR</span>
            <p class="context-error">{session.error_message}</p>
          </div>
        {/if}

        <!-- Authz summary -->
        <div class="context-section">
          <span class="context-label">AUTHORIZATION</span>
          <div class="authz-summary">
            <span class="authz-stat authz-stat--ok">{authzAllowed} ALLOWED</span>
            {#if authzDenied > 0}
              <span class="authz-stat authz-stat--denied">{authzDenied} DENIED</span>
            {:else}
              <span class="authz-stat">{authzDenied} DENIED</span>
            {/if}
          </div>
        </div>

        <!-- Metrics (computed from events, not entity counters) -->
        <div class="context-section">
          <span class="context-label">METRICS</span>
          <div class="metrics-grid">
            <div class="metric">
              <span class="metric-label">TURNS</span>
              <span class="metric-value">{metrics.turns}{maxTurns > 0 ? ` / ${maxTurns}` : ''}</span>
              {#if maxTurns > 0}
                <div class="progress-bar"><div class="progress-fill" style:width="{turnProgress}%"></div></div>
              {/if}
            </div>
            <div class="metric">
              <span class="metric-label">CONTEXT TOKENS</span>
              <span class="metric-value">{metrics.inputTokens.toLocaleString()}</span>
              <span class="metric-detail">Current context window size</span>
            </div>
            <div class="metric">
              <span class="metric-label">OUTPUT TOKENS</span>
              <span class="metric-value">{metrics.totalOutputTokens.toLocaleString()}</span>
              <span class="metric-detail">Total generated across all turns</span>
            </div>
          </div>
        </div>

        <!-- Identity (collapsible) -->
        <div class="context-section">
          <button class="section-toggle" onclick={() => identityExpanded = !identityExpanded}>
            <span class="context-label">IDENTITY</span>
            <span class="toggle-icon">{identityExpanded ? '[-]' : '[+]'}</span>
          </button>
          {#if identityExpanded}
            <div class="identity-content" transition:slide={{ duration: 150 }}>
              {#if soulEntity}
                {@const soulFileId = (soulEntity as Record<string, unknown>).ContentFileId ?? (soulEntity as Record<string, unknown>).content_file_id}
                <div class="identity-item">
                  <span class="identity-key">SOUL</span>
                  <span class="identity-val">{soulName ?? session.soul_id}</span>
                  {#if soulFileId}
                    <button class="expand-btn" onclick={async () => { soulContentExpanded = !soulContentExpanded; if (soulContentExpanded && !soulContent) soulContent = await fetchFileContent(soulFileId as string); }}>
                      {soulContentExpanded ? '[-] HIDE' : '[+] VIEW'}
                    </button>
                    {#if soulContentExpanded && soulContent}
                      <pre class="context-pre" transition:slide={{ duration: 150 }}>{soulContent}</pre>
                    {/if}
                  {/if}
                </div>
              {/if}
              {#if sessionSkills.length > 0}
                <div class="identity-item">
                  <span class="identity-key">SKILLS ({sessionSkills.length})</span>
                  {#each sessionSkills as skill (skill.Id)}
                    <span class="skill-tag">{skill.name || skill.Name || skill.Id}</span>
                  {/each}
                </div>
              {/if}
              {#if session.provider}
                <div class="identity-item">
                  <span class="identity-key">PROVIDER</span>
                  <span class="identity-val">{session.provider}</span>
                </div>
              {/if}
            </div>
          {/if}
        </div>

        <!-- Work Cycle -->
        {#if workcycle}
          <div class="context-section">
            <span class="context-label">WORK CYCLE</span>
            <a href="{base}/entities/WorkCycle/{workcycle.Id}" class="workcycle-link">
              <span>{workcycle.task_summary || 'Untitled'}</span>
              <StatusBadge status={workcycle.Status} />
              <GatePipeline {workcycle} />
            </a>
          </div>
        {/if}

        <!-- Task -->
        {#if session.user_message}
          <div class="context-section">
            <span class="context-label">TASK</span>
            {#if session.user_message.length > 200}
              <p class="context-task">{taskExpanded ? session.user_message : session.user_message.slice(0, 200) + '...'}</p>
              <button class="expand-btn" onclick={() => taskExpanded = !taskExpanded}>{taskExpanded ? '[-] LESS' : '[+] FULL'}</button>
            {:else}
              <p class="context-task">{session.user_message}</p>
            {/if}
          </div>
        {/if}
      </aside>

      <!-- Right Panel: Terminal -->
      <section class="desk-stream">
        <TerminalChrome
          name={agentName}
          id={session.Id}
          status={session.Status}
          active={isActive}
        />
        <TerminalFeed
          {turns}
          userMessage={session.user_message}
          {stateTransitions}
          policies={sessionPolicies}
        />
      </section>
    </div>
  {/if}
</div>

<style>
  .desk {
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
  }

  .desk-back {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.08em;
    color: var(--text-3);
    text-decoration: none;
    width: fit-content;
  }

  .desk-back:hover { color: var(--text-1); }

  .desk-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--sp-8) 0;
  }

  .desk-loading {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    letter-spacing: 0.06em;
  }

  .desk-layout {
    display: flex;
    gap: var(--sp-8);
  }

  .desk-context {
    width: 320px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }

  .context-section {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }

  .context-section--result {
    background: var(--surface);
    padding: var(--sp-4);
    border-radius: var(--radius);
    border-left: 2px solid var(--status-success);
  }

  .context-section--error {
    background: var(--surface);
    padding: var(--sp-4);
    border-radius: var(--radius);
    border-left: 2px solid var(--status-error);
  }

  .context-label {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.08em;
    color: var(--text-3);
  }

  .agent-link {
    font-family: var(--font-mono);
    font-size: var(--text-base);
    font-weight: 500;
    color: var(--text-1);
    text-decoration: none;
  }

  .agent-link:hover { text-decoration: underline; }

  .context-meta {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .context-result {
    font-size: var(--text-sm);
    color: var(--text-1);
    line-height: 1.5;
    white-space: pre-wrap;
  }

  .context-error {
    font-size: var(--text-sm);
    color: var(--status-error);
    line-height: 1.5;
    white-space: pre-wrap;
  }

  .context-task {
    font-size: var(--text-sm);
    color: var(--text-2);
    line-height: 1.5;
    white-space: pre-wrap;
  }

  .authz-summary {
    display: flex;
    gap: var(--sp-4);
  }

  .authz-stat {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.06em;
    color: var(--text-3);
  }

  .authz-stat--ok { color: var(--accent); }
  .authz-stat--denied { color: var(--status-error); }

  .metrics-grid {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .metric {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .metric-label {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.08em;
    color: var(--text-3);
  }

  .metric-value {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-1);
  }

  .metric-detail {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .progress-bar {
    height: 3px;
    background: var(--border);
    border-radius: 0;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent);
    transition: width var(--duration) var(--ease);
  }

  .section-toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 0;
    cursor: pointer;
  }

  .toggle-icon {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .identity-content {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding-top: var(--sp-1);
  }

  .identity-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .identity-key {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.06em;
    color: var(--text-3);
  }

  .identity-val {
    font-size: var(--text-xs);
    color: var(--text-1);
  }

  .skill-tag {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-2);
    background: var(--surface);
    border: 1px solid var(--border);
    padding: 1px var(--sp-2);
    border-radius: var(--radius);
    display: inline-block;
  }

  .expand-btn {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.06em;
    color: var(--text-3);
    padding: 2px 0;
    text-align: left;
    width: fit-content;
  }

  .expand-btn:hover { color: var(--text-1); }

  .context-pre {
    font-size: var(--text-xs);
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 150px;
    overflow: auto;
    background: var(--surface);
    border: 1px solid var(--border);
    padding: var(--sp-2);
    border-radius: var(--radius);
  }

  .workcycle-link {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    padding: var(--sp-2) var(--sp-3);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    text-decoration: none;
    color: var(--text-1);
    font-size: var(--text-sm);
  }

  .workcycle-link:hover { border-color: var(--border-strong); }

  .desk-stream {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  @media (max-width: 800px) {
    .desk-layout { flex-direction: column; gap: var(--sp-4); }
    .desk-context { width: 100%; }
  }

  @media (max-width: 640px) {
    .desk { gap: var(--sp-3); }
    .desk-layout { gap: var(--sp-3); }
    .context-section--result,
    .context-section--error { padding: var(--sp-3); }
    .context-pre { max-height: 200px; }
  }
</style>
