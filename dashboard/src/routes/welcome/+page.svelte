<script lang="ts">
  import { onMount } from 'svelte';
  import { base } from '$app/paths';
  import {
    fetchSetupStatus,
    generateSoulPreview,
    getCurrentSoul,
    saveGeneratedSoul,
    saveSecret,
    getSecret,
    type CurrentSoul,
    type GeneratedSoul,
    type SetupStatus,
    type UserInterview,
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

  // Soul personalization
  let currentSoul = $state<CurrentSoul | null>(null);
  let showSoulForm = $state(false);
  let soulPreview = $state<GeneratedSoul | null>(null);
  let soulFeedback = $state('');
  let soulError = $state('');
  let generatingSoul = $state(false);
  let savingSoul = $state(false);
  let interviewName = $state('');
  let interviewAbout = $state('');
  let interviewIdeal = $state('');
  let interviewWorkingStyle = $state('');
  let interviewPushback = $state('');
  const providerKeyMap: Record<string, string> = {
    anthropic: 'anthropic_api_key',
    openai: 'openai_api_key',
    openrouter: 'openrouter_api_key',
  };

  let setupComplete = $derived(
    status?.has_anthropic_key
      && status?.has_agents
      && status?.discord_connected
      && status?.has_personalized_soul
  );

  let completedSteps = $derived.by(() => {
    if (!status) return 0;
    let n = 0;
    if (status.has_anthropic_key) n++;
    if (status.discord_connected) n++;
    if (status.has_agents) n++;
    if (status.has_personalized_soul) n++;
    return n;
  });

  onMount(async () => {
    try {
      status = await fetchSetupStatus();

      if (status.has_anthropic_key) {
        // Detect active provider
        const savedProvider = await getSecret('llm_provider');
        if (savedProvider) llmProvider = savedProvider;
        currentSoul = await getCurrentSoul();
        showSoulForm = !status.has_personalized_soul;
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
    if (llmProvider === 'openai_codex') {
      await saveSecret('llm_provider', llmProvider);
      keyError = 'Use Settings to sign in with OpenAI Codex.';
      return;
    }
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
      showSoulForm = true;
      llmKey = '';
      currentSoul = await getCurrentSoul();
    } catch (err) {
      keyError = err instanceof Error ? err.message : 'Failed to save API key';
    } finally {
      savingKey = false;
    }
  }

  async function refresh() {
    status = await fetchSetupStatus();
    currentSoul = await getCurrentSoul();
  }

  function buildInterview(): UserInterview {
    return {
      name: interviewName.trim(),
      about_you: interviewAbout.trim(),
      ideal_paw: interviewIdeal.trim(),
      followup_answers: [
        ['How should Paw adapt to how you work?', interviewWorkingStyle.trim()],
        ['When should Paw challenge or push back on your thinking?', interviewPushback.trim()],
      ],
    };
  }

  let canGenerateSoul = $derived(
    !!interviewName.trim() && !!interviewAbout.trim() && !!interviewIdeal.trim()
  );

  async function generateSoul(feedback?: string) {
    if (!canGenerateSoul) return;
    generatingSoul = true;
    soulError = '';
    try {
      soulPreview = await generateSoulPreview({
        interview: buildInterview(),
        previous_summary: soulPreview?.summary,
        feedback: feedback?.trim() || undefined,
      });
    } catch (err) {
      soulError = err instanceof Error ? err.message : 'Failed to generate soul';
    } finally {
      generatingSoul = false;
    }
  }

  async function saveSoul() {
    if (!soulPreview) return;
    savingSoul = true;
    soulError = '';
    try {
      await saveGeneratedSoul(soulPreview);
      status = await fetchSetupStatus();
      currentSoul = await getCurrentSoul();
      showSoulForm = false;
      soulFeedback = '';
    } catch (err) {
      soulError = err instanceof Error ? err.message : 'Failed to save soul';
    } finally {
      savingSoul = false;
    }
  }
</script>

<svelte:head>
  <title>Setup &middot; Temper Paw</title>
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
      <h1 class="title">{setupComplete ? 'Temper Paw' : 'Setup'}</h1>
      <p class="subtitle">{setupComplete
        ? 'Your instance is ready.'
        : `${completedSteps} of 4 steps complete. Configure below or ask Paw for help.`}</p>
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
            {#if llmProvider === 'openai_codex'}
              <div class="input-row">
                <a class="btn" href="{base}/settings">Open Codex sign in</a>
              </div>
            {:else}
              <div class="input-row">
                <input type="password" bind:value={llmKey} placeholder="API key" disabled={savingKey} />
                <button class="btn" type="submit" disabled={!llmKey.trim() || savingKey}>
                  {savingKey ? '...' : 'Save'}
                </button>
              </div>
            {/if}
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
          <p class="step-hint">Add your Discord bot token and guild ID in Settings. Saving Discord values now reconnects automatically.</p>
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

      <!-- 4. Paw soul -->
      <div class="step" class:step-done={status?.has_personalized_soul}>
        <div class="step-row">
          <span class="step-dot" class:step-dot--on={status?.has_personalized_soul}></span>
          <div class="step-info">
            <span class="step-name">Paw Personality</span>
            <span class="step-status">
              {#if status?.has_personalized_soul}
                Personalized
              {:else}
                Interview needed
              {/if}
            </span>
          </div>
          {#if status?.has_anthropic_key}
            <button class="step-act" onclick={() => showSoulForm = !showSoulForm}>
              {showSoulForm ? 'Close' : status?.has_personalized_soul ? 'Review' : 'Start'}
            </button>
          {/if}
        </div>
        {#if status?.has_personalized_soul && currentSoul && !showSoulForm}
          <p class="step-hint">{currentSoul.summary}</p>
        {/if}
        {#if !status?.has_anthropic_key}
          <p class="step-hint">Configure an LLM first so Paw can interview you and generate a personalized operating style.</p>
        {/if}
        {#if showSoulForm && status?.has_anthropic_key}
          <div class="step-form soul-form">
            <div class="soul-grid">
              <label>
                <span>Name</span>
                <input type="text" bind:value={interviewName} placeholder="What should Paw call you?" />
              </label>
              <label>
                <span>About You</span>
                <textarea bind:value={interviewAbout} rows="3" placeholder="Your role, what you're building, and what matters most."></textarea>
              </label>
              <label>
                <span>Ideal Paw</span>
                <textarea bind:value={interviewIdeal} rows="3" placeholder="Describe the kind of teammate you want Paw to be."></textarea>
              </label>
              <label>
                <span>Working Style</span>
                <textarea bind:value={interviewWorkingStyle} rows="2" placeholder="How should Paw adapt to your pace, context, and communication style?"></textarea>
              </label>
              <label>
                <span>Pushback</span>
                <textarea bind:value={interviewPushback} rows="2" placeholder="When should Paw challenge your assumptions or slow you down?"></textarea>
              </label>
            </div>

            <div class="soul-actions">
              <button class="btn" type="button" onclick={() => generateSoul()} disabled={!canGenerateSoul || generatingSoul}>
                {generatingSoul ? 'Generating…' : soulPreview ? 'Regenerate' : 'Generate'}
              </button>
              {#if soulPreview}
                <button class="btn-ghost" type="button" onclick={saveSoul} disabled={savingSoul}>
                  {savingSoul ? 'Saving…' : 'Save Soul'}
                </button>
              {/if}
            </div>

            {#if soulPreview}
              <div class="soul-preview">
                <p class="step-hint">{soulPreview.summary}</p>
                <textarea
                  bind:value={soulFeedback}
                  rows="2"
                  placeholder="Optional feedback for a revision, for example: make Paw more concise and more willing to push back."
                ></textarea>
                <button class="step-act" type="button" onclick={() => generateSoul(soulFeedback)} disabled={!soulFeedback.trim() || generatingSoul}>
                  Refine
                </button>
                <pre>{soulPreview.soul_md}</pre>
              </div>
            {:else if currentSoul}
              <div class="soul-preview">
                <p class="step-hint">{currentSoul.summary}</p>
                <pre>{currentSoul.content}</pre>
              </div>
            {/if}

            {#if soulError}
              <p class="err">{soulError}</p>
            {/if}
          </div>
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

  textarea {
    width: 100%;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--sp-2);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-1);
    resize: vertical;
    min-height: 4rem;
    box-sizing: border-box;
  }

  textarea::placeholder { color: var(--text-3); }
  textarea:focus { outline: none; border-color: var(--text-2); }

  .soul-form {
    gap: var(--sp-3);
  }

  .soul-grid {
    display: grid;
    gap: var(--sp-2);
  }

  .soul-grid label {
    display: grid;
    gap: var(--sp-1);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-2);
  }

  .soul-actions {
    display: flex;
    gap: var(--sp-2);
    align-items: center;
  }

  .soul-preview {
    display: grid;
    gap: var(--sp-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--sp-3);
  }

  .soul-preview pre {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-2);
    white-space: pre-wrap;
    max-height: 16rem;
    overflow: auto;
  }

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
