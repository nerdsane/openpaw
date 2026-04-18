<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import PawLogo from './PawLogo.svelte';
  import ChatExecutionTrace from './ChatExecutionTrace.svelte';
  import {
    pawChat,
    sendMessage,
    clearConversation,
    closePanel,
    discoverPawAgent,
    restoreFromServer,
    type PawMessage,
  } from '$lib/stores/paw-chat';

  let input = $state('');
  let sending = $state(false);
  let bodyEl = $state<HTMLDivElement | null>(null);
  let inputEl = $state<HTMLTextAreaElement | null>(null);

  // Subscribe to store
  let panelState = $state({
    sessionId: null as string | null,
    agentId: null as string | null,
    messages: [] as PawMessage[],
    status: 'idle' as string,
    panelOpen: false,
  });

  const unsub = pawChat.subscribe((s) => { panelState = s; });
  onDestroy(unsub);

  // Auto-scroll tracking
  let userScrolledUp = $state(false);

  function onBodyScroll() {
    if (!bodyEl) return;
    const distFromBottom = bodyEl.scrollHeight - bodyEl.scrollTop - bodyEl.clientHeight;
    userScrolledUp = distFromBottom > 100;
  }

  function scrollToBottom() {
    if (!bodyEl || userScrolledUp) return;
    requestAnimationFrame(() => {
      bodyEl!.scrollTop = bodyEl!.scrollHeight;
    });
  }

  // Scroll when messages change
  $effect(() => {
    panelState.messages;
    scrollToBottom();
  });

  // Focus input when panel opens
  $effect(() => {
    if (panelState.panelOpen && inputEl) {
      requestAnimationFrame(() => inputEl!.focus());
    }
  });

  onMount(async () => {
    await discoverPawAgent();
    await restoreFromServer();
  });

  async function handleSend() {
    const text = input.trim();
    if (!text || sending || panelState.status === 'thinking' || panelState.status === 'executing') return;

    input = '';
    sending = true;
    try {
      await sendMessage(text);
    } finally {
      sending = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      handleSend();
    }
  }

  function handleClear() {
    clearConversation();
    input = '';
  }

  let isActive = $derived(
    panelState.status === 'thinking' || panelState.status === 'executing' || panelState.status === 'compacting'
  );

  function escapeHtml(s: string): string {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  }

  function renderMarkdown(text: string): string {
    // Extract code blocks first, escape their content separately
    const codeBlocks: string[] = [];
    let safe = text.replace(/```([\s\S]*?)```/g, (_m, code: string) => {
      const idx = codeBlocks.length;
      codeBlocks.push(`<pre class="msg-code">${escapeHtml(code.replace(/^\w*\n/, ''))}</pre>`);
      return `\x00CB${idx}\x00`;
    });

    // Escape remaining HTML
    safe = escapeHtml(safe);

    // Apply inline markdown on escaped text
    safe = safe
      .replace(/`([^`]+)`/g, '<code class="msg-inline-code">$1</code>')
      .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
      .replace(/\n/g, '<br>');

    // Restore code blocks
    safe = safe.replace(/\x00CB(\d+)\x00/g, (_m, idx: string) => codeBlocks[parseInt(idx)]);

    return safe;
  }
</script>

{#if panelState.panelOpen}
  <!-- Backdrop on mobile -->
  <button class="backdrop" onclick={closePanel} aria-label="Close chat panel"></button>

  <div class="panel" role="dialog" aria-label="Chat with Paw">
    <!-- Header -->
    <div class="panel-header">
      <div class="panel-title">
        <PawLogo size={16} />
        <span>PAW</span>
        {#if isActive}
          <span class="pulse-dot"></span>
        {/if}
      </div>
      <div class="panel-actions">
        {#if panelState.messages.length > 0}
          <button class="panel-btn" onclick={handleClear} title="Clear conversation">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M3 6h18M8 6V4h8v2M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/></svg>
          </button>
        {/if}
        <button class="panel-btn" onclick={closePanel} title="Close (Esc)">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M18 6L6 18M6 6l12 12"/></svg>
        </button>
      </div>
    </div>

    <!-- Messages -->
    <div class="panel-body" bind:this={bodyEl} onscroll={onBodyScroll} role="log" aria-live="polite">
      {#if panelState.messages.length === 0}
        <div class="empty">
          <PawLogo size={32} />
          <p class="empty-text">Ask Paw anything about TemperPaw, your agents, or your setup.</p>
        </div>
      {:else}
        {#each panelState.messages as msg (msg.id)}
          <div
            class="msg"
            class:msg-user={msg.role === 'user'}
            class:msg-err={msg.role === 'system'}
            class:msg-assistant={msg.role === 'assistant'}
          >
            {#if msg.role !== 'user'}
              <span class="msg-from">{msg.role === 'assistant' ? 'PAW' : 'ERROR'}</span>
            {/if}

            {#if msg.role === 'assistant' && msg.status && msg.status !== 'completed'}
              <!-- In-progress: show execution trace -->
              <div class="msg-body">
                <ChatExecutionTrace
                  turns={msg.turns ?? []}
                  status={msg.status}
                />
              </div>
            {:else if msg.role === 'assistant' && msg.content}
              <!-- Completed: show result + optional trace -->
              <div class="msg-body">
                {@html renderMarkdown(msg.content)}
              </div>
              {#if msg.turns && msg.turns.length > 0}
                <ChatExecutionTrace
                  turns={msg.turns}
                  status="completed"
                />
              {/if}
            {:else if msg.role === 'system'}
              <div class="msg-body">{msg.content}</div>
            {:else}
              <div class="msg-body">{msg.content}</div>
            {/if}

            {#if msg.errorMessage}
              <div class="msg-error-detail">{msg.errorMessage}</div>
            {/if}
          </div>
        {/each}
      {/if}
    </div>

    <!-- Input -->
    <form class="panel-input" onsubmit={(e) => { e.preventDefault(); handleSend(); }}>
      <textarea
        class="input-field"
        bind:this={inputEl}
        bind:value={input}
        onkeydown={handleKeydown}
        placeholder={panelState.agentId ? 'Message Paw...' : 'Setting up...'}
        disabled={!panelState.agentId || isActive}
        rows="1"
        autocomplete="off"
      ></textarea>
      <button
        class="send-btn"
        type="submit"
        disabled={!input.trim() || !panelState.agentId || isActive}
        aria-label="Send message"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 2L11 13M22 2l-7 20-4-9-9-4 20-7z"/></svg>
      </button>
    </form>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.3);
    z-index: 49;
    display: none;
    border: none;
    cursor: default;
  }

  .panel {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: 420px;
    max-width: 100vw;
    background: var(--bg);
    border-left: 1px solid var(--border);
    z-index: 50;
    display: flex;
    flex-direction: column;
    animation: slide-in 200ms var(--ease) both;
  }

  @keyframes slide-in {
    from { transform: translateX(100%); }
  }

  /* ── Header ── */
  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .panel-title {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--text-2);
  }

  .pulse-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent, #4ade80);
    animation: pulse 1.5s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .panel-actions {
    display: flex;
    gap: var(--sp-1);
  }

  .panel-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: var(--radius);
    color: var(--text-3);
    cursor: pointer;
  }

  .panel-btn:hover {
    color: var(--text-1);
    background: var(--accent-subtle);
  }

  /* ── Body ── */
  .panel-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: var(--sp-4);
  }

  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--sp-3);
    color: var(--text-3);
    text-align: center;
    padding: var(--sp-8);
  }

  .empty-text {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    line-height: 1.6;
    max-width: 260px;
    margin: 0;
  }

  /* ── Messages ── */
  .msg {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-width: 95%;
    animation: msg-in 150ms var(--ease) both;
  }

  @keyframes msg-in {
    from { opacity: 0; transform: translateY(8px); }
  }

  .msg-user {
    align-self: flex-end;
  }

  .msg-from {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.06em;
    color: var(--text-3);
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
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .msg-err .msg-body {
    color: var(--status-error);
    border-color: rgba(255, 104, 104, 0.2);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .msg-error-detail {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--status-error);
    padding: 0 var(--sp-1);
    opacity: 0.7;
  }

  /* Markdown rendering */
  .msg-body :global(pre.msg-code) {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    background: var(--terminal-bg, rgba(0,0,0,0.1));
    padding: var(--sp-2);
    border-radius: var(--radius);
    overflow-x: auto;
    margin: var(--sp-1) 0;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .msg-body :global(code.msg-inline-code) {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    background: var(--terminal-bg, rgba(0,0,0,0.1));
    padding: 1px var(--sp-1);
    border-radius: 3px;
  }

  /* ── Input ── */
  .panel-input {
    display: flex;
    align-items: flex-end;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-4);
    border-top: 1px solid var(--border);
    flex-shrink: 0;
  }

  .input-field {
    flex: 1;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-1);
    padding: var(--sp-2) var(--sp-3);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    resize: none;
    min-height: 36px;
    max-height: 120px;
    line-height: 1.4;
    field-sizing: content;
  }

  .input-field::placeholder {
    color: var(--text-3);
  }

  .input-field:focus {
    outline: none;
    border-color: var(--text-2);
  }

  .input-field:disabled {
    opacity: 0.5;
  }

  .send-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border-radius: var(--radius);
    border: 1px solid var(--text-1);
    background: var(--text-1);
    color: var(--bg);
    cursor: pointer;
    flex-shrink: 0;
  }

  .send-btn:disabled {
    opacity: 0.2;
    cursor: not-allowed;
  }

  .send-btn:not(:disabled):hover {
    opacity: 0.85;
  }

  /* ── Mobile ── */
  @media (max-width: 640px) {
    .backdrop {
      display: block;
    }

    .panel {
      width: 100vw;
    }

    .panel-input {
      padding-bottom: calc(var(--sp-3) + env(safe-area-inset-bottom, 0px));
    }

    .input-field {
      font-size: 16px;
      min-height: 44px;
    }

    .send-btn {
      width: 44px;
      height: 44px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .panel { animation: none; }
    .msg { animation: none; }
    .pulse-dot { animation: none; }
  }
</style>
