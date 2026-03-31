import { derived } from 'svelte/store';
import { events, type StateChangeEvent } from '$lib/sse';

// Re-export the SSE store for convenience
export { events };
export type { StateChangeEvent };

export const recentEvents = derived(events, ($events) => $events.slice(0, 100));

export const agentEvents = derived(events, ($events) =>
  $events.filter((e) => e.entity_type === 'Agent')
);

export const workCycleEvents = derived(events, ($events) =>
  $events.filter((e) => e.entity_type === 'WorkCycle')
);
