import { writable, get } from 'svelte/store';
import { createSession, steerSession, getEntity, queryEntities } from '$lib/api';
import { createSSEConnection, closeSSEConnection, type StateChangeEvent } from '$lib/sse';
import { eventsToTurns, type SessionTurn } from '$lib/parse';
import type { EntityEvent } from '$lib/types';

export interface PawMessage {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  timestamp: string;
  sessionId: string;
  turns?: SessionTurn[];
  status?: 'thinking' | 'executing' | 'completed' | 'failed';
  errorMessage?: string;
}

export type PawStatus = 'idle' | 'thinking' | 'executing' | 'compacting' | 'error';

interface PawChatState {
  sessionId: string | null;
  agentId: string | null;
  messages: PawMessage[];
  status: PawStatus;
  panelOpen: boolean;
}

const STORAGE_KEY = 'paw_chat_state';

const isBrowser = typeof window !== 'undefined';

function loadFromStorage(): Partial<PawChatState> {
  if (!isBrowser) return {};
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    return JSON.parse(raw);
  } catch {
    return {};
  }
}

function saveToStorage(state: PawChatState): void {
  if (!isBrowser) return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({
      sessionId: state.sessionId,
      agentId: state.agentId,
      messages: state.messages,
    }));
  } catch {
    // localStorage may be full or unavailable
  }
}

const saved = loadFromStorage();
const initialState: PawChatState = {
  sessionId: saved.sessionId ?? null,
  agentId: saved.agentId ?? null,
  messages: saved.messages ?? [],
  status: 'idle',
  panelOpen: saved.panelOpen ?? false,
};

export const pawChat = writable<PawChatState>(initialState);

// Auto-persist on change (debounced)
let saveTimer: ReturnType<typeof setTimeout> | null = null;
pawChat.subscribe((state) => {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => saveToStorage(state), 300);
});

// ── Actions ──

function genId(): string {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
}

function statusFromEntity(entityStatus: string): PawStatus {
  switch (entityStatus) {
    case 'Thinking': return 'thinking';
    case 'Executing': return 'executing';
    case 'Compacting': return 'compacting';
    case 'Completed': return 'idle';
    case 'Failed':
    case 'Cancelled': return 'error';
    default: return 'thinking';
  }
}

function mapStatusToMessage(entityStatus: string): PawMessage['status'] {
  switch (entityStatus) {
    case 'Completed': return 'completed';
    case 'Failed':
    case 'Cancelled': return 'failed';
    default: return 'executing';
  }
}

/** Connect SSE to the active session and update state in real-time */
function connectToSession(sessionId: string): void {
  createSSEConnection('paw-chat', 'Session', sessionId, (event: StateChangeEvent) => {
    if (event.entity_id !== sessionId) return;

    const newStatus = statusFromEntity(event.status);

    pawChat.update((s) => {
      const messages = [...s.messages];
      const lastMsg = messages[messages.length - 1];
      if (lastMsg && lastMsg.role === 'assistant' && lastMsg.status !== 'completed') {
        messages[messages.length - 1] = { ...lastMsg, status: mapStatusToMessage(event.status) };
      }

      return { ...s, status: newStatus, messages };
    });

    // On state changes, refetch the entity to get updated events and result
    if (event.action === 'ProcessToolCalls' || event.action === 'HandleToolResults') {
      refetchSessionEvents(sessionId);
    }

    // Terminal states
    if (event.status === 'Completed' || event.status === 'Failed' || event.status === 'Cancelled') {
      closeSSEConnection('paw-chat');
      finishSession(sessionId, event.status);
    }
  }, () => {
    // SSE connection permanently closed — fall back to polling the entity once
    finishSession(sessionId, 'Failed');
  });
}

/** Refetch entity to get latest _events for tool call display */
async function refetchSessionEvents(sessionId: string): Promise<void> {
  try {
    const entity = await getEntity('Sessions', sessionId);
    const rawEvents = (entity._events ?? []) as EntityEvent[];
    const turns = eventsToTurns(rawEvents);

    pawChat.update((s) => {
      const messages = [...s.messages];
      const lastMsg = messages[messages.length - 1];
      if (lastMsg && lastMsg.role === 'assistant' && lastMsg.status !== 'completed') {
        messages[messages.length - 1] = { ...lastMsg, turns };
      }
      return { ...s, messages };
    });
  } catch {
    // Ignore refetch errors
  }
}

/** Handle terminal session state — extract result or error */
async function finishSession(sessionId: string, entityStatus: string): Promise<void> {
  try {
    const entity = await getEntity('Sessions', sessionId);
    const result = (entity.result as string) || '';
    const errorMessage = (entity.error_message as string) || '';
    const rawEvents = (entity._events ?? []) as EntityEvent[];
    const turns = eventsToTurns(rawEvents);

    pawChat.update((s) => {
      const messages = [...s.messages];
      const lastMsg = messages[messages.length - 1];

      if (lastMsg && lastMsg.role === 'assistant') {
        messages[messages.length - 1] = {
          ...lastMsg,
          content: result || lastMsg.content,
          turns,
          status: entityStatus === 'Completed' ? 'completed' : 'failed',
          errorMessage: errorMessage || undefined,
        };
      } else if (entityStatus === 'Completed' && result) {
        messages.push({
          id: genId(),
          role: 'assistant',
          content: result,
          timestamp: new Date().toISOString(),
          sessionId,
          turns,
          status: 'completed',
        });
      } else if (entityStatus === 'Failed' || entityStatus === 'Cancelled') {
        messages.push({
          id: genId(),
          role: 'system',
          content: errorMessage || 'Session failed',
          timestamp: new Date().toISOString(),
          sessionId,
          status: 'failed',
          errorMessage,
        });
      }

      return {
        ...s,
        messages,
        status: entityStatus === 'Completed' ? 'idle' : 'error',
      };
    });
  } catch {
    pawChat.update((s) => ({ ...s, status: 'error' }));
  }
}

