/**
 * Canvas node data types for the Svelte Flow mission control canvas.
 */

import type { Agent, Team, Session, Skill, Soul, Harness, WorkCycle } from './types';

export type CanvasNodeData =
  | ProjectNodeData
  | AgentNodeData
  | SessionNodeData
  | EntityBadgeData;

export interface ProjectNodeData {
  type: 'project';
  team: Team;
  harness: Harness | null;
  agents: Agent[];
  workCycles: WorkCycle[];
}

export interface AgentNodeData {
  type: 'agent';
  agent: Agent;
  sessionCount: number;
  activeSessionCount: number;
  soul: Soul | null;
  skillCount: number;
  hasPendingDecision: boolean;
}

export interface SessionNodeData {
  type: 'session';
  session: Session;
  agentName: string;
}

export interface EntityBadgeData {
  type: 'entity';
  entityType: string;
  entityId: string;
  status: string;
  label: string;
}

/** Layout constants */
export const LAYOUT = {
  PROJECT_START_X: 20,
  PROJECT_START_Y: 20,
  PROJECT_WIDTH: 2400,
  PROJECT_GAP: 80,
  // Agent identity cards (small, top row)
  // Gap between agents must be >= SESSION_WIDTH so sessions don't overlap
  AGENT_ROW_Y: 60,
  AGENT_WIDTH: 170,
  AGENT_HEIGHT: 50,
  AGENT_COLUMN_WIDTH: 220, // each agent column: enough for session below
  AGENTS_PER_ROW: 5,
  AGENT_START_X: 20,
  // Session terminals (below agents)
  SESSION_ROW_Y: 140,
  SESSION_WIDTH: 200,
  SESSION_HEIGHT: 180,
  SESSION_GAP: 20,
  // Entity badges (small, near sessions)
  ENTITY_WIDTH: 140,
  ENTITY_HEIGHT: 28,
  ENTITY_GAP: 8,
  ENTITY_OFFSET_Y: 20, // below session
} as const;
