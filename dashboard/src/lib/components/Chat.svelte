<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { connectSSE, disconnectSSE, type StateChangeEvent } from '$lib/sse';
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
  let sessionStatus = $state<string>('idle');
  let sending = $state(false);
  let chatContainer: HTMLDivElement | undefined = $state();
  let eventSource: EventSource | null = null;

  // Add greeting on mount
  onMount(() => {
    if (greeting) {
      messages = [{ role: 'assistant', content: greeting }];
    }
  });

  onDestroy(() => {
    if (eventSource) {
      eventSource.close();
      eventSource = null;
    }
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
    if (!text || sending) return;

    input = '';
    sending = true;
    messages = [...messages, { role: 'user', content: text }];
    scrollToBottom();

    try {
      if (!sessionId) {
        // First message — create a new session
        const { session_id } = await createSession({
          agent_id: agentId,
          user_message: text,
          system_prompt: systemPrompt,
        });
        sessionId = session_id;
        sessionStatus = 'running';

        // Start watching this session's events
        watchSession(session_id);
      } else {
        // Follow-up message — steer the existing session
        await steerSession(sessionId, text);
      }
    } catch (err) {
      messages = [...messages, {
        role: 'system',
        content: `Error: ${err instanceof Error ? err.message : 'Failed to send message'}`
      }];
    } finally {
      sending = false;
      scrollToBottom();
    }
  }

  function watchSession(sid: string) {
    // Use SSE to watch for state changes on this session
    eventSource = connectSSE('Session', sid);

    // Poll for result periodically since SSE gives state changes, not content
    const pollInterval = setInterval(async () => {
      try {
        const entity = await getEntity('Sessions', sid);
        const status = (entity as any).Status || (entity as any)._status || '';
        sessionStatus = status;

        if (status === 'Completed') {
          const result = (entity as any).result || '';
          if (result) {
            messages = [...messages, { role: 'assistant', content: result }];
            scrollToBottom();
          }
          onComplete?.(result);
          clearInterval(pollInterval);
          sending = false;
        } else if (status === 'Failed') {
          const error = (entity as any).error_message || 'Session failed';
          messages = [...messages, { role: 'system', content: `Error: ${error}` }];
          scrollToBottom();
          clearInterval(pollInterval);
          sending = false;
        }
      } catch {
        // Ignore polling errors
      }
    }, 2000);

    // Clean up on destroy
    const origDestroy = onDestroy;
    onDestroy(() => {
      clearInterval(pollInterval);
    });
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      send();
    }
  }
</script>

<div class="chat">
  <div class="chat-messages" bind:this={chatContainer}>
    {#each messages as msg}
      <div class="chat-msg chat-msg--{msg.role}">
        <span class="chat-sender">{msg.role === 'user' ? 'You' : msg.role === 'assistant' ? 'Paw' : 'System'}</span>
        <div class="chat-bubble">{msg.content}</div>
      </div>
    {/each}
    {#if sending && sessionStatus === 'running'}
      <div class="chat-msg chat-msg--assistant">
        <span class="chat-sender">Paw</span>
        <div class="chat-bubble chat-bubble--typing">Thinking...</div>
      </div>
    {/if}
  </div>

  <div class="chat-input-row">
    <textarea
      class="chat-input"
      bind:value={input}
      onkeydown={handleKeydown}
      placeholder="Type a message..."
      rows="1"
      disabled={sending}
    ></textarea>
    <button
      class="chat-send"
      onclick={send}
      disabled={!input.trim() || sending}
      type="button"
    >Send</button>
  </div>
</div>

<style>
  .chat {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .chat-messages {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-4);
  }

  .chat-msg {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-width: 85%;
  }

  .chat-msg--user {
    align-self: flex-end;
  }

  .chat-msg--assistant,
  .chat-msg--system {
    align-self: flex-start;
  }

  .chat-sender {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-3);
    padding: 0 var(--sp-1);
  }

  .chat-bubble {
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--radius);
    font-size: var(--text-base);
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .chat-msg--user .chat-bubble {
    background: var(--accent);
    color: var(--bg);
  }

  .chat-msg--assistant .chat-bubble {
    background: var(--surface);
    color: var(--text-1);
    border: 1px solid var(--border);
  }

  .chat-msg--system .chat-bubble {
    background: rgba(255, 104, 104, 0.12);
    color: var(--status-error);
    font-size: var(--text-sm);
  }

  .chat-bubble--typing {
    color: var(--text-3);
    font-style: italic;
  }

  .chat-input-row {
    display: flex;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-4);
    border-top: 1px solid var(--border);
  }

  .chat-input {
    flex: 1;
    resize: none;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-1);
    padding: var(--sp-2) var(--sp-3);
    font-size: var(--text-base);
    font-family: inherit;
  }

  .chat-input:disabled {
    opacity: 0.5;
  }

  .chat-send {
    padding: var(--sp-2) var(--sp-4);
    border-radius: var(--radius);
    border: none;
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
    font-size: var(--text-sm);
    cursor: pointer;
    white-space: nowrap;
  }

  .chat-send:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
