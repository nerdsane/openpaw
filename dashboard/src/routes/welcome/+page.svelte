<script lang="ts">
  import { onMount } from 'svelte';
  import { base } from '$app/paths';
  import {
    fetchSetupStatus,
    saveSecret,
    getSecret,
    type SetupStatus,
  } from '$lib/api';
  import { openPanel } from '$lib/stores/paw-chat';

  let status = $state<SetupStatus | null>(null);
  let loading = $state(true);
  let error = $state('');

  // LLM setup
  let llmProvider = $state('anthropic');
  let llmKey = $state('');
  let savingKey = $state(false);
  let keyError = $state('');
  let showLlmForm = $state(false);

  const providerKeyMap: Record<string, string> = {
    anthropic: 'anthropic_api_key',
    openai: 'openai_api_key',
    openai_codex: 'openai_codex_token',
    openrouter: 'openrouter_api_key',
  };

  let setupComplete = $derived(
    status?.has_anthropic_key && status?.has_agents && status?.discord_connected
  );

  let completedSteps = $derived.by(() => {
    if (!status) return 0;
    let n = 0;
    if (status.has_anthropic_key) n++;
    if (status.discord_connected) n++;
    if (status.has_agents) n++;
    return n;
  });

  onMount(async () => {
    try {
      status = await fetchSetupStatus();

      if (status.has_anthropic_key) {
        // Detect active provider
        const savedProvider = await getSecret('llm_provider');
        if (savedProvider) llmProvider = savedProvider;
      } else {
        showLlmForm = true;
      }
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load setup status';
    } finally {
      loading = false;
    }
  });

  async function saveLlmKey() {
    const key = llmKey.trim();
    if (!key) return;
    savingKey = true;
    keyError = '';
    try {
      const secretKey = providerKeyMap[llmProvider];
      await saveSecret(secretKey, key);
      await saveSecret('llm_provider', llmProvider);
      status = await fetchSetupStatus();
      showLlmForm = false;
      llmKey = '';
    } catch (err) {
      keyError = err instanceof Error ? err.message : 'Failed to save API key';
    } finally {
      savingKey = false;
    }
  }

  async function refresh() {
    status = await fetchSetupStatus();
  }
</script>

<svelte:head>
  <title>Setup &middot; Open Paw</title>
</svelte:head>

