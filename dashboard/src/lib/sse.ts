import { writable } from 'svelte/store';

export interface StateChangeEvent {
  seq: number;
  entity_type: string;
  entity_id: string;
  action: string;
  status: string;
  tenant: string;
  agent_id?: string;
  session_id?: string;
}

export const events = writable<StateChangeEvent[]>([]);

let source: EventSource | null = null;

export function connectSSE(entityType?: string, entityId?: string): EventSource {
  if (source) source.close();

  let url = '/observe/events/stream';
  const params = new URLSearchParams();
  if (entityType) params.set('entity_type', entityType);
  if (entityId) params.set('entity_id', entityId);
  const qs = params.toString();
  if (qs) url += `?${qs}`;

  source = new EventSource(url);
  source.addEventListener('state_change', (e) => {
    const event: StateChangeEvent = JSON.parse((e as MessageEvent).data);
    events.update((list) => [event, ...list].slice(0, 500));
  });

  return source;
}

export function disconnectSSE(): void {
  if (source) {
    source.close();
    source = null;
  }
}
