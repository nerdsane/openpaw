<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { base } from '$app/paths';
  import { SvelteFlow, Background, MiniMap, Controls } from '@xyflow/svelte';
  import '@xyflow/svelte/dist/style.css';

  import { canvasNodes, canvasEdges, buildCanvasGraph, zoomLevel } from '$lib/stores/canvas';
  import { connectSSE, disconnectSSE, events } from '$lib/sse';
  import { refreshSession } from '$lib/stores/sessions';
  import { updateAgentSessions } from '$lib/stores/canvas';
  import { fetchAgentSessions } from '$lib/stores/agents';
  import { queryEntities, queryTeams, queryAgentsForTeam } from '$lib/api';
  import type { Node, Edge, NodeTypes } from '@xyflow/svelte';
  import type { CanvasNodeData } from '$lib/canvas-types';
  import type { Project, Team, Agent, Session, Soul, Skill, Harness, WorkCycle } from '$lib/types';
  import { parsePendingToolCalls, formatToolInput, computeMetrics } from '$lib/parse';
  import StatusBadge from '$lib/components/StatusBadge.svelte';

  import ProjectFrame from '$lib/components/canvas/ProjectFrame.svelte';
  import ActivityCard from '$lib/components/canvas/ActivityCard.svelte';
  import IdleGroup from '$lib/components/canvas/IdleGroup.svelte';

  const nodeTypes: NodeTypes = {
    project: ProjectFrame as any,
    activity: ActivityCard as any,
    idleGroup: IdleGroup as any,
  };

  let loaded = $state(false);
  let error = $state<string | null>(null);
  let nodes = $state<Node<CanvasNodeData>[]>([]);
  let edges = $state<Edge[]>([]);
  let selectedNode = $state<Node<CanvasNodeData> | null>(null);

  function handleNodeClick({ node }: { node: Node<CanvasNodeData>; event: MouseEvent | TouchEvent }) {
    selectedNode = selectedNode?.id === node.id ? null : node;
  }

  function handlePaneClick() {
    selectedNode = null;
  }

  // Derive detail panel content from selected node
  let panelType = $derived(selectedNode?.data?.type ?? null);

  let panelActivity = $derived.by(() => {
    if (panelType !== 'activity') return null;
    return selectedNode!.data as any;
  });

  let panelToolCalls = $derived.by(() => {
    const session = panelActivity?.session;
    if (!session) return [];
    const events = (session as any)._events ?? [];
    const calls: Array<{ name: string; args: string }> = [];
    for (const e of events) {
      if (e.action === 'ProcessToolCalls' && e.params?.pending_tool_calls) {
        const tcs = parsePendingToolCalls(e.params.pending_tool_calls as string);
        for (const tc of tcs) {
          calls.push({ name: tc.name, args: formatToolInput(tc.name, tc.input) });
        }
      }
    }
    return calls;
  });

  onMount(async () => {
    try {
      const [projectsRaw, teamsRaw, sessionsRaw, soulsRaw, skillsRaw, harnessesRaw, workCyclesRaw] = await Promise.all([
        queryEntities('Projects').catch(() => []),
        queryTeams().catch(() => []),
        queryEntities('Sessions', undefined, 'Id desc', 100).catch(() => []),
        queryEntities('Souls').catch(() => []),
        queryEntities('Skills').catch(() => []),
        queryEntities('Harnesses').catch(() => queryEntities('ProjectHarnesses').catch(() => [])),
        queryEntities('WorkCycles').catch(() => []),
      ]);

      const projects = projectsRaw as unknown as Project[];
      const teams = teamsRaw as unknown as Team[];
      const sessions = (sessionsRaw as unknown as Session[]).filter(s => s.agent_id || s.soul_id);
      const souls = soulsRaw as unknown as Soul[];
      const skills = skillsRaw as unknown as Skill[];
      const harnesses = harnessesRaw as unknown as Harness[];
      const workCycles = workCyclesRaw as unknown as WorkCycle[];

      const agentsMap: Record<string, Agent[]> = {};
      await Promise.all(teams.map(async (team) => {
        if (!team.Id) return;
        try {
          const teamAgents = await queryAgentsForTeam(team.Id);
          agentsMap[team.Id] = teamAgents as unknown as Agent[];
        } catch {}
      }));
      buildCanvasGraph({ projects, teams, agents: agentsMap, sessions, souls, skills, harnesses, workCycles });

      canvasNodes.subscribe(v => { nodes = v; });
      canvasEdges.subscribe(v => { edges = v; });

      loaded = true;
      connectSSE();
    } catch (e) {
      console.error('Canvas load error:', e);
      error = 'Failed to load canvas data';
      loaded = true;
    }
  });

  onDestroy(() => { disconnectSSE(); });

  // SSE live updates
  let lastSeq = $state(0);
  $effect(() => {
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

      <!-- Detail panel -->
      {#if selectedNode && panelType === 'activity' && panelActivity}
        <div class="detail-panel">
          <button class="panel-close" onclick={() => selectedNode = null}>&times;</button>

          <div class="panel-header">
            <span class="panel-name">{panelActivity.agent?.name}</span>
            {#if panelActivity.agent?.role}
              <span class="panel-role">{panelActivity.agent.role}</span>
            {/if}
            <StatusBadge status={panelActivity.session.Status} />
          </div>

          <div class="panel-meta">{panelActivity.agent?.provider}/{panelActivity.agent?.model}</div>

          <!-- Task -->
          {#if panelActivity.session.user_message ?? (panelActivity.session as any).fields?.user_message}
            <div class="panel-section">
              <span class="panel-label">Task</span>
              <div class="panel-task">{(panelActivity.session as any).user_message ?? (panelActivity.session as any).fields?.user_message}</div>
            </div>
          {/if}

          <!-- Tools -->
          {#if panelActivity.tools?.length > 0}
            <div class="panel-section">
              <span class="panel-label">Tools</span>
              <div class="panel-pills">
                {#each panelActivity.tools as tool}
                  <span class="panel-pill">{tool}</span>
                {/each}
              </div>
            </div>
          {/if}

          <!-- Skills -->
          {#if panelActivity.skills?.length > 0}
            <div class="panel-section">
              <span class="panel-label">Skills ({panelActivity.skills.length})</span>
              <div class="panel-pills">
                {#each panelActivity.skills as skill}
                  <span class="panel-pill">{skill.name || skill.Name}</span>
                {/each}
              </div>
            </div>
          {/if}

          <!-- Soul -->
          {#if panelActivity.soul}
            <div class="panel-section">
              <span class="panel-label">Soul</span>
              <span class="panel-value">{panelActivity.soul.Name ?? panelActivity.soul.name}</span>
            </div>
          {/if}

          <!-- Activity -->
          {#if panelToolCalls.length > 0}
            <div class="panel-section">
              <span class="panel-label">Activity ({panelToolCalls.length} tool calls)</span>
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
        </div>
      {/if}
    {:else if error}
      <div class="canvas-empty">
        <span class="empty-label">CANVAS</span>
        <span class="empty-text">{error}</span>
      </div>
    {:else}
      <div class="canvas-empty">
        <span class="empty-label">CANVAS</span>
        <span class="empty-text">No activity yet. Agents will appear here when sessions are running.</span>
        <a href="{base}/welcome" class="empty-link">Talk to Paw</a>
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
    height: 100dvh;
    position: relative;
  }

  .canvas-container :global(.svelte-flow) {
    background: var(--bg) !important;
  }

  .canvas-container :global(.svelte-flow__minimap) {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .canvas-container :global(.svelte-flow__controls) {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }

  .canvas-container :global(.svelte-flow__controls-button) {
    background: var(--surface);
    color: var(--text-2);
    border: none;
    border-bottom: 1px solid var(--border);
  }

  .canvas-container :global(.svelte-flow__controls-button:hover) {
    background: var(--surface-raised);
  }

  .canvas-container :global(.svelte-flow__controls-button svg) {
    fill: var(--text-2);
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
    gap: var(--sp-2);
  }

  .empty-label {
    font-family: var(--font-mono);
    font-size: var(--text-xl);
    letter-spacing: 0.08em;
    color: var(--text-3);
    opacity: 0.3;
  }

  .empty-text {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text-3);
  }

  .empty-link {
    font-family: var(--font-mono);
    font-size: var(--text-base);
    color: var(--accent);
    text-decoration: none;
    margin-top: var(--sp-1);
  }

  .empty-link:hover {
    text-decoration: underline;
  }

  /* ── Detail Panel ── */
  .detail-panel {
    position: absolute;
    top: 0;
    right: 0;
    width: 360px;
    height: 100%;
    background: var(--bg);
    border-left: 1px solid var(--border);
    z-index: 20;
    overflow-y: auto;
    padding: var(--sp-4);
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    font-family: var(--font-mono);
  }

  .panel-close {
    position: absolute;
    top: var(--sp-2);
    right: var(--sp-3);
    font-size: var(--text-lg);
    color: var(--text-3);
    cursor: pointer;
    background: none;
    border: none;
    line-height: 1;
  }
  .panel-close:hover { color: var(--text-1); }

  .panel-header {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding-right: var(--sp-6);
  }

  .panel-name {
    font-size: var(--text-lg);
    font-weight: 600;
    color: var(--accent);
  }

  .panel-role {
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .panel-meta {
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .panel-section {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }

  .panel-label {
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-3);
  }

  /* Terminal in panel */
  .panel-terminal {
    background: var(--terminal-bg);
    border: 1px solid var(--terminal-border);
    border-radius: var(--radius);
    padding: var(--sp-2) var(--sp-3);
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-height: 320px;
    overflow-y: auto;
  }

  .panel-line {
    display: flex;
    gap: var(--sp-1);
    font-size: var(--text-xs);
    white-space: nowrap;
    overflow: hidden;
  }

  .panel-prompt { color: var(--accent); flex-shrink: 0; user-select: none; }
  .panel-cmd { color: var(--text-1); font-weight: 500; flex-shrink: 0; }
  .panel-args { color: var(--text-3); overflow: hidden; text-overflow: ellipsis; }

  .panel-pills { display: flex; flex-wrap: wrap; gap: var(--sp-1); }
  .panel-pill {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-2);
    background: var(--surface);
    border: 1px solid var(--border);
    padding: 1px var(--sp-1);
    border-radius: var(--radius);
  }


  @media (max-width: 768px) {
    .detail-panel { width: 100%; }
  }

  @media (max-width: 640px) {
    .canvas-container {
      height: calc(100dvh - 64px);
    }

    .detail-panel {
      width: 100%;
      height: 60%;
      top: auto;
      bottom: 0;
      border-left: none;
      border-top: 1px solid var(--border);
      border-radius: var(--radius) var(--radius) 0 0;
    }

    .panel-close {
      width: 44px;
      height: 44px;
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: var(--text-xl);
    }

    .empty-text {
      padding: 0 var(--sp-4);
      text-align: center;
    }
  }
</style>
