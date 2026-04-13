<script lang="ts">
  import {
    connectDiscord,
    connectSlack,
    disconnectDiscord,
    disconnectSlack,
    fetchSetupStatus,
    generateSoulPreview,
    getCurrentSoul,
    getSecret,
    saveGeneratedSoul,
    saveSecret,
    type GeneratedSoul,
    type SetupStatus,
    type UserInterview
  } from '$lib/api';
  import { changePassword } from '$lib/auth';
  import { onMount } from 'svelte';

  let status = $state<SetupStatus | null>(null);
  let loading = $state(true);
  let error = $state('');
  let success = $state('');

  let llmProvider = $state<'anthropic' | 'openrouter' | 'openai'>('anthropic');
  let anthropicApiKey = $state('');
  let openrouterApiKey = $state('');
  let openaiApiKey = $state('');

  let discordBotToken = $state('');
  let discordPublicKey = $state('');
  let discordGuildId = $state('');
  let discordFeedChannelId = $state('');
  let discordForumChannelId = $state('');

  let slackAppToken = $state('');
  let slackBotToken = $state('');
  let slackSigningSecret = $state('');

  let githubToken = $state('');
  let exaApiKey = $state('');
  let apiKey = $state('');

  let soul = $state<{ summary: string; content: string } | null>(null);
  let generatedSoul = $state<GeneratedSoul | null>(null);
  let soulFeedback = $state('');
  let interview = $state<UserInterview>({
    name: '',
    about_you: '',
    ideal_paw: '',
    followup_answers: []
  });

  let currentPassword = $state('');
  let newPassword = $state('');

  onMount(async () => {
    await load();
  });

  async function load() {
    loading = true;
    error = '';
    success = '';

    try {
      const [
        nextStatus,
        currentSoul,
        savedAnthropic,
        savedOpenRouter,
        savedOpenAi,
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
        savedProvider,
        savedApiKey
      ] = await Promise.all([
        fetchSetupStatus(),
        getCurrentSoul(),
        getSecret('anthropic_api_key'),
        getSecret('openrouter_api_key'),
        getSecret('openai_api_key'),
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
        getSecret('llm_provider'),
        getSecret('temper_api_key')
      ]);

      status = nextStatus;
      soul = currentSoul;
      anthropicApiKey = savedAnthropic ?? '';
      openrouterApiKey = savedOpenRouter ?? '';
      openaiApiKey = savedOpenAi ?? '';
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
      apiKey = savedApiKey ?? '';
      llmProvider = (savedProvider as typeof llmProvider) || (savedOpenRouter ? 'openrouter' : savedOpenAi ? 'openai' : 'anthropic');
    } catch (err) {
      error = err instanceof Error ? err.message : 'Could not load settings';
    } finally {
      loading = false;
    }
  }

  async function saveProviderSecret(provider: 'anthropic' | 'openrouter' | 'openai') {
    const value =
      provider === 'anthropic' ? anthropicApiKey :
      provider === 'openrouter' ? openrouterApiKey :
      openaiApiKey;

    await saveSecret('llm_provider', provider);
    if (provider === 'anthropic') await saveSecret('anthropic_api_key', value);
    if (provider === 'openrouter') await saveSecret('openrouter_api_key', value);
    if (provider === 'openai') await saveSecret('openai_api_key', value);
    success = `${provider} configuration saved`;
    await load();
  }

  async function saveDiscord() {
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
    success = 'Discord connected';
    await load();
  }

  async function saveSlack() {
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
    success = 'Slack connected';
    await load();
  }

  async function generateSoul() {
    generatedSoul = await generateSoulPreview({
      interview,
      previous_summary: generatedSoul?.summary,
      feedback: soulFeedback || undefined
    });
    soulFeedback = '';
    success = 'New Paw preview ready';
  }

  async function saveSoul() {
    if (!generatedSoul) return;
    await saveGeneratedSoul(generatedSoul);
    success = 'Paw soul updated';
    soul = await getCurrentSoul();
  }

  async function updatePassword() {
    await changePassword(currentPassword, newPassword);
    currentPassword = '';
    newPassword = '';
    success = 'Password updated';
  }
</script>

<svelte:head>
  <title>Settings • Open Paw</title>
</svelte:head>

