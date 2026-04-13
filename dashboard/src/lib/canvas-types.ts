/**
 * Canvas node data types — simplified for vertical flow layout.
 * Three node types: project frame, activity card, idle group.
 */

import type { Agent, Team, Session, Skill, Soul, Harness, WorkCycle } from './types';

export type CanvasNodeData =
  | ProjectNodeData
  | ActivityCardData
  | IdleGroupData;

export interface ProjectNodeData extends Record<string, unknown> {
  type: 'project';
  team: Team;
  harness: Harness | null;
  agentCount: number;
  activeCount: number;
  apps: string[]; // installed app names
  workCycles: WorkCycle[];
}

export interface ActivityCardData extends Record<string, unknown> {
  type: 'activity';
  agent: Agent;
  session: Session;
  soul: Soul | null;
  skills: Skill[];
  tools: string[];
}

export interface IdleGroupData extends Record<string, unknown> {
  type: 'idleGroup';
  agents: Agent[];
}

// Legacy canvas components still import these aliases.
export interface AgentNodeData extends Record<string, unknown> {
  type: 'agent';
  agent: Agent;
  sessionCount: number;
  activeSessionCount: number;
  hasPendingDecision: boolean;
}

export interface EntityBadgeData extends Record<string, unknown> {
  type: 'entityBadge';
  entityType: string;
  label: string;
}

export interface PlatformNodeData extends Record<string, unknown> {
  type: 'platform';
  skills: Skill[];
}

export interface SessionNodeData extends Record<string, unknown> {
  type: 'session';
  session: Session;
  agentName: string;
}

export interface SoulNodeData extends Record<string, unknown> {
  type: 'soul';
  soul: Soul;
}

/** Layout constants — vertical flow, fits viewport */
export const LAYOUT = {
  PROJECT_START_X: 20,
  PROJECT_START_Y: 20,
  PROJECT_GAP: 40,
  PROJECT_WIDTH: 640,
  PROJECT_HEADER_HEIGHT: 64,
  PROJECT_FOOTER_HEIGHT: 48,
  // Activity cards (agent + session combined, full width)
  ACTIVITY_WIDTH: 600,
  ACTIVITY_HEIGHT: 180,
  ACTIVITY_GAP: 12,
  ACTIVITY_START_X: 20,
  ACTIVITY_START_Y: 70,
  // Idle group (compressed row of names)
  IDLE_HEIGHT: 32,
  IDLE_GAP: 8,
} as const;