<div class="page">
  {#if loading}
    <div class="center">
      <p class="dim">Checking setup...</p>
    </div>
  {:else if error}
    <div class="center">
      <p class="err">{error}</p>
    </div>
  {:else}
    <div class="header">
      <h1 class="title">{setupComplete ? 'Open Paw' : 'Setup'}</h1>
      <p class="subtitle">{setupComplete
        ? 'Your instance is ready.'
        : `${completedSteps} of 3 steps complete. Configure below or ask Paw for help.`}</p>
    </div>

    <!-- Setup checklist -->
    <div class="checklist">
      <!-- 1. LLM -->
      <div class="step" class:step-done={status?.has_anthropic_key}>
        <div class="step-row">
          <span class="step-dot" class:step-dot--on={status?.has_anthropic_key}></span>
          <div class="step-info">
            <span class="step-name">LLM Provider</span>
            <span class="step-status">
              {#if status?.has_anthropic_key}
                {status.llm_provider || 'Configured'}
              {:else}
                Not configured
              {/if}
            </span>
          </div>
          {#if status?.has_anthropic_key}
            <button class="step-act" onclick={() => showLlmForm = !showLlmForm}>
              {showLlmForm ? 'Close' : 'Change'}
            </button>
          {:else}
            <button class="step-act step-act--primary" onclick={() => showLlmForm = !showLlmForm}>
              {showLlmForm ? 'Close' : 'Configure'}
            </button>
          {/if}
        </div>
        {#if showLlmForm}
          <form class="step-form" onsubmit={(e) => { e.preventDefault(); saveLlmKey(); }}>
            <div class="tab-row">
              <button class="tab" class:tab-active={llmProvider === 'anthropic'} onclick={() => llmProvider = 'anthropic'} type="button">Anthropic</button>
              <button class="tab" class:tab-active={llmProvider === 'openai'} onclick={() => llmProvider = 'openai'} type="button">OpenAI</button>
              <button class="tab" class:tab-active={llmProvider === 'openai_codex'} onclick={() => llmProvider = 'openai_codex'} type="button">Codex</button>
              <button class="tab" class:tab-active={llmProvider === 'openrouter'} onclick={() => llmProvider = 'openrouter'} type="button">OpenRouter</button>
            </div>
            <div class="input-row">
              <input type="password" bind:value={llmKey} placeholder="API key or token" disabled={savingKey} />
              <button class="btn" type="submit" disabled={!llmKey.trim() || savingKey}>
                {savingKey ? '...' : 'Save'}
              </button>
            </div>
            {#if keyError}
              <p class="err">{keyError}</p>
            {/if}
          </form>
        {/if}
      </div>

      <!-- 2. Discord -->
      <div class="step" class:step-done={status?.discord_connected}>
        <div class="step-row">
          <span class="step-dot" class:step-dot--on={status?.discord_connected}></span>
          <div class="step-info">
            <span class="step-name">Discord</span>
            <span class="step-status">
              {status?.discord_connected ? 'Connected' : 'Not connected'}
            </span>
          </div>
          <a class="step-act" href="{base}/settings">
            {status?.discord_connected ? 'Settings' : 'Configure'}
          </a>
        </div>
        {#if !status?.discord_connected}
          <p class="step-hint">Add your Discord bot token and guild ID in Settings, then click Connect.</p>
        {/if}
      </div>

      <!-- 3. Agents -->
      <div class="step" class:step-done={status?.has_agents}>
        <div class="step-row">
          <span class="step-dot" class:step-dot--on={status?.has_agents}></span>
          <div class="step-info">
            <span class="step-name">Agents</span>
            <span class="step-status">
              {#if status?.has_agents}
                {status.agent_count} active
              {:else}
                None running
              {/if}
            </span>
          </div>
          {#if !status?.has_agents}
            <button class="step-act" onclick={refresh}>Refresh</button>
          {:else}
            <a class="step-act" href="{base}/">Canvas</a>
          {/if}
        </div>
        {#if !status?.has_agents && status?.has_anthropic_key}
          <p class="step-hint">Agents bootstrap automatically when an LLM is configured. Try refreshing.</p>
        {/if}
      </div>
    </div>

    <!-- Actions -->
    <div class="actions">
      {#if setupComplete}
        <a class="btn" href="{base}/">Open Dashboard</a>
      {/if}
      <button class="btn-ghost" onclick={openPanel}>
        Ask Paw for help
      </button>
    </div>

    <!-- Footer links -->
    <div class="footer">
      <a href="{base}/" class="footer-link">Dashboard</a>
      <a href="{base}/settings" class="footer-link">Settings</a>
      <a href="{base}/sessions" class="footer-link">Sessions</a>
    </div>
  {/if}
</div>

<style>
  .page {
    max-width: 520px;
    margin: 0 auto;
    padding: var(--sp-6) var(--sp-4);
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
  }

  .center {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 200px;
    gap: var(--sp-3);
    text-align: center;
  }

  .header {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }

  .title {
    font-family: var(--font-sans);
    font-size: var(--text-xl);
    font-weight: 500;
    color: var(--text-1);
    margin: 0;
  }

  .subtitle {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    margin: 0;
  }

  .dim {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .err {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--status-error);
    margin: 0;
  }

  /* ── Checklist ── */
  .checklist {
    display: flex;
    flex-direction: column;
  }

  .step {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding: var(--sp-3) 0;
    border-top: 1px solid var(--border);
  }

  .step:first-child {
    border-top: none;
    padding-top: 0;
  }

  .step-row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }

  .step-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-3);
    opacity: 0.3;
    flex-shrink: 0;
  }

  .step-dot--on {
    background: var(--accent);
    opacity: 1;
  }

  .step-info {
    flex: 1;
    display: flex;
    align-items: baseline;
    gap: var(--sp-2);
    min-width: 0;
  }

  .step-name {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text-1);
    white-space: nowrap;
  }

  .step-status {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .step-act {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    background: none;
    border: none;
    cursor: pointer;
    padding: var(--sp-1) var(--sp-2);
    border-radius: var(--radius);
    text-decoration: none;
    white-space: nowrap;
    transition: color var(--duration) var(--ease);
  }

  .step-act:hover { color: var(--text-1); }

  .step-act--primary {
    color: var(--bg);
    background: var(--text-1);
    font-weight: 600;
    letter-spacing: 0.06em;
  }

  .step-act--primary:hover { opacity: 0.85; color: var(--bg); }

  .step-hint {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    padding-left: calc(8px + var(--sp-2));
    margin: 0;
    line-height: 1.5;
  }

  /* ── Step form (LLM) ── */
  .step-form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    padding-left: calc(8px + var(--sp-2));
  }

  .tab-row {
    display: flex;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }

  .tab {
    flex: 1;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.04em;
    padding: var(--sp-1) var(--sp-2);
    background: transparent;
    color: var(--text-3);
    border: none;
    border-right: 1px solid var(--border);
    cursor: pointer;
    text-align: center;
  }

  .tab:last-child { border-right: none; }
  .tab:hover { color: var(--text-1); }
  .tab-active { background: var(--text-1); color: var(--bg); }

  .input-row {
    display: flex;
    gap: var(--sp-2);
  }

  input {
    flex: 1;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--sp-1) var(--sp-2);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-1);
  }

  input::placeholder { color: var(--text-3); }
  input:focus { outline: none; border-color: var(--text-2); }
  input:disabled { opacity: 0.5; }

  /* ── Buttons ── */
  .btn {
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
    text-decoration: none;
    text-align: center;
    white-space: nowrap;
  }

  .btn:disabled { opacity: 0.25; cursor: not-allowed; }

  .btn-ghost {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.06em;
    padding: var(--sp-2) var(--sp-4);
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-2);
    cursor: pointer;
    white-space: nowrap;
  }

  .btn-ghost:hover { border-color: var(--text-2); color: var(--text-1); }

  .actions {
    display: flex;
    gap: var(--sp-2);
    align-items: center;
  }

  /* ── Footer ── */
  .footer {
    display: flex;
    gap: var(--sp-4);
    padding-top: var(--sp-4);
    border-top: 1px solid var(--border);
  }

  .footer-link {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    text-decoration: none;
    letter-spacing: 0.04em;
  }

  .footer-link:hover { color: var(--text-1); }

  @media (max-width: 640px) {
    .page { padding: var(--sp-4) var(--sp-3); }
    .step-info { flex-direction: column; gap: 0; }
    .tab { padding: var(--sp-1); font-size: 10px; }
    .actions { flex-direction: column; align-items: stretch; }
  }
</style>