<section class="settings-shell">
  <header class="hero">
    <p class="eyebrow">SETTINGS</p>
    <h1>Personal configuration</h1>
    <p class="lede">Infrastructure comes from deploy-time env vars. Everything here is the live, personal layer your dashboard and terminal both read from the same vault.</p>
  </header>

  {#if loading}
    <p>Loading settings…</p>
  {:else}
    {#if error}<p class="message error">{error}</p>{/if}
    {#if success}<p class="message success">{success}</p>{/if}

    <div class="grid">
      <section class="card span-2">
        <h2>Integration status</h2>
        <div class="status-grid">
          <article>
            <span class:ok={status?.has_anthropic_key}>LLM</span>
            <strong>{status?.has_anthropic_key ? 'Configured' : 'Missing'}</strong>
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

      <section class="card span-2">
        <h2>LLM provider</h2>
        <div class="provider-tabs">
          <button class:active={llmProvider === 'anthropic'} onclick={() => llmProvider = 'anthropic'} type="button">Anthropic</button>
          <button class:active={llmProvider === 'openrouter'} onclick={() => llmProvider = 'openrouter'} type="button">OpenRouter</button>
          <button class:active={llmProvider === 'openai'} onclick={() => llmProvider = 'openai'} type="button">OpenAI</button>
        </div>

        {#if llmProvider === 'anthropic'}
          <label><span>Anthropic API key</span><input bind:value={anthropicApiKey} placeholder="sk-ant-..." type="password" /></label>
        {:else if llmProvider === 'openrouter'}
          <label><span>OpenRouter API key</span><input bind:value={openrouterApiKey} placeholder="sk-or-..." type="password" /></label>
        {:else}
          <label><span>OpenAI API key</span><input bind:value={openaiApiKey} placeholder="sk-..." type="password" /></label>
        {/if}

        <button class="action" onclick={() => saveProviderSecret(llmProvider)} type="button">Save provider</button>
      </section>

      <section class="card">
        <h2>Discord</h2>
        <label><span>Bot token</span><input bind:value={discordBotToken} type="password" /></label>
        <label><span>Public key</span><input bind:value={discordPublicKey} /></label>
        <label><span>Guild ID</span><input bind:value={discordGuildId} /></label>
        <label><span>Feed channel</span><input bind:value={discordFeedChannelId} /></label>
        <label><span>Forum channel</span><input bind:value={discordForumChannelId} /></label>
        <div class="row">
          <button class="action" onclick={saveDiscord} type="button">Connect Discord</button>
          <button class="ghost" onclick={disconnectDiscord} type="button">Disconnect</button>
        </div>
        {#if status?.discord_interaction_url}
          <p class="muted">{status.discord_interaction_url}</p>
        {/if}
      </section>

      <section class="card">
        <h2>Slack</h2>
        <label><span>App token</span><input bind:value={slackAppToken} type="password" /></label>
        <label><span>Bot token</span><input bind:value={slackBotToken} type="password" /></label>
        <label><span>Signing secret</span><input bind:value={slackSigningSecret} type="password" /></label>
        <div class="row">
          <button class="action" onclick={saveSlack} type="button">Connect Slack</button>
          <button class="ghost" onclick={disconnectSlack} type="button">Disconnect</button>
        </div>
      </section>

      <section class="card">
        <h2>Other integrations</h2>
        <label><span>GitHub token</span><input bind:value={githubToken} type="password" /></label>
        <label><span>Exa API key</span><input bind:value={exaApiKey} type="password" /></label>
        <div class="row">
          <button class="action" onclick={() => saveSecret('github_token', githubToken)} type="button">Save GitHub</button>
          <button class="action" onclick={() => saveSecret('exa_api_key', exaApiKey)} type="button">Save Exa</button>
        </div>
      </section>

      <section class="card">
        <h2>Account</h2>
        <label><span>Programmatic API key</span><input bind:value={apiKey} readonly /></label>
        <label><span>Current password</span><input bind:value={currentPassword} type="password" /></label>
        <label><span>New password</span><input bind:value={newPassword} type="password" /></label>
        <button class="action" onclick={updatePassword} type="button">Change password</button>
      </section>

      <section class="card span-2">
        <h2>Soul personalization</h2>
        <p class="muted">The browser now uses the same generation flow as `openpaw setup`, then saves the result into the live Paw soul file.</p>
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
          <label class="wide"><span>Adjustment feedback</span><textarea bind:value={soulFeedback} rows="3" placeholder="Optional. Use this after a preview if you want a more specific revision."></textarea></label>
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
    </div>
  {/if}
</section>

<style>
  .settings-shell {
    display: grid;
    gap: 1.5rem;
  }

  .hero h1 {
    margin: 0;
    font-size: clamp(2rem, 4vw, 3rem);
  }

  .eyebrow {
    margin: 0 0 0.5rem;
    font-size: 0.75rem;
    letter-spacing: 0.14em;
    color: var(--text-3);
  }

  .lede,
  .muted {
    color: var(--text-2);
    line-height: 1.5;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 1rem;
  }

  .card {
    display: grid;
    gap: 0.9rem;
    padding: 1.25rem;
    border: 1px solid var(--border);
    border-radius: 22px;
    background: color-mix(in srgb, var(--bg) 88%, white 4%);
  }

  .span-2 {
    grid-column: span 2;
  }

  .status-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 1rem;
  }

  .status-grid article {
    padding: 1rem;
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.03);
  }

  .status-grid span {
    display: block;
    margin-bottom: 0.3rem;
    color: var(--text-3);
  }

  .status-grid span.ok {
    color: #58d68d;
  }

  .provider-tabs,
  .row {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .provider-tabs button,
  .action,
  .ghost {
    padding: 0.75rem 1rem;
    border-radius: 999px;
    border: 1px solid var(--border);
  }

  .provider-tabs button.active,
  .action {
    background: linear-gradient(135deg, #53a0ff, #7a6cff);
    color: white;
    border-color: transparent;
  }

  .ghost {
    background: transparent;
    color: var(--text-1);
  }

  label {
    display: grid;
    gap: 0.35rem;
  }

  label span {
    color: var(--text-2);
    font-size: 0.85rem;
  }

  input,
  textarea {
    width: 100%;
    border-radius: 14px;
    border: 1px solid var(--border);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text-1);
    padding: 0.85rem 1rem;
  }

  .preview-block {
    padding: 1rem;
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.03);
  }

  .preview-block p {
    margin-bottom: 0;
  }

  .soul-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 1rem;
  }

  .wide {
    grid-column: span 2;
  }

  .message {
    margin: 0;
    padding: 0.9rem 1rem;
    border-radius: 14px;
  }

  .message.error {
    background: rgba(255, 104, 104, 0.12);
    color: #ff9e9e;
  }

  .message.success {
    background: rgba(88, 214, 141, 0.12);
    color: #8ff0b4;
  }

  @media (max-width: 900px) {
    .grid,
    .status-grid,
    .soul-grid {
      grid-template-columns: 1fr;
    }

    .span-2,
    .wide {
      grid-column: span 1;
    }
  }
</style>
