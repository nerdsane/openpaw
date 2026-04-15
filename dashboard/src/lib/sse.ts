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

// ── Named connections ──

export interface SSEHandle {
  source: EventSource;
  close(): void;
}

const connections = new Map<string, SSEHandle>();

export function createSSEConnection(
  name: string,
  entityType?: string,
  entityId?: string,
  onEvent?: (event: StateChangeEvent) => void,
  onError?: () => void
): SSEHandle {
  // Close any existing connection with the same name
  const existing = connections.get(name);
  if (existing) existing.close();

  let url = '/observe/events/stream';
  const params = new URLSearchParams();
  if (entityType) params.set('entity_type', entityType);
  if (entityId) params.set('entity_id', entityId);
  const qs = params.toString();
  if (qs) url += `?${qs}`;

  const source = new EventSource(url);
  source.addEventListener('state_change', (e) => {
    const event: StateChangeEvent = JSON.parse((e as MessageEvent).data);
    if (onEvent) {
      onEvent(event);
    }
    // Also push to the global store
    events.update((list) => [event, ...list].slice(0, 500));
  });

  source.onerror = () => {
    // EventSource auto-reconnects on transient errors, but if it gives up
    // (readyState === CLOSED), clean up and notify the caller.
    if (source.readyState === EventSource.CLOSED) {
      connections.delete(name);
      if (onError) onError();
    }
  };

  const handle: SSEHandle = {
    source,
    close() {
      source.close();
      connections.delete(name);
    },
  };

  connections.set(name, handle);
  return handle;
}

export function closeSSEConnection(name: string): void {
  const handle = connections.get(name);
  if (handle) handle.close();
}

// ── Backward-compatible singleton wrappers ──

export function connectSSE(entityType?: string, entityId?: string): EventSource {
  const handle = createSSEConnection('default', entityType, entityId);
  return handle.source;
}

export function disconnectSSE(): void {
  closeSSEConnection('default');
}
