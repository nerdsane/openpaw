import { writable, derived } from 'svelte/store';
import type { Session } from '$lib/types';
import { queryEntities, getEntity } from '$lib/api';

export const sessions = writable<Session[]>([]);

export const activeSessions = derived(sessions, ($sessions) =>
  $sessions.filter((a) => !['Completed', 'Failed', 'Cancelled'].includes(a.Status))
);

export async function loadSessions(): Promise<void> {
  const data = await queryEntities('Sessions', undefined, 'SequenceNr desc', 50);
  sessions.set(data as unknown as Session[]);
}

export async function refreshSession(id: string): Promise<void> {
  const data = await getEntity('Sessions', id);
  sessions.update((list) => list.map((a) => (a.Id === id ? (data as unknown as Session) : a)));
}
