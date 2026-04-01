import { writable, derived } from 'svelte/store';
import type { WorkCycle } from '$lib/types';
import { queryEntities, getEntity } from '$lib/api';

export const workcycles = writable<WorkCycle[]>([]);

export const activeWorkCycles = derived(workcycles, ($wc) =>
  $wc.filter((w) => !['Completed', 'Failed', 'Cancelled'].includes(w.Status))
);

/**
 * Load work cycles. Discovers harnesses and loads from each harness's
 * work_cycle_type entity set. Falls back to generic WorkCycles if no
 * harnesses are found.
 */
export async function loadWorkCycles(): Promise<void> {
  const allWcs: WorkCycle[] = [];
  try {
    const harnesses = await queryEntities('Harnesses').catch(() =>
      queryEntities('ProjectHarnesses').catch(() => [])
    );
    const triedSets = new Set<string>();
    for (const h of harnesses) {
      const wcType = ((h as Record<string, unknown>).work_cycle_type as string) || 'WorkCycles';
      const hId = (h as Record<string, unknown>).Id as string;
      if (!triedSets.has(`${wcType}:${hId}`)) {
        triedSets.add(`${wcType}:${hId}`);
        try {
          const data = await queryEntities(wcType, `harness_id eq '${hId}'`, 'SequenceNr desc', 50);
          allWcs.push(...(data as unknown as WorkCycle[]));
        } catch { /* entity set may not exist */ }
      }
    }
    // Fallback if no harnesses found
    if (harnesses.length === 0) {
      const data = await queryEntities('WorkCycles', undefined, 'SequenceNr desc', 50);
      allWcs.push(...(data as unknown as WorkCycle[]));
    }
  } catch {
    // Final fallback
    try {
      const data = await queryEntities('WorkCycles', undefined, 'SequenceNr desc', 50);
      allWcs.push(...(data as unknown as WorkCycle[]));
    } catch { /* ignore */ }
  }
  workcycles.set(allWcs);
}

export async function refreshWorkCycle(id: string, entitySet?: string): Promise<void> {
  const set = entitySet || 'WorkCycles';
  try {
    const data = await getEntity(set, id);
    workcycles.update((list) =>
      list.map((w) => (w.Id === id ? (data as unknown as WorkCycle) : w))
    );
  } catch {
    // Try generic WorkCycles as fallback
    if (set !== 'WorkCycles') {
      try {
        const data = await getEntity('WorkCycles', id);
        workcycles.update((list) =>
          list.map((w) => (w.Id === id ? (data as unknown as WorkCycle) : w))
        );
      } catch { /* ignore */ }
    }
  }
}
