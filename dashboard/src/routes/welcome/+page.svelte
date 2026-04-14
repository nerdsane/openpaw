<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { base } from '$app/paths';
  import Chat from '$lib/components/Chat.svelte';
  import { fetchSetupStatus, queryEntities, saveSecret, type SetupStatus } from '$lib/api';

  let status = $state<SetupStatus | null>(null);
  let pawAgentId = $state<string | null>(null);
  let loading = $state(true);
  let error = $state('');

  // LLM bootstrap form (shown when no LLM key is configured)
  let llmProvider = $state('anthropic');
  let llmKey = $state('');
  let savingKey = $state(false);
  let keyError = $state('');

  const providerKeyMap: Record<string, string> = {
    anthropic: 'anthropic_api_key',
    openai: 'openai_api_key',
    openai_codex: 'openai_codex_token',
    openrouter: 'openrouter_api_key',
  };

  const providerLabels: Record<string, string> = {
    anthropic: 'Anthropic API Key',
    openai: 'OpenAI API Key',
    openai_codex: 'OpenAI Codex Token',
    openrouter: 'OpenRouter API Key',
  };

  onMount(async () => {
    try {
      status = await fetchSetupStatus();

      // If setup is already complete, go to dashboard
      if (status.has_anthropic_key && status.has_agents) {
        await goto(`${base}/`);
        return;
      }

      // If LLM is configured, find the Paw agent for chat
      if (status.has_anthropic_key) {
        await findPawAgent();
      }
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load setup status';
    } finally {
      loading = false;
    }
  });

  async function findPawAgent() {
    try {
      const agents = await queryEntities('Agents', "name eq 'Paw' and Status eq 'Active'");
      if (agents.length > 0) {
        pawAgentId = (agents[0] as any)._entity_id || (agents[0] as any).Id;
      }
    } catch {
      // Agent may not exist yet on very fresh deploy
    }
  }

  async function saveLlmKey() {
    const key = llmKey.trim();
    if (!key) return;

    savingKey = true;
    keyError = '';

    try {
      const secretKey = providerKeyMap[llmProvider];
      await saveSecret(secretKey, key);
      await saveSecret('llm_provider', llmProvider);

      // Refresh status and find Paw agent
      status = await fetchSetupStatus();
      await findPawAgent();
    } catch (err) {
      keyError = err instanceof Error ? err.message : 'Failed to save API key';
    } finally {
      savingKey = false;
    }
  }

  function handleChatComplete(_result: string) {
    // Refresh status after chat completes setup
    fetchSetupStatus().then(s => {
      status = s;
    });
  }
</script>

