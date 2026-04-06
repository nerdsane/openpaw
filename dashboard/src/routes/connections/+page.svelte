<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getTransportStatus, connectDiscord, disconnectDiscord, connectSlack, disconnectSlack } from '$lib/api';
  import type { TransportStatusResponse } from '$lib/api';

  let status = $state<TransportStatusResponse | null>(null);
  let pollInterval: ReturnType<typeof setInterval>;

  // Discord form
  let discordToken = $state('');
  let discordPublicKey = $state('');
  let discordGuildId = $state('');
  let discordFeedChannelId = $state('');
  let discordForumChannelId = $state('');
  let discordConnecting = $state(false);

  // Slack form
  let slackAppToken = $state('');
  let slackBotToken = $state('');
  let slackSigningSecret = $state('');
  let slackConnecting = $state(false);

  let showDiscordForm = $state(false);
  let showSlackForm = $state(false);

  async function refresh() {
    try {
      status = await getTransportStatus();
    } catch { /* ignore */ }
  }

  async function handleConnectDiscord() {
    discordConnecting = true;
    try {
      await connectDiscord({
        bot_token: discordToken,
        public_key: discordPublicKey || undefined,
        guild_id: discordGuildId || undefined,
        feed_channel_id: discordFeedChannelId || undefined,
        forum_channel_id: discordForumChannelId || undefined,
      });
      showDiscordForm = false;
      discordToken = '';
      await refresh();
    } catch (e) {
      alert(`Failed: ${e}`);
    } finally {
      discordConnecting = false;
    }
  }

  async function handleDisconnectDiscord() {
    await disconnectDiscord();
    await refresh();
  }

  async function handleConnectSlack() {
    slackConnecting = true;
    try {
      await connectSlack({
        app_token: slackAppToken,
        bot_token: slackBotToken,
        signing_secret: slackSigningSecret || undefined,
      });
      showSlackForm = false;
      slackAppToken = '';
      slackBotToken = '';
      await refresh();
    } catch (e) {
      alert(`Failed: ${e}`);
    } finally {
      slackConnecting = false;
    }
  }

  async function handleDisconnectSlack() {
    await disconnectSlack();
    await refresh();
  }

  function statusColor(s: string): string {
    if (s === 'connected') return 'var(--status-active)';
    if (s === 'connecting') return 'var(--status-warning)';
    if (s === 'error') return 'var(--status-error)';
    return 'var(--status-idle)';
  }

  onMount(() => {
    refresh();
    pollInterval = setInterval(refresh, 5000);
  });

  onDestroy(() => {
    clearInterval(pollInterval);
  });
</script>

