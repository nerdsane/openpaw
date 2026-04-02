import { writable, derived } from 'svelte/store';
import type { Session } from '$lib/types';
import { queryEntities, getEntity } from '$lib/api';

export const sessions = writable<Session[]>([]);

export const activeSessions = derived(sessions, ($sessions) =>
  $sessions.filter((a) => !['Completed', 'Failed', 'Cancelled'].includes(a.Status))
);

export async function loadSessions(): Promise<void> {
  const data = await queryEntities('Sessions', undefined, 'SequenceNr desc', 50);
  // Only show sessions that belong to a team agent or soul
  const meaningful = (data as unknown as Session[]).filter(
    (s) => s.agent_id || s.soul_id
  );
  sessions.set(meaningful);
}

export async function refreshSession(id: string): Promise<void> {
  const data = await getEntity('Sessions', id);
  sessions.update((list) => list.map((a) => (a.Id === id ? (data as unknown as Session) : a)));
}
