/**
 * Canvas store — vertical flow layout.
 * Projects stack vertically. Active agents show as full-width terminal cards.
 * Idle agents compress into a single row.
 */

import { writable } from 'svelte/store';
import type { Node, Edge } from '@xyflow/svelte';
import type { Team, Agent, Session, Skill, Soul, Harness, WorkCycle } from '$lib/types';
import type { CanvasNodeData } from '$lib/canvas-types';
import { LAYOUT } from '$lib/canvas-types';

export const canvasNodes = writable<Node<CanvasNodeData>[]>([]);
export const canvasEdges = writable<Edge[]>([]);
export const selectedNodeId = writable<string | null>(null);
export const zoomLevel = writable<number>(0.5);
export const focusTarget = writable<{ type: string; id: string } | null>(null);

const TERMINAL_STATUSES = ['Completed', 'Failed', 'Cancelled'];

// Known apps per project — in practice this would come from the API
const PROJECT_APPS: Record<string, string[]> = {
  'default': ['paw-agent', 'paw-harness', 'paw-heal', 'paw-foresight', 'paw-pm', 'paw-fs'],
};

export function buildCanvasGraph(data: {
  teams: Team[];
  agents: Record<string, Agent[]>;
  sessions: Session[];
  souls: Soul[];
  skills: Skill[];
  harnesses: Harness[];
  workCycles: WorkCycle[];
}): void {
  const nodes: Node<CanvasNodeData>[] = [];
  const edges: Edge[] = [];

  let projectY = LAYOUT.PROJECT_START_Y;

  for (const team of data.teams) {
    const teamAgents = data.agents[team.Id] ?? [];
    const projectId = `project-${team.Id}`;

    const harness = data.harnesses.find(h =>
      h.Id === (team as any).harness_id
    ) ?? data.harnesses[0] ?? null;

    // Separate active vs idle agents
    const activeAgentsWithSessions: Array<{ agent: Agent; session: Session }> = [];
    const idleAgents: Agent[] = [];

    for (const agent of teamAgents) {
      const agentSessions = data.sessions.filter(s => s.agent_id === agent.Id);
      const activeSession = agentSessions.find(s => !TERMINAL_STATUSES.includes(s.Status));
      if (activeSession) {
        activeAgentsWithSessions.push({ agent, session: activeSession });
      } else {
        idleAgents.push(agent);
      }
    }

    // Compute project frame height
    const activityBlockHeight = activeAgentsWithSessions.length * (LAYOUT.ACTIVITY_HEIGHT + LAYOUT.ACTIVITY_GAP);
    const idleBlockHeight = idleAgents.length > 0 ? LAYOUT.IDLE_HEIGHT + LAYOUT.IDLE_GAP : 0;
    const frameHeight = LAYOUT.PROJECT_HEADER_HEIGHT + activityBlockHeight + idleBlockHeight + LAYOUT.PROJECT_FOOTER_HEIGHT + 16;

    const apps = PROJECT_APPS['default'] ?? [];

    nodes.push({
      id: projectId,
      type: 'project',
      position: { x: LAYOUT.PROJECT_START_X, y: projectY },
      data: {
        type: 'project',
        team,
        harness,
        agentCount: teamAgents.length,
        activeCount: activeAgentsWithSessions.length,
        apps,
        workCycles: data.workCycles,
      },
      style: `width: ${LAYOUT.PROJECT_WIDTH}px; height: ${frameHeight}px;`,
    });

    // ── Activity cards (active agents with sessions) ──
    let cardY = LAYOUT.ACTIVITY_START_Y;

    activeAgentsWithSessions.forEach((item) => {
      const cardId = `activity-${item.agent.Id}`;

      const soul = data.souls.find(s =>
        s.Id === item.agent.soul_id || s.Name === item.agent.soul_id || s.name === item.agent.soul_id
      ) ?? null;

      const agentSkills = data.skills.filter(sk => {
        const scope = ((sk as any).scope ?? '') as string;
        const soulName = soul?.Name ?? soul?.name ?? item.agent.soul_id ?? '';
        return scope === 'global' || scope === soulName;
      });

      const tools = ((item.agent as any).tools_enabled || '').split(',').filter(Boolean);

      nodes.push({
        id: cardId,
        type: 'activity',
        position: { x: LAYOUT.ACTIVITY_START_X, y: cardY },
        data: {
          type: 'activity',
          agent: item.agent,
          session: item.session,
          soul,
          skills: agentSkills,
          tools,
        },
        parentId: projectId,
        extent: 'parent' as const,
        style: `width: ${LAYOUT.ACTIVITY_WIDTH}px; height: ${LAYOUT.ACTIVITY_HEIGHT}px;`,
      });

      cardY += LAYOUT.ACTIVITY_HEIGHT + LAYOUT.ACTIVITY_GAP;
    });

    // ── Idle group (compressed) ──
    if (idleAgents.length > 0) {
      const idleId = `idle-${team.Id}`;
      nodes.push({
        id: idleId,
        type: 'idleGroup',
        position: { x: LAYOUT.ACTIVITY_START_X, y: cardY },
        data: {
          type: 'idleGroup',
          agents: idleAgents,
        },
        parentId: projectId,
        extent: 'parent' as const,
        style: `width: ${LAYOUT.ACTIVITY_WIDTH}px; height: ${LAYOUT.IDLE_HEIGHT}px;`,
      });
    }

    projectY += frameHeight + LAYOUT.PROJECT_GAP;
  }

  // ── Orphan sessions (no agent, no team) ──
  const orphanSessions = data.sessions.filter(s =>
    !s.agent_id && !TERMINAL_STATUSES.includes(s.Status)
  );

  orphanSessions.forEach((session, i) => {
    nodes.push({
      id: `activity-orphan-${session.Id}`,
      type: 'activity',
      position: {
        x: LAYOUT.PROJECT_START_X,
        y: projectY + i * (LAYOUT.ACTIVITY_HEIGHT + LAYOUT.ACTIVITY_GAP),
      },
      data: {
        type: 'activity',
        agent: { Id: session.Id, Status: session.Status, name: session.soul_id || 'Session', role: '', soul_id: session.soul_id } as any,
        session,
        soul: data.souls.find(s => s.Name === session.soul_id || s.name === session.soul_id) ?? null,
        skills: [],
        tools: [],
      },
      style: `width: ${LAYOUT.ACTIVITY_WIDTH}px; height: ${LAYOUT.ACTIVITY_HEIGHT}px;`,
    });
  });

  canvasNodes.set(nodes);
  canvasEdges.set(edges); // No edges needed — relationships are shown inline
}

export function updateAgentSessions(agentId: string, sessions: Session[]): void {
  canvasNodes.update(n => n);
}
