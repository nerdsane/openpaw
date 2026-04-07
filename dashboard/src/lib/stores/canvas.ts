/**
 * Canvas store — builds the Svelte Flow node/edge graph.
 * Agents as identity cards, sessions as terminal nodes below, edges showing relationships.
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

    const wcs = data.workCycles;

    // Collect active sessions for this team's agents
    const teamActiveSessions: Array<{ agent: Agent; session: Session }> = [];
    for (const agent of teamAgents) {
      const agentSessions = data.sessions.filter(s => s.agent_id === agent.Id);
      for (const s of agentSessions) {
        if (!TERMINAL_STATUSES.includes(s.Status)) {
          teamActiveSessions.push({ agent, session: s });
        }
      }
    }

    // Compute frame height
    const sessionCount = teamActiveSessions.length;
    const sessionCols = Math.min(sessionCount, 4);
    const sessionRows = Math.ceil(sessionCount / 4) || 0;
    const frameHeight = LAYOUT.AGENT_ROW_Y + LAYOUT.AGENT_HEIGHT + 20 +
      (sessionRows > 0 ? sessionRows * (LAYOUT.SESSION_HEIGHT + LAYOUT.SESSION_GAP) + 20 : 0) +
      60; // footer

    nodes.push({
      id: projectId,
      type: 'project',
      position: { x: LAYOUT.PROJECT_START_X, y: projectY },
      data: { type: 'project', team, harness, agents: teamAgents, workCycles: wcs },
      style: `width: ${LAYOUT.PROJECT_WIDTH}px; height: ${frameHeight}px;`,
    });

    // ── Agent identity cards (top row) ──
    teamAgents.forEach((agent, i) => {
      const agentNodeId = `agent-${agent.Id}`;
      const agentSessions = data.sessions.filter(s => s.agent_id === agent.Id);
      const activeSessions = agentSessions.filter(s => !TERMINAL_STATUSES.includes(s.Status));
      const soul = data.souls.find(s =>
        s.Id === agent.soul_id || s.Name === agent.soul_id || s.name === agent.soul_id
      ) ?? null;

      const skillCount = data.skills.filter(sk => {
        const scope = ((sk as any).scope ?? (sk as any).Scope ?? '') as string;
        const soulName = soul?.Name ?? soul?.name ?? agent.soul_id ?? '';
        return scope === 'global' || scope === soulName;
      }).length;

      nodes.push({
        id: agentNodeId,
        type: 'agent',
        position: {
          x: LAYOUT.AGENT_START_X + i * (LAYOUT.AGENT_WIDTH + LAYOUT.AGENT_GAP),
          y: LAYOUT.AGENT_ROW_Y,
        },
        data: {
          type: 'agent',
          agent,
          sessionCount: agentSessions.length,
          activeSessionCount: activeSessions.length,
          soul,
          skillCount,
          hasPendingDecision: agentSessions.some(s =>
            !!(s as any).pending_decision_id || !!(s as any).fields?.pending_decision_id
          ),
        },
        parentId: projectId,
        extent: 'parent' as const,
        style: `width: ${LAYOUT.AGENT_WIDTH}px; height: ${LAYOUT.AGENT_HEIGHT}px;`,
      });
    });

    // ── Session terminal nodes (below agents) ──
    teamActiveSessions.forEach((item, i) => {
      const col = i % 4;
      const row = Math.floor(i / 4);
      const sessionNodeId = `session-${item.session.Id}`;
      const agentNodeId = `agent-${item.agent.Id}`;

      nodes.push({
        id: sessionNodeId,
        type: 'session',
        position: {
          x: LAYOUT.AGENT_START_X + col * (LAYOUT.SESSION_WIDTH + LAYOUT.SESSION_GAP),
          y: LAYOUT.SESSION_ROW_Y + row * (LAYOUT.SESSION_HEIGHT + LAYOUT.SESSION_GAP),
        },
        data: {
          type: 'session',
          session: item.session,
          agentName: item.agent.name || item.agent.role || 'Agent',
        },
        parentId: projectId,
        extent: 'parent' as const,
        style: `width: ${LAYOUT.SESSION_WIDTH}px; height: ${LAYOUT.SESSION_HEIGHT}px;`,
      });

      // Edge: agent → session
      edges.push({
        id: `edge-${agentNodeId}-${sessionNodeId}`,
        source: agentNodeId,
        target: sessionNodeId,
        type: 'smoothstep',
        style: 'stroke: var(--accent, #00DC82); stroke-width: 1; opacity: 0.5;',
      });

      // Edge: parent session → child session
      const parentId = (item.session as any).parent_session_id ?? (item.session as any).fields?.parent_session_id;
      if (parentId) {
        edges.push({
          id: `edge-parent-${parentId}-${sessionNodeId}`,
          source: `session-${parentId}`,
          target: sessionNodeId,
          type: 'smoothstep',
          animated: true,
          style: 'stroke: var(--text-3, #666); stroke-width: 1; stroke-dasharray: 4 2;',
        });
      }

      // ── Entity badges from temper_action tool calls ──
      const events = (item.session as any)._events ?? [];
      const referencedEntities = new Set<string>();
      for (const e of events) {
        if (e.action === 'ProcessToolCalls' && e.params?.pending_tool_calls) {
          try {
            const tcs = JSON.parse(e.params.pending_tool_calls);
            for (const tc of tcs) {
              if (tc.name === 'temper_action' && tc.input?.entity_set && tc.input?.entity_id) {
                const key = `${tc.input.entity_set}:${tc.input.entity_id}`;
                if (!referencedEntities.has(key)) {
                  referencedEntities.add(key);
                  const entityNodeId = `entity-${tc.input.entity_set}-${tc.input.entity_id}`;

                  nodes.push({
                    id: entityNodeId,
                    type: 'entity',
                    position: {
                      x: LAYOUT.AGENT_START_X + col * (LAYOUT.SESSION_WIDTH + LAYOUT.SESSION_GAP) +
                        (referencedEntities.size - 1) * (LAYOUT.ENTITY_WIDTH + LAYOUT.ENTITY_GAP),
                      y: LAYOUT.SESSION_ROW_Y + row * (LAYOUT.SESSION_HEIGHT + LAYOUT.SESSION_GAP) +
                        LAYOUT.SESSION_HEIGHT + LAYOUT.ENTITY_OFFSET_Y,
                    },
                    data: {
                      type: 'entity',
                      entityType: tc.input.entity_set.replace(/s$/, ''),
                      entityId: tc.input.entity_id,
                      status: '',
                      label: `${tc.input.action ?? ''}`,
                    },
                    parentId: projectId,
                    extent: 'parent' as const,
                    style: `width: ${LAYOUT.ENTITY_WIDTH}px; height: ${LAYOUT.ENTITY_HEIGHT}px;`,
                  });

                  // Edge: session → entity
                  edges.push({
                    id: `edge-${sessionNodeId}-${entityNodeId}`,
                    source: sessionNodeId,
                    target: entityNodeId,
                    type: 'smoothstep',
                    style: 'stroke: var(--text-3, #666); stroke-width: 1; opacity: 0.3; stroke-dasharray: 2 2;',
                  });
                }
              }
            }
          } catch { /* skip malformed */ }
        }
      }
    });

    projectY += frameHeight + LAYOUT.PROJECT_GAP;
  }

  // ── Orphan sessions (no agent, e.g. probes) ──
  const orphanSessions = data.sessions.filter(s =>
    !s.agent_id && !TERMINAL_STATUSES.includes(s.Status)
  );

  if (orphanSessions.length > 0) {
    orphanSessions.forEach((session, i) => {
      const nodeId = `session-${session.Id}`;
      nodes.push({
        id: nodeId,
        type: 'session',
        position: {
          x: LAYOUT.PROJECT_START_X + i * (LAYOUT.SESSION_WIDTH + LAYOUT.SESSION_GAP),
          y: projectY + 20,
        },
        data: {
          type: 'session',
          session,
          agentName: session.soul_id || 'Session',
        },
        style: `width: ${LAYOUT.SESSION_WIDTH}px; height: ${LAYOUT.SESSION_HEIGHT}px;`,
      });
    });
  }

  canvasNodes.set(nodes);
  canvasEdges.set(edges);
}

export function updateAgentSessions(agentId: string, sessions: Session[]): void {
  // For live updates, rebuild would be needed — simplified for now
  canvasNodes.update(n => n);
}
