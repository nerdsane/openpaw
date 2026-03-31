import { writable, derived } from 'svelte/store';
import type { Agent } from '$lib/types';
import { queryEntities, getEntity } from '$lib/api';

export const agents = writable<Agent[]>([]);

export const activeAgents = derived(agents, ($agents) =>
  $agents.filter((a) => !['Completed', 'Failed', 'Cancelled'].includes(a.Status))
);

export async function loadAgents(): Promise<void> {
  const data = await queryEntities('Agents', undefined, 'SequenceNr desc', 50);
  agents.set(data as unknown as Agent[]);
}

export async function refreshAgent(id: string): Promise<void> {
  const data = await getEntity('Agents', id);
  agents.update((list) => list.map((a) => (a.Id === id ? (data as unknown as Agent) : a)));
}
