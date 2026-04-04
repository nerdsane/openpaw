import { writable, get } from 'svelte/store';
import type { Agent, Session } from '$lib/types';
import { getEntity, queryEntities, queryTeams, queryAgentsForTeam } from '$lib/api';

/** Cache of agent/soul ID → display name */
const nameMap = writable<Map<string, string>>(new Map());

/** All agents across all teams */
export const allAgents = writable<Agent[]>([]);

/**
 * Resolve a human-readable name for a session's agent.
 * Tries agent_id first (name or role), then soul_id (name), caches result.
 */
export async function resolveAgentName(session: { agent_id?: string; soul_id?: string }): Promise<string> {
  const map = get(nameMap);

  // Try agent_id
  if (session.agent_id) {
    const cached = map.get(`agent:${session.agent_id}`);
    if (cached) return cached;

    try {
      const agent = await getEntity('Agents', session.agent_id);
      const f = agent as Record<string, unknown>;
      const name = (f.name ?? f.role ?? null) as string | null;
      if (name) {
        nameMap.update(m => { m.set(`agent:${session.agent_id!}`, name); return new Map(m); });
        return name;
      }
    } catch { /* fallback */ }
  }

  // Try soul_id
  if (session.soul_id) {
    const cached = map.get(`soul:${session.soul_id}`);
    if (cached) return cached;

    try {
      const soul = await getEntity('Souls', session.soul_id);
      const name = (soul as { name?: string; Name?: string }).name
        ?? (soul as { Name?: string }).Name ?? null;
      if (name) {
        nameMap.update(m => { m.set(`soul:${session.soul_id!}`, name); return new Map(m); });
        return name;
      }
    } catch {
      // soul_id might be a name string
      nameMap.update(m => { m.set(`soul:${session.soul_id!}`, session.soul_id!); return new Map(m); });
      return session.soul_id;
    }
  }

  return 'Session';
}

/**
 * Load all agents across all teams.
 */
export async function loadAllAgents(): Promise<void> {
  const teams = await queryTeams();
  const agents: Agent[] = [];
  await Promise.all(teams.map(async (team) => {
    const teamId = (team.Id ?? team.id ?? '') as string;
    if (!teamId) return;
    try {
      const teamAgents = await queryAgentsForTeam(teamId);
      agents.push(...(teamAgents as unknown as Agent[]));
    } catch { /* ignore */ }
  }));
  allAgents.set(agents);
}

/**
 * Fetch recent sessions for a specific agent.
 */
export async function fetchAgentSessions(agentId: string, top = 20): Promise<Session[]> {
  const data = await queryEntities('Sessions', `agent_id eq '${agentId}'`, 'SequenceNr desc', top);
  return data as unknown as Session[];
}