export async function sendMessage(text: string): Promise<void> {
  const state = get(pawChat);
  if (!state.agentId) return;

  const userMsg: PawMessage = {
    id: genId(),
    role: 'user',
    content: text,
    timestamp: new Date().toISOString(),
    sessionId: state.sessionId || '',
  };

  // Add a placeholder assistant message for streaming status
  const assistantMsg: PawMessage = {
    id: genId(),
    role: 'assistant',
    content: '',
    timestamp: new Date().toISOString(),
    sessionId: state.sessionId || '',
    status: 'thinking',
    turns: [],
  };

  pawChat.update((s) => ({
    ...s,
    messages: [...s.messages, userMsg, assistantMsg],
    status: 'thinking',
  }));

  try {
    if (!state.sessionId) {
      // Create new session
      const { session_id } = await createSession({
        agent_id: state.agentId,
        user_message: text,
      });

      // Update sessionId on both the messages and state
      pawChat.update((s) => {
        const messages = s.messages.map((m) =>
          m.sessionId === '' ? { ...m, sessionId: session_id } : m
        );
        return { ...s, sessionId: session_id, messages };
      });

      connectToSession(session_id);
    } else {
      // Steer existing session
      try {
        await steerSession(state.sessionId, text);
        connectToSession(state.sessionId);
      } catch {
        // Session may be in a terminal state — create a new one
        const { session_id } = await createSession({
          agent_id: state.agentId,
          user_message: text,
        });

        pawChat.update((s) => {
          const messages = s.messages.map((m) =>
            m.sessionId === '' ? { ...m, sessionId: session_id } : m
          );
          return { ...s, sessionId: session_id, messages };
        });

        connectToSession(session_id);
      }
    }
  } catch (err) {
    pawChat.update((s) => {
      // Remove the placeholder assistant message, add error
      const messages = s.messages.filter((m) => m.id !== assistantMsg.id);
      messages.push({
        id: genId(),
        role: 'system',
        content: err instanceof Error ? err.message : 'Failed to send message',
        timestamp: new Date().toISOString(),
        sessionId: state.sessionId || '',
      });
      return { ...s, messages, status: 'error' };
    });
  }
}

export function clearConversation(): void {
  closeSSEConnection('paw-chat');
  pawChat.update((s) => ({
    ...s,
    sessionId: null,
    messages: [],
    status: 'idle',
  }));
}

export function togglePanel(): void {
  pawChat.update((s) => ({ ...s, panelOpen: !s.panelOpen }));
}

export function openPanel(): void {
  pawChat.update((s) => ({ ...s, panelOpen: true }));
}

export function closePanel(): void {
  pawChat.update((s) => ({ ...s, panelOpen: false }));
}

/** Discover the Paw agent ID and cache it in the store */
export async function discoverPawAgent(): Promise<void> {
  const state = get(pawChat);
  if (state.agentId) return; // Already discovered

  try {
    let agents = await queryEntities('Agents', "name eq 'Paw'");
    if (agents.length === 0) {
      agents = await queryEntities('Agents');
    }
    if (agents.length > 0) {
      const agentId = (agents[0] as Record<string, unknown>)._entity_id as string
        || (agents[0] as Record<string, unknown>).Id as string;
      pawChat.update((s) => ({ ...s, agentId }));
    }
  } catch {
    // Agent may not exist yet
  }
}

/** Restore session state from server on page load */
export async function restoreFromServer(): Promise<void> {
  const state = get(pawChat);
  if (!state.sessionId) return;

  try {
    const entity = await getEntity('Sessions', state.sessionId);
    const entityStatus = (entity.Status as string) || '';

    // If session is still active, reconnect SSE
    if (!['Completed', 'Failed', 'Cancelled'].includes(entityStatus)) {
      pawChat.update((s) => ({ ...s, status: statusFromEntity(entityStatus) }));
      connectToSession(state.sessionId);
    } else {
      // Session is terminal — make sure we have the result
      const result = (entity.result as string) || '';
      const lastMsg = state.messages[state.messages.length - 1];
      if (lastMsg && lastMsg.role === 'assistant' && !lastMsg.content && result) {
        pawChat.update((s) => {
          const messages = [...s.messages];
          messages[messages.length - 1] = {
            ...lastMsg,
            content: result,
            status: 'completed',
          };
          return { ...s, messages, status: 'idle' };
        });
      }
    }
  } catch {
    // Session no longer exists — clear it but keep messages
    pawChat.update((s) => ({ ...s, sessionId: null, status: 'idle' }));
  }
}