<div class="welcome">
  <div class="welcome-header">
    <h1 class="welcome-title">Welcome to OpenPaw</h1>
    {#if status && status.has_anthropic_key && status.has_agents}
      <p class="welcome-subtitle">Everything looks good. You're all set.</p>
      <a href="{base}/" class="welcome-action">Go to Dashboard</a>
    {:else if loading}
      <p class="welcome-subtitle">Checking your setup...</p>
    {:else if error}
      <p class="welcome-subtitle welcome-error">{error}</p>
    {:else}
      <p class="welcome-subtitle">Let's get your instance configured.</p>
    {/if}
  </div>

  {#if !loading && !error}
    {#if status && !status.has_anthropic_key}
      <!-- Step 1: Need an LLM key before we can chat -->
      <div class="llm-setup">
        <h2 class="section-title">Connect an LLM Provider</h2>
        <p class="section-desc">Paw needs an LLM to think. Add an API key to get started.</p>

        <div class="llm-form">
          <div class="field">
            <label for="llm-provider">Provider</label>
            <select id="llm-provider" bind:value={llmProvider}>
              <option value="anthropic">Anthropic (Claude)</option>
              <option value="openai">OpenAI</option>
              <option value="openai_codex">OpenAI Codex</option>
              <option value="openrouter">OpenRouter</option>
            </select>
          </div>

          <div class="field">
            <label for="llm-key">{providerLabels[llmProvider]}</label>
            <input
              id="llm-key"
              type="password"
              bind:value={llmKey}
              placeholder="sk-..."
              disabled={savingKey}
            />
          </div>

          {#if keyError}
            <p class="field-error">{keyError}</p>
          {/if}

          <button
            class="btn-primary"
            onclick={saveLlmKey}
            disabled={!llmKey.trim() || savingKey}
          >
            {savingKey ? 'Saving...' : 'Save & Continue'}
          </button>
        </div>
      </div>
    {:else if pawAgentId}
      <!-- Step 2: LLM configured, chat with Paw for remaining setup -->
      <div class="chat-container">
        <Chat
          agentId={pawAgentId}
          systemPrompt="You are Paw, the user's AI assistant. You have the setup-guide skill loaded. Check what's already configured and help the user complete their OpenPaw setup. Be conversational and proactive — check first, ask second. Skip what's already done."
          greeting="Hey! I'm Paw, your AI assistant. Let me check what's configured and help you finish setting up."
          onComplete={handleChatComplete}
        />
      </div>
    {:else if status?.has_anthropic_key}
      <!-- LLM configured but no Paw agent found (rare edge case) -->
      <div class="fallback-setup">
        <p class="section-desc">LLM is configured but no Paw agent was found. The platform may still be starting up.</p>
        <button class="btn-primary" onclick={() => location.reload()}>Refresh</button>
        <a href="{base}/settings" class="welcome-link">Go to Settings</a>
      </div>
    {/if}
  {/if}

  {#if !loading && status && status.has_anthropic_key}
    <div class="welcome-footer">
      <a href="{base}/" class="welcome-link">Skip to Dashboard</a>
      <a href="{base}/settings" class="welcome-link">Manual Settings</a>
    </div>
  {/if}
</div>

<style>
  .welcome {
    max-width: 560px;
    margin: 0 auto;
    padding: var(--sp-8) var(--sp-4);
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
    min-height: 100vh;
  }

  .welcome-header {
    text-align: center;
  }

  .welcome-title {
    font-size: var(--text-xl);
    font-weight: 500;
    color: var(--text-1);
    margin-bottom: var(--sp-1);
  }

  .welcome-subtitle {
    font-size: var(--text-sm);
    color: var(--text-3);
  }

  .welcome-error {
    color: var(--status-error);
  }

  .welcome-action {
    display: inline-block;
    margin-top: var(--sp-4);
    padding: var(--sp-2) var(--sp-6);
    border-radius: var(--radius);
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
    font-size: var(--text-base);
    text-decoration: none;
  }

  /* ---- LLM Setup Form ---- */
  .llm-setup, .fallback-setup {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--sp-4);
  }

  .section-title {
    font-size: var(--text-lg);
    font-weight: 600;
    color: var(--text-1);
    margin-bottom: var(--sp-1);
  }

  .section-desc {
    font-size: var(--text-sm);
    color: var(--text-3);
    margin-bottom: var(--sp-4);
  }

  .llm-form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }

  .field label {
    font-size: var(--text-xs);
    font-weight: 500;
    color: var(--text-2);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .field select, .field input {
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-1);
    font-size: var(--text-base);
    font-family: inherit;
  }

  .field select option {
    background: var(--bg);
    color: var(--text-1);
  }

  .field-error {
    font-size: var(--text-sm);
    color: var(--status-error);
  }

  .btn-primary {
    padding: var(--sp-2) var(--sp-4);
    border-radius: var(--radius);
    border: none;
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
    font-size: var(--text-sm);
    cursor: pointer;
    align-self: flex-start;
  }

  .btn-primary:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* ---- Chat Container ---- */
  .chat-container {
    flex: 1;
    min-height: 400px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
  }

  /* ---- Footer ---- */
  .welcome-footer {
    display: flex;
    justify-content: center;
    gap: var(--sp-6);
    padding-top: var(--sp-2);
  }

  .welcome-link {
    font-size: var(--text-sm);
    color: var(--text-3);
    text-decoration: none;
  }

  .welcome-link:hover {
    color: var(--accent);
  }

  .fallback-setup {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-3);
  }
</style>