<div class="page">
  <h1 class="page-title">CONNECTIONS</h1>

  <div class="cards">
    <!-- Discord -->
    <div class="card">
      <div class="card-header">
        <div class="card-title-row">
          <span class="dot" style="background: {statusColor(status?.discord?.status ?? 'disconnected')}"></span>
          <span class="card-title">DISCORD</span>
          <span class="card-status">{status?.discord?.status ?? 'disconnected'}</span>
        </div>
        {#if status?.discord?.guild_id}
          <span class="card-detail">Guild: {status.discord.guild_id}</span>
        {/if}
      </div>

      <div class="card-actions">
        {#if status?.discord?.status === 'connected' || status?.discord?.status === 'connecting'}
          <button class="btn btn-danger" onclick={handleDisconnectDiscord}>DISCONNECT</button>
        {:else}
          <button class="btn btn-primary" onclick={() => showDiscordForm = !showDiscordForm}>
            {showDiscordForm ? 'CANCEL' : 'CONNECT'}
          </button>
        {/if}
      </div>

      {#if showDiscordForm}
        <form class="connect-form" onsubmit={(e) => { e.preventDefault(); handleConnectDiscord(); }}>
          <label>
            <span class="label-text">Bot Token *</span>
            <input type="password" bind:value={discordToken} required placeholder="MTI3..." />
          </label>
          <label>
            <span class="label-text">Public Key</span>
            <input type="text" bind:value={discordPublicKey} placeholder="Optional" />
          </label>
          <label>
            <span class="label-text">Guild ID</span>
            <input type="text" bind:value={discordGuildId} placeholder="Optional" />
          </label>
          <label>
            <span class="label-text">Feed Channel ID</span>
            <input type="text" bind:value={discordFeedChannelId} placeholder="Optional" />
          </label>
          <label>
            <span class="label-text">Forum Channel ID</span>
            <input type="text" bind:value={discordForumChannelId} placeholder="Optional" />
          </label>
          <button class="btn btn-primary" type="submit" disabled={discordConnecting || !discordToken}>
            {discordConnecting ? 'CONNECTING...' : 'CONNECT'}
          </button>
        </form>
      {/if}
    </div>

    <!-- Slack -->
    <div class="card">
      <div class="card-header">
        <div class="card-title-row">
          <span class="dot" style="background: {statusColor(status?.slack?.status ?? 'disconnected')}"></span>
          <span class="card-title">SLACK</span>
          <span class="card-status">{status?.slack?.status ?? 'disconnected'}</span>
        </div>
      </div>

      <div class="card-actions">
        {#if status?.slack?.status === 'connected' || status?.slack?.status === 'connecting'}
          <button class="btn btn-danger" onclick={handleDisconnectSlack}>DISCONNECT</button>
        {:else}
          <button class="btn btn-primary" onclick={() => showSlackForm = !showSlackForm}>
            {showSlackForm ? 'CANCEL' : 'CONNECT'}
          </button>
        {/if}
      </div>

      {#if showSlackForm}
        <form class="connect-form" onsubmit={(e) => { e.preventDefault(); handleConnectSlack(); }}>
          <label>
            <span class="label-text">App Token *</span>
            <input type="password" bind:value={slackAppToken} required placeholder="xapp-..." />
          </label>
          <label>
            <span class="label-text">Bot Token *</span>
            <input type="password" bind:value={slackBotToken} required placeholder="xoxb-..." />
          </label>
          <label>
            <span class="label-text">Signing Secret</span>
            <input type="text" bind:value={slackSigningSecret} placeholder="Optional" />
          </label>
          <button class="btn btn-primary" type="submit" disabled={slackConnecting || !slackAppToken || !slackBotToken}>
            {slackConnecting ? 'CONNECTING...' : 'CONNECT'}
          </button>
        </form>
      {/if}
    </div>
  </div>
</div>

<style>
  .page { max-width: 800px; }
  .page-title {
    font-family: var(--font-display);
    font-size: var(--heading);
    color: var(--text-display);
    margin-bottom: var(--space-xl);
    letter-spacing: 0.1em;
  }

  .cards { display: flex; flex-direction: column; gap: var(--space-lg); }

  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: var(--space-lg);
  }

  .card-header { margin-bottom: var(--space-md); }

  .card-title-row {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .card-title {
    font-family: var(--font-mono);
    font-size: var(--body);
    font-weight: 700;
    color: var(--text-display);
    letter-spacing: 0.08em;
  }

  .card-status {
    font-family: var(--font-mono);
    font-size: var(--caption);
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .card-detail {
    display: block;
    font-family: var(--font-mono);
    font-size: var(--caption);
    color: var(--text-disabled);
    margin-top: var(--space-xs);
    margin-left: calc(8px + var(--space-sm));
  }

  .card-actions { margin-bottom: var(--space-sm); }

  .connect-form {
    display: flex;
    flex-direction: column;
    gap: var(--space-sm);
    margin-top: var(--space-md);
    padding-top: var(--space-md);
    border-top: 1px solid var(--border);
  }

  .connect-form label {
    display: flex;
    flex-direction: column;
    gap: var(--space-2xs);
  }

  .label-text {
    font-family: var(--font-mono);
    font-size: var(--caption);
    color: var(--text-secondary);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .connect-form input {
    background: var(--terminal-bg);
    border: 1px solid var(--border-visible);
    border-radius: var(--radius-sm);
    padding: var(--space-sm) var(--space-md);
    font-family: var(--font-mono);
    font-size: var(--body-sm);
    color: var(--text-primary);
  }

  .connect-form input::placeholder { color: var(--text-disabled); }
  .connect-form input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .btn {
    font-family: var(--font-mono);
    font-size: var(--caption);
    letter-spacing: 0.1em;
    padding: var(--space-sm) var(--space-lg);
    border-radius: var(--radius-sm);
    cursor: pointer;
    border: 1px solid transparent;
    transition: all var(--duration-fast) var(--ease);
  }

  .btn-primary {
    background: var(--accent);
    color: var(--black);
    font-weight: 700;
  }
  .btn-primary:hover { opacity: 0.9; }
  .btn-primary:disabled { opacity: 0.4; cursor: not-allowed; }

  .btn-danger {
    background: transparent;
    color: var(--status-error);
    border-color: var(--status-error);
  }
  .btn-danger:hover { background: rgba(232, 91, 129, 0.1); }
</style>
