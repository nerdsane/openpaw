/**
 * Canvas store — manages the Svelte Flow node/edge graph,
 * layout computation, zoom state, and focus management.
 */

import { writable, get } from 'svelte/store';
import type { Node, Edge } from '@xyflow/svelte';
import type { Team, Agent, Session, Skill, Soul, Harness, WorkCycle } from '$lib/types';
import type {
  CanvasNodeData,
  ProjectNodeData,
  AgentNodeData,
  SoulNodeData,
  PlatformNodeData,
} from '$lib/canvas-types';
import { LAYOUT } from '$lib/canvas-types';

export const canvasNodes = writable<Node<CanvasNodeData>[]>([]);
export const canvasEdges = writable<Edge[]>([]);
export const selectedNodeId = writable<string | null>(null);
export const zoomLevel = writable<number>(0.5);

/** Focus target for deep-link navigation */
export const focusTarget = writable<{ type: string; id: string } | null>(null);

/**
 * Build the full canvas graph from loaded data.
 * Deterministic grid layout — no force-directed physics.
 */
export function buildCanvasGraph(data: {
  teams: Team[];
  agents: Record<string, Agent[]>; // teamId → agents
  sessions: Session[];
  souls: Soul[];
  skills: Skill[];
  harnesses: Harness[];
  workCycles: WorkCycle[];
}): void {
  const nodes: Node<CanvasNodeData>[] = [];
  const edges: Edge[] = [];

  // --- Platform cluster ---
  const platformId = 'platform-cluster';
  const platformHeight = Math.max(200, data.souls.length * (LAYOUT.SOUL_HEIGHT + LAYOUT.SOUL_GAP) + 80 + data.skills.length * 30);

  nodes.push({
    id: platformId,
    type: 'platform',
    position: { x: LAYOUT.PLATFORM_X, y: LAYOUT.PLATFORM_Y },
    data: {
      type: 'platform',
      souls: data.souls,
      skills: data.skills,
    },
    style: `width: ${LAYOUT.PLATFORM_WIDTH}px; height: ${platformHeight}px;`,
  });

  // Soul nodes inside platform
  data.souls.forEach((soul, i) => {
    const soulNodeId = `soul-${soul.Id}`;
    nodes.push({
      id: soulNodeId,
      type: 'soul',
      position: { x: 20, y: 50 + i * (LAYOUT.SOUL_HEIGHT + LAYOUT.SOUL_GAP) },
      data: { type: 'soul', soul },
      parentId: platformId,
      extent: 'parent' as const,
      style: `width: ${LAYOUT.PLATFORM_WIDTH - 40}px; height: ${LAYOUT.SOUL_HEIGHT}px;`,
    });
  });

  // --- Project frames ---
  let projectY = LAYOUT.PROJECT_START_Y;

  for (const team of data.teams) {
    const teamAgents = data.agents[team.Id] ?? [];
    const projectId = `project-${team.Id}`;

    // Find harness for this team
    const harness = data.harnesses.find(h =>
      h.Id === (team as Record<string, unknown>).harness_id
    ) ?? data.harnesses[0] ?? null;

    // Find work cycles
    const wcs = data.workCycles;

    // Compute frame height based on agent grid
    const perRow = LAYOUT.AGENTS_PER_ROW;
    const agentRows = Math.ceil(teamAgents.length / perRow) || 1;
    const projectHeight = LAYOUT.AGENT_START_Y + agentRows * (LAYOUT.AGENT_HEIGHT + LAYOUT.AGENT_GAP) + 60;

    nodes.push({
      id: projectId,
      type: 'project',
      position: { x: LAYOUT.PROJECT_START_X, y: projectY },
      data: {
        type: 'project',
        team,
        harness,
        agents: teamAgents,
        workCycles: wcs,
      },
      style: `width: ${LAYOUT.PROJECT_WIDTH}px; height: ${projectHeight}px;`,
    });

    // Agent nodes inside project
    teamAgents.forEach((agent, i) => {
      const col = i % perRow;
      const row = Math.floor(i / perRow);
      const agentNodeId = `agent-${agent.Id}`;

      // Find sessions for this agent
      const agentSessions = data.sessions.filter(s => s.agent_id === agent.Id);

      // Find soul
      const soul = data.souls.find(s =>
        s.Id === agent.soul_id || s.Name === agent.soul_id || s.name === agent.soul_id
      ) ?? null;

      // Find skills for this agent
      const agentSkills = data.skills.filter(sk => {
        const scope = ((sk as Record<string, unknown>).scope ?? (sk as Record<string, unknown>).Scope ?? '') as string;
        const soulName = soul?.Name ?? soul?.name ?? agent.soul_id ?? '';
        return scope === 'global' || scope === soulName;
      });

      nodes.push({
        id: agentNodeId,
        type: 'agent',
        position: {
          x: LAYOUT.AGENT_START_X + col * (LAYOUT.AGENT_WIDTH + LAYOUT.AGENT_GAP),
          y: LAYOUT.AGENT_START_Y + row * (LAYOUT.AGENT_HEIGHT + LAYOUT.AGENT_GAP),
        },
        data: {
          type: 'agent',
          agent,
          sessions: agentSessions,
          soul,
          skills: agentSkills,
          authzAllowed: 0,
          authzDenied: 0,
          hasPendingDecision: agentSessions.some(s => !!(s as any).pending_decision_id || !!(s as any).fields?.pending_decision_id),
        },
        parentId: projectId,
        extent: 'parent' as const,
        style: `width: ${LAYOUT.AGENT_WIDTH}px; height: ${LAYOUT.AGENT_HEIGHT}px;`,
      });

      // Edge: agent → soul (if soul exists in platform cluster)
      if (soul) {
        edges.push({
          id: `edge-${agentNodeId}-soul-${soul.Id}`,
          source: agentNodeId,
          target: `soul-${soul.Id}`,
          type: 'smoothstep',
          animated: false,
          style: 'stroke: var(--terminal-text); stroke-width: 1; opacity: 0.3;',
        });
      }
    });

    projectY += projectHeight + LAYOUT.PROJECT_GAP;
  }

  // --- Sessions without agents (e.g., Probe sessions) ---
  const orphanSessions = data.sessions.filter(s =>
    !s.agent_id && !['Completed', 'Failed', 'Cancelled'].includes(s.Status)
  );

  // Place orphan sessions as standalone agent-like nodes below projects
  orphanSessions.forEach((session, i) => {
    const nodeId = `session-${session.Id}`;
    nodes.push({
      id: nodeId,
      type: 'agent',
      position: {
        x: LAYOUT.PROJECT_START_X + i * (LAYOUT.AGENT_WIDTH + LAYOUT.AGENT_GAP),
        y: projectY + 20,
      },
      data: {
        type: 'agent',
        agent: {
          Id: session.Id,
          Status: session.Status,
          name: session.soul_id || 'Session',
          role: '',
          description: session.user_message || '',
          soul_id: session.soul_id,
          team_id: '',
          model: session.model,
          provider: session.provider,
          tools_enabled: '',
          max_turns: session.max_turns,
          skill_ids: '',
        } as Agent,
        sessions: [session],
        soul: data.souls.find(s => s.Name === session.soul_id || s.name === session.soul_id) ?? null,
        skills: [],
        authzAllowed: 0,
        authzDenied: 0,
        hasPendingDecision: false,
      },
      style: `width: ${LAYOUT.AGENT_WIDTH}px; height: ${LAYOUT.AGENT_HEIGHT}px;`,
    });
  });

  canvasNodes.set(nodes);
  canvasEdges.set(edges);
}

/**
 * Update a single agent node's session data (for SSE live updates).
 */
export function updateAgentSessions(agentId: string, sessions: Session[]): void {
  canvasNodes.update(nodes =>
    nodes.map(n => {
      if (n.id === `agent-${agentId}` && n.data.type === 'agent') {
        return { ...n, data: { ...n.data, sessions } };
      }
      return n;
    })
  );
}
