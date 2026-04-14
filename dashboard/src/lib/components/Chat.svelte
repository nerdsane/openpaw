<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getEntity, createSession, steerSession } from '$lib/api';

  interface Props {
    agentId: string;
    systemPrompt?: string;
    greeting?: string;
    onComplete?: (result: string) => void;
  }

  let { agentId, systemPrompt = '', greeting = '', onComplete }: Props = $props();

  interface ChatMessage {
    role: 'user' | 'assistant' | 'system';
    content: string;
  }

  let messages = $state<ChatMessage[]>([]);
  let input = $state('');
  let sessionId = $state<string | null>(null);
  let waiting = $state(false);
  let chatContainer: HTMLDivElement | undefined = $state();
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  onMount(() => {
    if (greeting) {
      messages = [{ role: 'assistant', content: greeting }];
    }
  });

  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer);
  });

  function scrollToBottom() {
    if (chatContainer) {
      requestAnimationFrame(() => {
        chatContainer!.scrollTop = chatContainer!.scrollHeight;
      });
    }
  }

  async function send() {
    const text = input.trim();
    if (!text || waiting) return;

    input = '';
    waiting = true;
    messages = [...messages, { role: 'user', content: text }];
    scrollToBottom();

    try {
      if (!sessionId) {
        const { session_id } = await createSession({
          agent_id: agentId,
          user_message: text,
          system_prompt: systemPrompt,
        });
        sessionId = session_id;
        pollForResult(session_id);
      } else {
        await steerSession(sessionId, text);
        pollForResult(sessionId);
      }
    } catch (err) {
      messages = [...messages, {
        role: 'system',
        content: err instanceof Error ? err.message : 'Failed to send message'
      }];
      waiting = false;
    }
    scrollToBottom();
  }

  function pollForResult(sid: string) {
    if (pollTimer) clearInterval(pollTimer);

    pollTimer = setInterval(async () => {
      try {
        const entity = await getEntity('Sessions', sid);
        const status = (entity as any).Status || '';

        if (status === 'Completed') {
          if (pollTimer) clearInterval(pollTimer);
          const result = (entity as any).result || '';
          if (result) {
            messages = [...messages, { role: 'assistant', content: result }];
            scrollToBottom();
          }
          waiting = false;
          sessionId = null; // allow next message to start a fresh session
          onComplete?.(result);
        } else if (status === 'Failed' || status === 'Cancelled') {
          if (pollTimer) clearInterval(pollTimer);
          const error = (entity as any).error_message || 'Session failed';
          messages = [...messages, { role: 'system', content: error }];
          scrollToBottom();
          waiting = false;
          sessionId = null; // allow retry with a fresh session
        }
      } catch {
        // ignore polling errors
      }
    }, 2000);
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      send();
    }
  }
</script>

<div class="chat">
  <div class="chat-log" bind:this={chatContainer}>
    {#each messages as msg}
      <div class="msg" class:msg-user={msg.role === 'user'} class:msg-err={msg.role === 'system'}>
        <span class="msg-from">{msg.role === 'user' ? 'You' : msg.role === 'assistant' ? 'Paw' : 'Error'}</span>
        <div class="msg-body">{msg.content}</div>
      </div>
    {/each}
    {#if waiting}
      <div class="msg">
        <span class="msg-from">Paw</span>
        <div class="msg-body msg-thinking">thinking...</div>
      </div>
    {/if}
  </div>

  <form class="chat-bar" onsubmit={(e) => { e.preventDefault(); send(); }}>
    <input
      class="chat-input"
      bind:value={input}
      onkeydown={handleKeydown}
      placeholder="Message Paw..."
      disabled={waiting}
      autocomplete="off"
    />
    <button class="chat-send" disabled={!input.trim() || waiting} type="submit">SEND</button>
  </form>
</div>

<style>
  .chat {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .chat-log {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-6) var(--sp-4);
  }

  .msg {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-width: 90%;
  }

  .msg-user {
    align-self: flex-end;
  }

  .msg-from {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.06em;
    color: var(--text-3);
    text-transform: uppercase;
    padding: 0 var(--sp-1);
  }

  .msg-body {
    font-family: var(--font-sans);
    font-size: var(--text-sm);
    line-height: 1.6;
    color: var(--text-1);
    white-space: pre-wrap;
    word-break: break-word;
    padding: var(--sp-2) var(--sp-3);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .msg-user .msg-body {
    background: var(--text-1);
    color: var(--bg);
    border-color: transparent;
  }

  .msg-err .msg-body {
    color: var(--status-error);
    border-color: rgba(255, 104, 104, 0.2);
  }

  .msg-thinking {
    color: var(--text-3);
    font-style: italic;
    border-style: dashed;
  }

  .chat-bar {
    display: flex;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-4);
    border-top: 1px solid var(--border);
  }

  .chat-input {
    flex: 1;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-1);
    padding: var(--sp-2) var(--sp-3);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }

  .chat-input::placeholder {
    color: var(--text-3);
  }

  .chat-input:focus {
    outline: none;
    border-color: var(--text-2);
  }

  .chat-input:disabled {
    opacity: 0.5;
  }

  .chat-send {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.08em;
    padding: var(--sp-2) var(--sp-4);
    border-radius: var(--radius);
    border: 1px solid var(--text-1);
    background: var(--text-1);
    color: var(--bg);
    font-weight: 600;
    cursor: pointer;
  }

  .chat-send:disabled {
    opacity: 0.25;
    cursor: not-allowed;
  }

  @media (max-width: 640px) {
    .chat-log { padding: var(--sp-3) var(--sp-2); gap: var(--sp-3); }
    .chat-bar { padding: var(--sp-2); padding-bottom: calc(var(--sp-2) + env(safe-area-inset-bottom, 0px)); }
    .chat-send { padding: var(--sp-2) var(--sp-3); min-height: 44px; }
    .chat-input { min-height: 44px; font-size: 16px; }
    .msg { max-width: 95%; }
    .msg-body { font-size: var(--text-base); }
  }
</style>
