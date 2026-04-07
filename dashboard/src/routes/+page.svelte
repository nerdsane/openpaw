<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { SvelteFlow, Background, MiniMap, Controls } from '@xyflow/svelte';
  import '@xyflow/svelte/dist/style.css';

  import { canvasNodes, canvasEdges, buildCanvasGraph, zoomLevel } from '$lib/stores/canvas';
  import { connectSSE, disconnectSSE, events } from '$lib/sse';
  import { refreshSession } from '$lib/stores/sessions';
  import { updateAgentSessions } from '$lib/stores/canvas';
  import { fetchAgentSessions } from '$lib/stores/agents';
  import { queryEntities, queryTeams, queryAgentsForTeam } from '$lib/api';
  import type { Node, Edge, NodeTypes } from '@xyflow/svelte';
  import type { Team, Agent, Session, Soul, Skill, Harness, WorkCycle } from '$lib/types';
  import { MOCK_ENTITIES, entitiesOfType } from '$lib/mock-data';
  import { parsePendingToolCalls, formatToolInput, computeMetrics } from '$lib/parse';
  import StatusBadge from '$lib/components/StatusBadge.svelte';

  import ProjectFrame from '$lib/components/canvas/ProjectFrame.svelte';
  import AgentNode from '$lib/components/canvas/AgentNode.svelte';
  import PlatformCluster from '$lib/components/canvas/PlatformCluster.svelte';
  import SoulNode from '$lib/components/canvas/SoulNode.svelte';

  const nodeTypes: NodeTypes = {
    project: ProjectFrame as any,
    agent: AgentNode as any,
    platform: PlatformCluster as any,
    soul: SoulNode as any,
  };

  let loaded = $state(false);
  let error = $state<string | null>(null);
  let usingMock = $state(false);
  let nodes = $state<Node[]>([]);
  let edges = $state<Edge[]>([]);
  let selectedNode = $state<Node | null>(null);

  function handleNodeClick({ node }: { node: Node; event: MouseEvent | TouchEvent }) {
    selectedNode = selectedNode?.id === node.id ? null : node;
  }

  function handlePaneClick() {
    selectedNode = null;
  }

  // Derive detail panel content from selected node
  let panelAgent = $derived.by(() => {
    if (!selectedNode || selectedNode.data?.type !== 'agent') return null;
    return selectedNode.data as any;
  });

  let panelSessions = $derived(panelAgent?.sessions ?? []);
  let panelActiveSession = $derived(
    panelSessions.find((s: any) => !['Completed', 'Failed', 'Cancelled'].includes(s.Status))
  );
  let panelEvents = $derived((panelActiveSession as any)?._events ?? []);
  let panelToolCalls = $derived.by(() => {
    const calls: Array<{ name: string; args: string; timestamp: string; tokens: number }> = [];
    for (const e of panelEvents) {
      if (e.action === 'ProcessToolCalls' && e.params?.pending_tool_calls) {
        const tcs = parsePendingToolCalls(e.params.pending_tool_calls as string);
        for (const tc of tcs) {
          calls.push({
            name: tc.name,
            args: formatToolInput(tc.name, tc.input),
            timestamp: e.timestamp ?? '',
            tokens: parseInt(String(e.params?.input_tokens ?? '0'), 10),
          });
        }
      }
    }
    return calls;
  });

  /** Convert mock entities to the shapes buildCanvasGraph expects */
  function loadFromMock() {
    const teams = entitiesOfType('Team').map(e => ({ ...e.fields, Id: e.Id, Status: e.Status }) as unknown as Team);
    const souls = entitiesOfType('Soul').map(e => ({ ...e.fields, Id: e.Id, Status: e.Status }) as unknown as Soul);
    const skills = entitiesOfType('Skill').map(e => ({ ...e.fields, Id: e.Id, Status: e.Status }) as unknown as Skill);
    const sessions = entitiesOfType('Session').map(e => ({ ...e.fields, Id: e.Id, Status: e.Status, _events: e._events, _sequence_nr: e._sequence_nr }) as unknown as Session);
    const harnesses = entitiesOfType('ProjectHarness').map(e => ({ ...e.fields, Id: e.Id, Status: e.Status }) as unknown as Harness);
    const workCycles = entitiesOfType('WorkCycle').map(e => ({ ...e.fields, Id: e.Id, Status: e.Status, _events: e._events }) as unknown as WorkCycle);
    const agents = entitiesOfType('Agent').map(e => ({ ...e.fields, Id: e.Id, Status: e.Status }) as unknown as Agent);

    const agentsMap: Record<string, Agent[]> = {};
    for (const t of teams) {
      agentsMap[t.Id] = agents.filter(a => (a as any).team_id === t.Id);
    }

    return { teams, agents: agentsMap, sessions, souls, skills, harnesses, workCycles };
  }

  onMount(async () => {
    try {
      // Try real API first
      const [teamsRaw, sessionsRaw, soulsRaw, skillsRaw, harnessesRaw, workCyclesRaw] = await Promise.all([
        queryTeams().catch(() => []),
        queryEntities('Sessions', undefined, 'Id desc', 100).catch(() => []),
        queryEntities('Souls').catch(() => []),
        queryEntities('Skills').catch(() => []),
        queryEntities('Harnesses').catch(() => queryEntities('ProjectHarnesses').catch(() => [])),
        queryEntities('WorkCycles').catch(() => []),
      ]);

      const teams = teamsRaw as unknown as Team[];
      const sessions = (sessionsRaw as unknown as Session[]).filter(s => s.agent_id || s.soul_id);
      const souls = soulsRaw as unknown as Soul[];
      const skills = skillsRaw as unknown as Skill[];
      const harnesses = harnessesRaw as unknown as Harness[];
      const workCycles = workCyclesRaw as unknown as WorkCycle[];

      // Use real data only if there's actual project structure (teams + agents)
      const hasData = teams.length > 0;

      if (hasData) {
        const agentsMap: Record<string, Agent[]> = {};
        await Promise.all(teams.map(async (team) => {
          if (!team.Id) return;
          try {
            const teamAgents = await queryAgentsForTeam(team.Id);
            agentsMap[team.Id] = teamAgents as unknown as Agent[];
          } catch {}
        }));
        buildCanvasGraph({ teams, agents: agentsMap, sessions, souls, skills, harnesses, workCycles });
      } else {
        // Fall back to mock data for UI development
        usingMock = true;
        buildCanvasGraph(loadFromMock());
      }

      canvasNodes.subscribe(v => { nodes = v; });
      canvasEdges.subscribe(v => { edges = v; });

      loaded = true;
      if (!usingMock) connectSSE();
    } catch (e) {
      // API completely unreachable — use mock
      console.warn('API unreachable, using mock data:', e);
      usingMock = true;
      buildCanvasGraph(loadFromMock());
      canvasNodes.subscribe(v => { nodes = v; });
      canvasEdges.subscribe(v => { edges = v; });
      loaded = true;
    }
  });

  onDestroy(() => { disconnectSSE(); });

  // SSE live updates
  let lastSeq = $state(0);
  $effect(() => {
    if (usingMock) return;
    const evts = $events;
    if (evts.length === 0) return;
    const latest = evts[0];
    if (latest.seq <= lastSeq) return;
    lastSeq = latest.seq;
    if (latest.entity_type === 'Session') {
      refreshSession(latest.entity_id);
      if (latest.agent_id) {
        fetchAgentSessions(latest.agent_id).then(sessions => {
          updateAgentSessions(latest.agent_id!, sessions);
        });
      }
    }
  });
