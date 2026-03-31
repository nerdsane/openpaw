import { writable, derived } from 'svelte/store';
import type { WorkCycle } from '$lib/types';
import { queryEntities, getEntity } from '$lib/api';

export const workcycles = writable<WorkCycle[]>([]);

export const activeWorkCycles = derived(workcycles, ($wc) =>
  $wc.filter((w) => !['Completed', 'Failed', 'Cancelled'].includes(w.Status))
);

export async function loadWorkCycles(): Promise<void> {
  const data = await queryEntities('WorkCycles', undefined, 'SequenceNr desc', 50);
  workcycles.set(data as unknown as WorkCycle[]);
}

export async function refreshWorkCycle(id: string): Promise<void> {
  const data = await getEntity('WorkCycles', id);
  workcycles.update((list) =>
    list.map((w) => (w.Id === id ? (data as unknown as WorkCycle) : w))
  );
}
