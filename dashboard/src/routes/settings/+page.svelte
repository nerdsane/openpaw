<script lang="ts">
  import {
    connectDiscord,
    connectSlack,
    disconnectDiscord,
    disconnectSlack,
    fetchSecretsSchema,
    fetchSetupStatus,
    generateSoulPreview,
    getCurrentSoul,
    getRailwayStatus,
    getSecret,
    listSecretKeys,
    saveGeneratedSoul,
    saveSecret,
    setRailwayVar,
    type GeneratedSoul,
    type RailwayStatus,
    type SecretSchemaEntry,
    type SetupStatus,
    type UserInterview
  } from '$lib/api';
  import { changePassword } from '$lib/auth';
  import { onMount } from 'svelte';

  let status = $state<SetupStatus | null>(null);
  let loading = $state(true);
  let sectionFeedback = $state<Record<string, { type: 'error' | 'success'; message: string }>>({});

  // LLM provider — now includes openai_codex
  let llmProvider = $state<'anthropic' | 'openrouter' | 'openai' | 'openai_codex'>('anthropic');
  let anthropicApiKey = $state('');
  let openrouterApiKey = $state('');
  let openaiApiKey = $state('');
  let openaiCodexToken = $state('');

  // Discord
  let discordBotToken = $state('');
  let discordPublicKey = $state('');
  let discordGuildId = $state('');
  let discordFeedChannelId = $state('');
  let discordForumChannelId = $state('');

  // Slack
  let slackAppToken = $state('');
  let slackBotToken = $state('');
  let slackSigningSecret = $state('');

  // Integrations
  let githubToken = $state('');
  let exaApiKey = $state('');

  // Observability
  let ddApiKey = $state('');
  let ddSite = $state('datadoghq.com');

  // Account
  let apiKey = $state('');
  let showApiKey = $state(false);
  let currentPassword = $state('');
  let newPassword = $state('');

  // Soul
  let soul = $state<{ summary: string; content: string } | null>(null);
  let generatedSoul = $state<GeneratedSoul | null>(null);
  let soulFeedback = $state('');
  let interview = $state<UserInterview>({
    name: '',
    about_you: '',
    ideal_paw: '',
    followup_answers: []
  });

  // Secrets schema + existing keys
  let schema = $state<SecretSchemaEntry[]>([]);
  let existingKeys = $state<string[]>([]);

  // Railway integration status
  let railwayStatus = $state<RailwayStatus | null>(null);

  function setFeedback(section: string, type: 'error' | 'success', message: string) {
    sectionFeedback = { ...sectionFeedback, [section]: { type, message } };
    if (type === 'success') {
      setTimeout(() => {
        sectionFeedback = { ...sectionFeedback, [section]: undefined as any };
      }, 4000);
    }
  }

  function clearFeedback(section: string) {
    const { [section]: _, ...rest } = sectionFeedback;
    sectionFeedback = rest;
  }

  // Derive LLM status label from provider
  let llmStatusLabel = $derived.by(() => {
    if (!status?.has_anthropic_key) return 'Missing';
    const p = status?.llm_provider;
    if (p === 'anthropic') return 'Anthropic';
    if (p === 'openrouter') return 'OpenRouter';
    if (p === 'openai' || p === 'openai_codex') return 'OpenAI';
    return 'Configured';
  });

  onMount(async () => {
    await load();
  });

  async function load() {
    loading = true;
    sectionFeedback = {};

    try {
      const [
        nextStatus,
        currentSoul,
        secretsSchema,
        secretKeys,
        railwayInfo,
        savedAnthropic,
        savedOpenRouter,
        savedOpenAi,
        savedCodex,
        savedDiscordBot,
        savedDiscordPublic,
        savedDiscordGuild,
        savedDiscordFeed,
        savedDiscordForum,
        savedSlackApp,
        savedSlackBot,
        savedSlackSigningSecret,
        savedGithub,
        savedExa,
        savedDdApiKey,
        savedDdSite,
        savedProvider,
        savedApiKey
      ] = await Promise.all([
        fetchSetupStatus(),
        getCurrentSoul(),
        fetchSecretsSchema(),
        listSecretKeys(),
        getRailwayStatus(),
        getSecret('anthropic_api_key'),
        getSecret('openrouter_api_key'),
        getSecret('openai_api_key'),
        getSecret('openai_codex_token'),
        getSecret('discord_bot_token'),
        getSecret('discord_public_key'),
        getSecret('discord_guild_id'),
        getSecret('discord_feed_channel_id'),
        getSecret('discord_forum_channel_id'),
        getSecret('slack_app_token'),
        getSecret('slack_bot_token'),
        getSecret('slack_signing_secret'),
        getSecret('github_token'),
        getSecret('exa_api_key'),
        getSecret('dd_api_key'),
        getSecret('dd_site'),
        getSecret('llm_provider'),
        getSecret('temper_api_key')
      ]);

      status = nextStatus;
      soul = currentSoul;
      schema = secretsSchema;
      existingKeys = secretKeys;
      railwayStatus = railwayInfo;
      anthropicApiKey = savedAnthropic ?? '';
      openrouterApiKey = savedOpenRouter ?? '';
      openaiApiKey = savedOpenAi ?? '';
      openaiCodexToken = savedCodex ?? '';
      discordBotToken = savedDiscordBot ?? '';
      discordPublicKey = savedDiscordPublic ?? '';
      discordGuildId = savedDiscordGuild ?? '';
      discordFeedChannelId = savedDiscordFeed ?? '';
      discordForumChannelId = savedDiscordForum ?? '';
      slackAppToken = savedSlackApp ?? '';
      slackBotToken = savedSlackBot ?? '';
      slackSigningSecret = savedSlackSigningSecret ?? '';
      githubToken = savedGithub ?? '';
      exaApiKey = savedExa ?? '';
      ddApiKey = savedDdApiKey ?? '';
      ddSite = savedDdSite || 'datadoghq.com';
      apiKey = savedApiKey ?? '';
      llmProvider = (savedProvider as typeof llmProvider) || (savedCodex ? 'openai_codex' : savedOpenRouter ? 'openrouter' : savedOpenAi ? 'openai' : 'anthropic');
    } catch (err) {
      setFeedback('global', 'error', err instanceof Error ? err.message : 'Could not load settings');
    } finally {
      loading = false;
    }
  }

  async function saveProviderSecret(provider: typeof llmProvider) {
    clearFeedback('llm');
    try {
      const value =
        provider === 'anthropic' ? anthropicApiKey :
        provider === 'openrouter' ? openrouterApiKey :
        provider === 'openai_codex' ? openaiCodexToken :
        openaiApiKey;

      await saveSecret('llm_provider', provider);
      if (provider === 'anthropic') await saveSecret('anthropic_api_key', value);
      if (provider === 'openrouter') await saveSecret('openrouter_api_key', value);
      if (provider === 'openai') await saveSecret('openai_api_key', value);
      if (provider === 'openai_codex') await saveSecret('openai_codex_token', value);
      setFeedback('llm', 'success', `${provider === 'openai_codex' ? 'OpenAI Codex' : provider} saved`);
      // Refresh status to update LLM display
      status = await fetchSetupStatus();
    } catch (err) {
      setFeedback('llm', 'error', err instanceof Error ? err.message : 'Failed to save provider');
    }
  }

  async function saveDiscord() {
    clearFeedback('discord');
    try {
      await Promise.all([
        saveSecret('discord_bot_token', discordBotToken),
        saveSecret('discord_public_key', discordPublicKey),
        saveSecret('discord_guild_id', discordGuildId),
        saveSecret('discord_feed_channel_id', discordFeedChannelId),
        saveSecret('discord_forum_channel_id', discordForumChannelId)
      ]);
      await connectDiscord({
        bot_token: discordBotToken,
        public_key: discordPublicKey,
        guild_id: discordGuildId || undefined,
        feed_channel_id: discordFeedChannelId || undefined,
        forum_channel_id: discordForumChannelId || undefined
      });
      setFeedback('discord', 'success', 'Discord connected');
      status = await fetchSetupStatus();
    } catch (err) {
      setFeedback('discord', 'error', err instanceof Error ? err.message : 'Failed to connect Discord');
    }
  }

  async function handleDisconnectDiscord() {
    clearFeedback('discord');
    try {
      await disconnectDiscord();
      setFeedback('discord', 'success', 'Discord disconnected');
      status = await fetchSetupStatus();
    } catch (err) {
      setFeedback('discord', 'error', err instanceof Error ? err.message : 'Failed to disconnect');
    }
  }

  async function saveSlack() {
    clearFeedback('slack');
    try {
      await Promise.all([
        saveSecret('slack_app_token', slackAppToken),
        saveSecret('slack_bot_token', slackBotToken),
        saveSecret('slack_signing_secret', slackSigningSecret)
      ]);
      await connectSlack({
        app_token: slackAppToken,
        bot_token: slackBotToken,
        signing_secret: slackSigningSecret || undefined
      });
      setFeedback('slack', 'success', 'Slack connected');
      status = await fetchSetupStatus();
    } catch (err) {
      setFeedback('slack', 'error', err instanceof Error ? err.message : 'Failed to connect Slack');
    }
  }

  async function handleDisconnectSlack() {
    clearFeedback('slack');
    try {
      await disconnectSlack();
      setFeedback('slack', 'success', 'Slack disconnected');
      status = await fetchSetupStatus();
    } catch (err) {
      setFeedback('slack', 'error', err instanceof Error ? err.message : 'Failed to disconnect');
    }
  }

  async function saveIntegrations() {
    clearFeedback('integrations');
    try {
      await Promise.all([
        githubToken ? saveSecret('github_token', githubToken) : Promise.resolve(),
        exaApiKey ? saveSecret('exa_api_key', exaApiKey) : Promise.resolve()
      ]);
      setFeedback('integrations', 'success', 'Integrations saved');
    } catch (err) {
      setFeedback('integrations', 'error', err instanceof Error ? err.message : 'Failed to save');
    }
  }

  async function saveObservability() {
    clearFeedback('observability');
    try {
      // Save to vault
      await Promise.all([
        ddApiKey ? saveSecret('dd_api_key', ddApiKey) : Promise.resolve(),
        saveSecret('dd_site', ddSite)
      ]);

      // Push to Railway otel-collector if integration is configured
      if (railwayStatus?.configured && ddApiKey) {
        try {
          await Promise.all([
            setRailwayVar('otel-collector', 'DD_API_KEY', ddApiKey),
            setRailwayVar('otel-collector', 'DD_SITE', ddSite),
          ]);
          setFeedback('observability', 'success', 'Saved to vault and pushed to Railway OTEL collector');
        } catch (railwayErr) {
          setFeedback('observability', 'success',
            `Saved to vault. Railway push failed: ${railwayErr instanceof Error ? railwayErr.message : 'unknown error'}`);
        }
      } else {
        setFeedback('observability', 'success',
          railwayStatus?.configured ? 'Saved to vault' : 'Saved to vault (Railway integration not configured)');
      }
    } catch (err) {
      setFeedback('observability', 'error', err instanceof Error ? err.message : 'Failed to save');
    }
  }

  async function generateSoul() {
    clearFeedback('soul');
    try {
      generatedSoul = await generateSoulPreview({
        interview,
        previous_summary: generatedSoul?.summary,
        feedback: soulFeedback || undefined
      });
      soulFeedback = '';
      setFeedback('soul', 'success', 'Preview ready');
    } catch (err) {
      setFeedback('soul', 'error', err instanceof Error ? err.message : 'Failed to generate preview');
    }
  }

  async function saveSoul() {
    if (!generatedSoul) return;
    clearFeedback('soul');
    try {
      await saveGeneratedSoul(generatedSoul);
      setFeedback('soul', 'success', 'Soul updated');
      soul = await getCurrentSoul();
    } catch (err) {
      setFeedback('soul', 'error', err instanceof Error ? err.message : 'Failed to save soul');
    }
  }

  async function updatePassword() {
    clearFeedback('account');
    try {
      await changePassword(currentPassword, newPassword);
      currentPassword = '';
      newPassword = '';
      setFeedback('account', 'success', 'Password updated');
    } catch (err) {
      setFeedback('account', 'error', err instanceof Error ? err.message : 'Failed to change password');
    }
  }
