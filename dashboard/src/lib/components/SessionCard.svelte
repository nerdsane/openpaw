<script lang="ts">
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import type { Session } from '$lib/types';
  import { getEntity } from '$lib/api';
  import StatusBadge from './StatusBadge.svelte';

  let { session }: { session: Session } = $props();

  let agentLabel = $state<string | null>(null);

  // Simple in-memory cache for agent/soul name resolution
  const nameCache = new Map<string, string>();

  onMount(async () => {
    // Prefer agent_id (shows role like "SWE"), fall back to soul_id
    if (session.agent_id) {
      const cacheKey = `agent:${session.agent_id}`;
      if (nameCache.has(cacheKey)) {
        agentLabel = nameCache.get(cacheKey) ?? null;
      } else {
        try {
          const agent = await getEntity('Agents', session.agent_id);
          const f = (agent as Record<string, unknown>);
          const name = (f.name ?? f.role ?? null) as string | null;
          if (name) {
            nameCache.set(cacheKey, name);
            agentLabel = name;
          }
        } catch {
          // Agent not found — use soul_id fallback below
        }
      }
    }
    if (!agentLabel && session.soul_id) {
      const cacheKey = `soul:${session.soul_id}`;
      if (nameCache.has(cacheKey)) {
        agentLabel = nameCache.get(cacheKey) ?? null;
      } else {
        try {
          const soul = await getEntity('Souls', session.soul_id);
          const name = (soul as { name?: string; Name?: string }).name
            ?? (soul as { Name?: string }).Name ?? null;
          if (name) {
            nameCache.set(cacheKey, name);
            agentLabel = name;
          }
        } catch {
          // Soul entity not found — soul_id might be a name string (e.g. "SWE")
          agentLabel = session.soul_id;
          nameCache.set(cacheKey, session.soul_id);
        }
      }
    }
  });

  let shortId = $derived(session.Id?.slice(0, 8) ?? '');

  let isActive = $derived(
    !['Completed', 'Failed', 'Cancelled'].includes(session.Status)
  );

  let truncatedMessage = $derived.by(() => {
    if (!session.user_message) return null;
    return session.user_message.length > 80
      ? session.user_message.slice(0, 80) + '...'
      : session.user_message;
  });

  let relativeTime = $derived.by(() => {
    if (!session.last_heartbeat_at) return null;
    // Handle non-ISO values like "alive"
    const then = new Date(session.last_heartbeat_at).getTime();
    if (isNaN(then)) return session.last_heartbeat_at; // Show raw value if not a date
    const now = Date.now();
    const diff = Math.max(0, now - then);
    const seconds = Math.floor(diff / 1000);
    if (seconds < 60) return `${seconds}s ago`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
  });

  let totalTokens = $derived((session.input_tokens ?? 0) + (session.output_tokens ?? 0));
  let costDisplay = $derived.by(() => {
    const c = parseFloat(session.cost_cents || '0');
    if (c === 0) return '--';
    return `$${(c / 100).toFixed(4)}`;
  });
</script>

<a
  href="/sessions/{session.Id}"
  class="session-card card"
  class:session-card--active={isActive}
  transition:fade={{ duration: 200 }}
>
  <div class="session-card__header">
    <h3 class="session-card__name">{agentLabel ?? 'Session'}</h3>
    <code class="session-card__id">{shortId}</code>
  </div>

  <StatusBadge status={session.Status} />

  {#if truncatedMessage}
    <p class="session-card__task">{truncatedMessage}</p>
  {/if}

  <div class="session-card__stats">
    <span class="stat">
      <span class="stat-label">turns</span>
      <span class="stat-value">{session.turn_count ?? 0}</span>
    </span>
    <span class="stat">
      <span class="stat-label">tokens</span>
      <span class="stat-value">{totalTokens.toLocaleString()}</span>
    </span>
    <span class="stat">
      <span class="stat-label">cost</span>
      <span class="stat-value">{costDisplay}</span>
    </span>
  </div>

  {#if relativeTime}
    <div class="session-card__time">{relativeTime}</div>
  {/if}
</a>

<style>
  .session-card {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    text-decoration: none;
    color: inherit;
    transition: box-shadow var(--duration-fast) var(--ease),
                transform var(--duration-fast) var(--ease);
    position: relative;
    overflow: hidden;
  }

  .session-card:hover {
    text-decoration: none;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
  }

  .session-card--active::before {
    content: '';
    position: absolute;
    top: 50%;
    left: 50%;
    width: 120%;
    height: 120%;
    transform: translate(-50%, -50%);
    border-radius: 50%;
    background: radial-gradient(
      circle,
      var(--status-active) 0%,
      transparent 70%
    );
    opacity: 0;
    animation: pulse-glow 3s ease-in-out infinite;
    pointer-events: none;
  }

  @keyframes pulse-glow {
    0%, 100% {
      opacity: 0;
    }
    50% {
      opacity: 0.04;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .session-card--active::before {
      animation: none;
      opacity: 0.02;
    }
  }

  .session-card__header {
    display: flex;
    align-items: baseline;
    gap: var(--space-1);
  }

  .session-card__name {
    font-size: var(--text-base);
    font-weight: 600;
  }

  .session-card__id {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }

  .session-card__task {
    font-size: var(--text-sm);
    color: var(--text-secondary);
    line-height: 1.5;
    margin-top: 4px;
  }

  .session-card__stats {
    display: flex;
    gap: var(--space-2);
    margin-top: var(--space-1);
  }

  .stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .stat-label {
    font-size: 0.625rem;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .stat-value {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }

  .session-card__time {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    margin-top: 4px;
  }
</style>
