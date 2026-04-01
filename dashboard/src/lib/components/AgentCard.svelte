<script lang="ts">
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import type { Agent } from '$lib/types';
  import { getEntity } from '$lib/api';
  import StatusBadge from './StatusBadge.svelte';

  let { agent }: { agent: Agent } = $props();

  let soulName = $state<string | null>(null);

  // Simple in-memory soul cache
  const soulCache = new Map<string, string>();

  onMount(async () => {
    if (agent.soul_id) {
      if (soulCache.has(agent.soul_id)) {
        soulName = soulCache.get(agent.soul_id) ?? null;
      } else {
        try {
          const soul = await getEntity('Souls', agent.soul_id);
          const name = (soul as { name?: string }).name ?? null;
          if (name) {
            soulCache.set(agent.soul_id, name);
            soulName = name;
          }
        } catch {
          // Soul entity not found by ID — soul_id might be a name string (e.g. "SWE")
          soulName = agent.soul_id;
          soulCache.set(agent.soul_id, agent.soul_id);
        }
      }
    }
  });

  let shortId = $derived(agent.Id?.slice(0, 8) ?? '');

  let isActive = $derived(
    !['Completed', 'Failed', 'Cancelled'].includes(agent.Status)
  );

  let truncatedMessage = $derived.by(() => {
    if (!agent.user_message) return null;
    return agent.user_message.length > 80
      ? agent.user_message.slice(0, 80) + '...'
      : agent.user_message;
  });

  let relativeTime = $derived.by(() => {
    if (!agent.last_heartbeat_at) return null;
    // Handle non-ISO values like "alive"
    const then = new Date(agent.last_heartbeat_at).getTime();
    if (isNaN(then)) return agent.last_heartbeat_at; // Show raw value if not a date
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

  let totalTokens = $derived((agent.input_tokens ?? 0) + (agent.output_tokens ?? 0));
  let costDisplay = $derived.by(() => {
    const c = parseFloat(agent.cost_cents || '0');
    if (c === 0) return '--';
    return `$${(c / 100).toFixed(4)}`;
  });
</script>

<a
  href="/agents/{agent.Id}"
  class="agent-card card"
  class:agent-card--active={isActive}
  transition:fade={{ duration: 200 }}
>
  <div class="agent-card__header">
    <h3 class="agent-card__name">{soulName ?? 'Agent'}</h3>
    <code class="agent-card__id">{shortId}</code>
  </div>

  <StatusBadge status={agent.Status} />

  {#if truncatedMessage}
    <p class="agent-card__task">{truncatedMessage}</p>
  {/if}

  <div class="agent-card__stats">
    <span class="stat">
      <span class="stat-label">turns</span>
      <span class="stat-value">{agent.turn_count ?? 0}</span>
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
    <div class="agent-card__time">{relativeTime}</div>
  {/if}
</a>

<style>
  .agent-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    text-decoration: none;
    color: inherit;
    transition: background var(--duration-fast) var(--ease),
                transform var(--duration-fast) var(--ease);
    position: relative;
  }

  .agent-card:hover {
    text-decoration: none;
    background: var(--surface-overlay);
    transform: translateY(-1px);
  }

  .agent-card__header {
    display: flex;
    align-items: baseline;
    gap: var(--space-1);
  }

  .agent-card__name {
    font-size: var(--text-base);
    font-weight: 600;
  }

  .agent-card__id {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }

  .agent-card__task {
    font-size: var(--text-sm);
    color: var(--text-secondary);
    line-height: 1.5;
  }

  .agent-card__stats {
    display: flex;
    gap: var(--space-2);
    margin-top: 2px;
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

  .agent-card__time {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
  }
</style>