</script>

<svelte:head>
  <title>Settings · Open Paw</title>
</svelte:head>

<section class="settings-shell">
  <header class="hero">
    <p class="eyebrow">SETTINGS</p>
    <h1>Configuration</h1>
    <p class="lede">Infrastructure comes from deploy-time env vars. Everything here is the live, personal layer your dashboard and agents read from the same vault.</p>
  </header>

  {#if loading}
    <p>Loading settings...</p>
  {:else}
    {#if sectionFeedback.global}
      <p class="message {sectionFeedback.global.type}">{sectionFeedback.global.message}</p>
    {/if}

    <!-- ─── Status Bar ─── -->
    <section class="card full">
      <h2>Integration status</h2>
      <div class="status-grid">
        <article>
          <span class:ok={status?.has_anthropic_key}>LLM</span>
          <strong>{llmStatusLabel}</strong>
        </article>
        <article>
          <span class:ok={status?.discord_connected}>Discord</span>
          <strong>{status?.discord_connected ? 'Connected' : 'Disconnected'}</strong>
        </article>
        <article>
          <span class:ok={status?.slack_connected}>Slack</span>
          <strong>{status?.slack_connected ? 'Connected' : 'Disconnected'}</strong>
        </article>
        <article>
          <span class:ok={status?.agent_count}>Agents</span>
          <strong>{status?.agent_count ?? 0}</strong>
        </article>
      </div>
    </section>

    <!-- ─── LLM Provider ─── -->
    <section class="card full">
      <h2>LLM provider</h2>
      {#if sectionFeedback.llm}
        <p class="message {sectionFeedback.llm.type}">{sectionFeedback.llm.message}</p>
      {/if}
      <div class="provider-tabs">
        <button class:active={llmProvider === 'anthropic'} onclick={() => llmProvider = 'anthropic'} type="button">Anthropic</button>
        <button class:active={llmProvider === 'openrouter'} onclick={() => llmProvider = 'openrouter'} type="button">OpenRouter</button>
        <button class:active={llmProvider === 'openai'} onclick={() => llmProvider = 'openai'} type="button">OpenAI</button>
        <button class:active={llmProvider === 'openai_codex'} onclick={() => llmProvider = 'openai_codex'} type="button">OpenAI Codex</button>
      </div>

      {#if llmProvider === 'anthropic'}
        <label><span>Anthropic API key</span><input bind:value={anthropicApiKey} placeholder="sk-ant-..." type="password" /></label>
      {:else if llmProvider === 'openrouter'}
        <label><span>OpenRouter API key</span><input bind:value={openrouterApiKey} placeholder="sk-or-..." type="password" /></label>
      {:else if llmProvider === 'openai_codex'}
        <label><span>Codex OAuth token</span><input bind:value={openaiCodexToken} placeholder="Paste from ~/.codex/auth.json or run codex login" type="password" /></label>
        <p class="hint">Included with ChatGPT Plus/Pro subscription. Run <code>codex login</code> to get a token.</p>
      {:else}
        <label><span>OpenAI API key</span><input bind:value={openaiApiKey} placeholder="sk-..." type="password" /></label>
      {/if}

      <button class="action" onclick={() => saveProviderSecret(llmProvider)} type="button">Save provider</button>
    </section>

    <div class="grid">
      <!-- ─── Discord ─── -->
      <section class="card">
        <h2>Discord</h2>
        {#if sectionFeedback.discord}
          <p class="message {sectionFeedback.discord.type}">{sectionFeedback.discord.message}</p>
        {/if}
        <label><span>Bot token</span><input bind:value={discordBotToken} type="password" /></label>
        <label><span>Public key</span><input bind:value={discordPublicKey} /></label>
        <label><span>Guild ID</span><input bind:value={discordGuildId} /></label>
        <label><span>Feed channel</span><input bind:value={discordFeedChannelId} /></label>
        <label><span>Forum channel</span><input bind:value={discordForumChannelId} /></label>
        <div class="row">
          <button class="action" onclick={saveDiscord} type="button">Connect</button>
          <button class="ghost" onclick={handleDisconnectDiscord} type="button">Disconnect</button>
        </div>
        {#if status?.discord_interaction_url}
          <p class="hint">Interaction URL: {status.discord_interaction_url}</p>
        {/if}
      </section>

      <!-- ─── Slack ─── -->
      <section class="card">
        <h2>Slack</h2>
        {#if sectionFeedback.slack}
          <p class="message {sectionFeedback.slack.type}">{sectionFeedback.slack.message}</p>
        {/if}
        <label><span>App token</span><input bind:value={slackAppToken} type="password" /></label>
        <label><span>Bot token</span><input bind:value={slackBotToken} type="password" /></label>
        <label><span>Signing secret</span><input bind:value={slackSigningSecret} type="password" /></label>
        <div class="row">
          <button class="action" onclick={saveSlack} type="button">Connect</button>
          <button class="ghost" onclick={handleDisconnectSlack} type="button">Disconnect</button>
        </div>
      </section>

      <!-- ─── Integrations ─── -->
      <section class="card">
        <h2>Integrations</h2>
        {#if sectionFeedback.integrations}
          <p class="message {sectionFeedback.integrations.type}">{sectionFeedback.integrations.message}</p>
        {/if}
        <label><span>GitHub token</span><input bind:value={githubToken} type="password" /></label>
        <label><span>Exa API key</span><input bind:value={exaApiKey} type="password" /></label>
        <button class="action" onclick={saveIntegrations} type="button">Save integrations</button>
      </section>

      <!-- ─── Observability ─── -->
      <section class="card">
        <h2>Observability</h2>
        {#if sectionFeedback.observability}
          <p class="message {sectionFeedback.observability.type}">{sectionFeedback.observability.message}</p>
        {/if}
        <label><span>Datadog API key</span><input bind:value={ddApiKey} type="password" placeholder="Enter to enable Datadog traces/metrics" /></label>
        <label><span>Datadog site</span><input bind:value={ddSite} placeholder="datadoghq.com" /></label>
        {#if railwayStatus?.configured}
          <p class="hint">Railway integration active. Saving will push DD_API_KEY to the OTEL collector service and restart it.</p>
        {:else}
          <p class="hint">Saves to vault. Deploy with <code>openpaw deploy</code> to enable automatic Railway push.</p>
        {/if}
        <button class="action" onclick={saveObservability} type="button">Save observability</button>
      </section>
    </div>

    <!-- ─── Soul Personalization ─── -->
    <section class="card full">
      <h2>Soul personalization</h2>
      {#if sectionFeedback.soul}
        <p class="message {sectionFeedback.soul.type}">{sectionFeedback.soul.message}</p>
      {/if}
      <p class="hint">Same generation flow as <code>openpaw run</code> — the result is saved into the live Paw soul.</p>
      {#if soul}
        <div class="preview-block">
          <strong>Current summary</strong>
          <p>{soul.summary}</p>
        </div>
      {/if}

      <div class="soul-grid">
        <label><span>Your name</span><input bind:value={interview.name} /></label>
        <label class="wide"><span>About you</span><textarea bind:value={interview.about_you} rows="4"></textarea></label>
        <label class="wide"><span>What kind of Paw do you want?</span><textarea bind:value={interview.ideal_paw} rows="4"></textarea></label>
        <label class="wide"><span>Adjustment feedback</span><textarea bind:value={soulFeedback} rows="3" placeholder="Optional — refine after a preview."></textarea></label>
      </div>

      <div class="row">
        <button class="action" onclick={generateSoul} type="button">Generate preview</button>
        <button class="action" disabled={!generatedSoul} onclick={saveSoul} type="button">Save soul</button>
      </div>

      {#if generatedSoul}
        <div class="preview-block">
          <strong>Preview</strong>
          <p>{generatedSoul.summary}</p>
        </div>
      {/if}
    </section>

    <!-- ─── Account ─── -->
    <section class="card full">
      <h2>Account</h2>
      {#if sectionFeedback.account}
        <p class="message {sectionFeedback.account.type}">{sectionFeedback.account.message}</p>
      {/if}
      <label>
        <span>Programmatic API key</span>
        <div class="row">
          <input value={apiKey} readonly type={showApiKey ? 'text' : 'password'} />
          <button class="ghost" type="button" onclick={() => showApiKey = !showApiKey}>{showApiKey ? 'Hide' : 'Show'}</button>
        </div>
      </label>
      <label><span>Current password</span><input bind:value={currentPassword} type="password" /></label>
      <label><span>New password</span><input bind:value={newPassword} type="password" /></label>
      <button class="action" onclick={updatePassword} type="button">Change password</button>
    </section>
  {/if}
</section>

<style>
  .settings-shell {
    display: grid;
    gap: var(--sp-4);
    max-width: 960px;
    margin: 0 auto;
    padding: var(--sp-6) var(--sp-4);
  }

  .hero h1 {
    margin: 0;
    font-size: var(--text-xl);
  }

  .eyebrow {
    margin: 0 0 var(--sp-1);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.08em;
    color: var(--text-3);
  }

  .lede {
    font-size: var(--text-sm);
    color: var(--text-2);
    line-height: 1.5;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-3);
  }

  .card {
    display: grid;
    gap: var(--sp-3);
    padding: var(--sp-4);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
  }

  .full {
    grid-column: 1 / -1;
  }

  h2 {
    margin: 0;
    font-size: var(--text-lg);
    font-weight: 600;
  }

  .status-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--sp-3);
  }

  .status-grid article {
    padding: var(--sp-3);
    border-radius: var(--radius);
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border);
  }

  .status-grid span {
    display: block;
    margin-bottom: var(--sp-1);
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-3);
  }

  .status-grid span.ok {
    color: var(--status-success);
  }

  .status-grid strong {
    font-size: var(--text-base);
  }

  .provider-tabs,
  .row {
    display: flex;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }

  .provider-tabs button {
    padding: var(--sp-1) var(--sp-3);
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-2);
    cursor: pointer;
    font-size: var(--text-sm);
  }

  .provider-tabs button.active {
    background: var(--accent);
    color: var(--bg);
    border-color: transparent;
    font-weight: 600;
  }

  .action {
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--radius);
    border: 1px solid transparent;
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
    cursor: pointer;
    font-size: var(--text-sm);
  }

  .action:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .ghost {
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
    font-size: var(--text-sm);
  }

  label {
    display: grid;
    gap: var(--sp-1);
  }

  label span {
    color: var(--text-2);
    font-size: var(--text-sm);
  }

  input,
  textarea {
    width: 100%;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-1);
    padding: var(--sp-2) var(--sp-3);
    font-size: var(--text-base);
  }

  code {
    background: rgba(255, 255, 255, 0.06);
    padding: 1px 4px;
    border-radius: var(--radius);
    font-size: var(--text-sm);
  }

  .hint {
    color: var(--text-3);
    font-size: var(--text-xs);
    line-height: 1.4;
    margin: 0;
  }

  .preview-block {
    padding: var(--sp-3);
    border-radius: var(--radius);
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border);
    font-size: var(--text-sm);
  }

  .preview-block p {
    margin-bottom: 0;
  }

  .soul-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-3);
  }

  .wide {
    grid-column: span 2;
  }

  .message {
    margin: 0;
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--radius);
    font-size: var(--text-sm);
  }

  .message.error {
    background: rgba(255, 104, 104, 0.12);
    color: var(--status-error);
  }

  .message.success {
    background: rgba(52, 211, 153, 0.12);
    color: var(--status-success);
  }

  @media (max-width: 900px) {
    .grid,
    .status-grid,
    .soul-grid {
      grid-template-columns: 1fr;
    }

    .wide {
      grid-column: span 1;
    }
  }
</style>
