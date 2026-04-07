/**
 * Canvas node data types for the Svelte Flow mission control canvas.
 * Each node type has a discriminated union tag for type-safe rendering.
 */

import type { Agent, Team, Session, Skill, Soul, Harness, WorkCycle } from './types';

export type CanvasNodeData =
  | ProjectNodeData
  | AgentNodeData
  | SoulNodeData
  | PlatformNodeData;

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
  sessions: Session[];
  soul: Soul | null;
  skills: Skill[];
  authzAllowed: number;
  authzDenied: number;
  hasPendingDecision: boolean;
}

export interface SoulNodeData {
  type: 'soul';
  soul: Soul;
}

export interface PlatformNodeData {
  type: 'platform';
  souls: Soul[];
  skills: Skill[];
}

/** Layout constants */
export const LAYOUT = {
  PLATFORM_X: -500,
  PLATFORM_Y: 0,
  PLATFORM_WIDTH: 320,
  PROJECT_START_X: 50,
  PROJECT_START_Y: 0,
  PROJECT_WIDTH: 1200,
  PROJECT_GAP: 80,
  PROJECT_HEADER_HEIGHT: 100,
  AGENT_WIDTH: 340,
  AGENT_HEIGHT: 280,
  AGENT_GAP: 16,
  AGENT_START_X: 20,
  AGENT_START_Y: 100,
  AGENTS_PER_ROW: 3,
  SOUL_HEIGHT: 40,
  SOUL_GAP: 6,
} as const;