</script>

<div class="canvas-container">
  {#if loaded}
    {#if nodes.length > 0}
      {#if usingMock}
        <div class="mock-banner">MOCK DATA — backend not connected</div>
      {/if}
      <SvelteFlow
        {nodes}
        {edges}
        {nodeTypes}
        fitView
        minZoom={0.1}
        maxZoom={3}
        nodesDraggable={false}
        nodesConnectable={false}
        elementsSelectable={true}
        onnodeclick={handleNodeClick}
        onpaneclick={handlePaneClick}
      >
        <Background gap={20} />
        <MiniMap />
        <Controls />
      </SvelteFlow>

      <!-- Detail panel (slides in from right when a node is selected) -->
      {#if selectedNode && panelAgent}
        <div class="detail-panel">
          <button class="panel-close" onclick={() => selectedNode = null}>&times;</button>
          <div class="panel-header">
            <span class="panel-name">{panelAgent.agent?.name ?? 'Agent'}</span>
            <span class="panel-role">{panelAgent.agent?.role ?? ''}</span>
            <StatusBadge status={panelActiveSession?.Status ?? panelAgent.agent?.Status ?? 'Idle'} />
          </div>

          {#if panelAgent.agent?.model}
            <div class="panel-meta">{panelAgent.agent.provider}/{panelAgent.agent.model}</div>
          {/if}

          <!-- Sessions -->
          <div class="panel-section">
            <span class="panel-label">Sessions ({panelSessions.length})</span>
            {#each panelSessions.slice(0, 5) as sess}
              <div class="panel-session">
                <StatusBadge status={sess.Status} />
                <span class="panel-session-id">{sess.Id}</span>
                <span class="panel-session-turns">{(sess as any).turn_count ?? (sess as any).fields?.turn_count ?? 0}t</span>
              </div>
            {/each}
          </div>

          <!-- Active session terminal -->
          {#if panelToolCalls.length > 0}
            <div class="panel-section">
              <span class="panel-label">Activity</span>
              <div class="panel-terminal">
                {#each panelToolCalls as tc}
                  <div class="panel-line">
                    <span class="panel-prompt">&gt;</span>
                    <span class="panel-cmd">{tc.name}</span>
                    <span class="panel-args">{tc.args}</span>
                  </div>
                {/each}
              </div>
            </div>
          {/if}

          <!-- Skills -->
          {#if panelAgent.skills?.length > 0}
            <div class="panel-section">
              <span class="panel-label">Skills</span>
              <div class="panel-tags">
                {#each panelAgent.skills as skill}
                  <span class="panel-tag">{skill.name || skill.Name}</span>
                {/each}
              </div>
            </div>
          {/if}

          <!-- Authz -->
          <div class="panel-section">
            <span class="panel-label">Authorization</span>
            <div class="panel-authz">
              <span class="panel-authz-ok">{panelAgent.authzAllowed} allowed</span>
              <span class="panel-authz-deny">{panelAgent.authzDenied} denied</span>
            </div>
          </div>

          {#if panelAgent.hasPendingDecision}
            <div class="panel-warning">Pending approval required</div>
          {/if}
        </div>
      {/if}
    {:else}
      <div class="canvas-empty">
        <span class="empty-label">CANVAS</span>
        <span class="empty-text">No entities found.</span>
      </div>
    {/if}
  {:else}
    <div class="canvas-empty">
      <span class="empty-text">Loading...</span>
    </div>
  {/if}
</div>

<style>
  .canvas-container {
    width: 100%;
    height: 100vh;
    position: relative;
  }

  .mock-banner {
    position: absolute;
    top: 8px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 10;
    font-family: var(--font-mono);
    font-size: var(--text-xs, 11px);
    color: var(--status-warning, #eab308);
    background: var(--surface, #141414);
    border: 1px solid var(--status-warning, #eab308);
    padding: 2px 12px;
    border-radius: var(--radius, 4px);
    letter-spacing: 0.04em;
  }

  .canvas-container :global(.svelte-flow) {
    background: var(--bg, #0a0a0a) !important;
  }

  .canvas-container :global(.svelte-flow__minimap) {
    background: var(--surface, #141414);
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    border-radius: var(--radius, 4px);
  }

  .canvas-container :global(.svelte-flow__controls) {
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    border-radius: var(--radius, 4px);
    overflow: hidden;
  }

  .canvas-container :global(.svelte-flow__controls-button) {
    background: var(--surface, #141414);
    color: var(--text-2, #a0a0a0);
    border: none;
    border-bottom: 1px solid var(--border, rgba(255,255,255,0.08));
  }

  .canvas-container :global(.svelte-flow__controls-button:hover) {
    background: var(--surface-raised, #1c1c1c);
  }

  .canvas-container :global(.svelte-flow__controls-button svg) {
    fill: var(--text-2, #a0a0a0);
  }

  .canvas-container :global(.svelte-flow__attribution) {
    display: none;
  }

  .canvas-empty {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
  }

  .empty-label {
    font-family: var(--font-mono);
    font-size: 20px;
    letter-spacing: 0.08em;
    color: var(--text-3, #666);
    opacity: 0.3;
  }

  .empty-text {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-3, #666);
  }

  /* ── Detail Panel ── */
  .detail-panel {
    position: absolute;
    top: 0;
    right: 0;
    width: 360px;
    height: 100%;
    background: var(--bg, #0a0a0a);
    border-left: 1px solid var(--border, rgba(255,255,255,0.08));
    z-index: 20;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    font-family: var(--font-mono, monospace);
  }

  .panel-close {
    position: absolute;
    top: 8px;
    right: 12px;
    font-size: 18px;
    color: var(--text-3, #666);
    cursor: pointer;
    background: none;
    border: none;
    line-height: 1;
  }
  .panel-close:hover { color: var(--text-1, #eee); }

  .panel-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-right: 24px;
  }

  .panel-name {
    font-size: 15px;
    font-weight: 600;
    color: var(--accent, #00DC82);
  }

  .panel-role {
    font-size: 11px;
    color: var(--text-3, #666);
  }

  .panel-meta {
    font-size: 10px;
    color: var(--text-3, #666);
  }

  .panel-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .panel-label {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-3, #666);
  }

  .panel-session {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
  }

  .panel-session-id {
    color: var(--text-2, #a0a0a0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }

  .panel-session-turns {
    color: var(--text-3, #666);
    flex-shrink: 0;
  }

  /* Terminal in panel */
  .panel-terminal {
    background: var(--surface, #141414);
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    border-radius: var(--radius, 4px);
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 300px;
    overflow-y: auto;
  }

  .panel-line {
    display: flex;
    gap: 4px;
    font-size: 10px;
    white-space: nowrap;
    overflow: hidden;
  }

  .panel-prompt { color: var(--accent, #00DC82); flex-shrink: 0; }
  .panel-cmd { color: var(--accent, #00DC82); flex-shrink: 0; }
  .panel-args { color: var(--text-2, #a0a0a0); overflow: hidden; text-overflow: ellipsis; }

  /* Tags */
  .panel-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .panel-tag {
    font-size: 10px;
    color: var(--text-2, #a0a0a0);
    background: var(--surface, #141414);
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    padding: 1px 6px;
    border-radius: 3px;
  }

  /* Authz */
  .panel-authz {
    display: flex;
    gap: 12px;
    font-size: 11px;
  }

  .panel-authz-ok { color: var(--accent, #00DC82); }
  .panel-authz-deny { color: var(--status-error, #f87171); }

  .panel-warning {
    font-size: 10px;
    color: var(--status-warning, #eab308);
    background: rgba(234, 179, 8, 0.08);
    border: 1px solid rgba(234, 179, 8, 0.2);
    padding: 6px 10px;
    border-radius: var(--radius, 4px);
  }

  @media (max-width: 768px) {
    .detail-panel { width: 100%; }
  }
</style>
