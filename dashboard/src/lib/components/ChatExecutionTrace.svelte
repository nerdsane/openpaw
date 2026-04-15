<script lang="ts">
  import { slide } from 'svelte/transition';
  import type { SessionTurn } from '$lib/parse';
  import { formatToolInput } from '$lib/parse';

  let {
    turns = [],
    status = 'thinking',
  }: {
    turns: SessionTurn[];
    status: 'thinking' | 'executing' | 'completed' | 'failed';
  } = $props();

  let expanded = $state(false);

  let totalToolCalls = $derived(turns.reduce((n, t) => n + t.toolCalls.length, 0));

  let statusLabel = $derived.by(() => {
    switch (status) {
      case 'thinking': return 'THINKING';
      case 'executing': return `EXECUTING (${totalToolCalls} tool call${totalToolCalls !== 1 ? 's' : ''})`;
      case 'completed': return '';
      case 'failed': return 'FAILED';
      default: return '';
    }
  });

  function toolPreview(name: string, input: Record<string, unknown>): string {
    const formatted = formatToolInput(name, input);
    return formatted.length <= 80 ? formatted : formatted.slice(0, 80) + '...';
  }
</script>

{#if status !== 'completed' || turns.length > 0}
  <div class="trace">
    {#if status !== 'completed' && statusLabel}
      <div class="trace-status" class:trace-status--failed={status === 'failed'}>
        <span class="trace-label">{statusLabel}</span>
        {#if status === 'thinking' || status === 'executing'}
          <span class="trace-cursor"></span>
        {/if}
      </div>
    {/if}

    {#if turns.length > 0}
      <button class="trace-toggle" onclick={() => expanded = !expanded}>
        {expanded ? '[-]' : '[+]'} {totalToolCalls} tool call{totalToolCalls !== 1 ? 's' : ''}
      </button>

      {#if expanded}
        <div class="trace-tools" transition:slide={{ duration: 150 }}>
          {#each turns as turn}
            {#each turn.toolCalls as tc}
              <div class="trace-tool">
                <span class="trace-prompt">&gt;</span>
                <span class="trace-tool-name">{tc.name}</span>
                <span class="trace-tool-args">{toolPreview(tc.name, tc.input)}</span>
              </div>
            {/each}
          {/each}
        </div>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .trace {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    line-height: 1.4;
    margin-top: var(--sp-1);
  }

  .trace-status {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    color: var(--accent, #4ade80);
    letter-spacing: 0.06em;
  }

  .trace-status--failed {
    color: var(--status-error);
  }

  .trace-label {
    font-weight: 600;
  }

  .trace-cursor {
    display: inline-block;
    width: 7px;
    height: 12px;
    background: currentColor;
    animation: blink 1s step-end infinite;
  }

  @keyframes blink {
    50% { opacity: 0; }
  }

  .trace-toggle {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-3);
    cursor: pointer;
    padding: 2px 0;
    letter-spacing: 0.04em;
  }

  .trace-toggle:hover {
    color: var(--text-1);
  }

  .trace-tools {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: var(--sp-1) 0;
  }

  .trace-tool {
    display: flex;
    gap: var(--sp-1);
    align-items: baseline;
    animation: tool-in 150ms var(--ease) both;
  }

  @keyframes tool-in {
    from { opacity: 0; transform: translateY(4px); }
  }

  .trace-prompt {
    color: var(--accent, #4ade80);
    flex-shrink: 0;
  }

  .trace-tool-name {
    color: var(--text-2);
    flex-shrink: 0;
  }

  .trace-tool-args {
    color: var(--text-3);
    word-break: break-all;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (prefers-reduced-motion: reduce) {
    .trace-cursor { animation: none; }
    .trace-tool { animation: none; }
  }
</style>
