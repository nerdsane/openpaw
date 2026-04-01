<script lang="ts">
  import type { AgentTurn } from '$lib/types';
  import { formatToolInput } from '$lib/parse';

  let { turns }: { turns: AgentTurn[] } = $props();

  let expandedTurns = $state<Set<number>>(new Set());

  function toggleExpand(turnNumber: number) {
    const next = new Set(expandedTurns);
    if (next.has(turnNumber)) next.delete(turnNumber);
    else next.add(turnNumber);
    expandedTurns = next;
  }

  function formatTime(ts: string): string {
    if (!ts) return '';
    const d = new Date(ts);
    if (isNaN(d.getTime())) return '';
    return d.toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }

  function authzLabel(authz?: { allowed: boolean }): string {
    if (!authz) return '';
    return authz.allowed ? 'ALLOWED' : 'DENIED';
  }

  function authzClass(authz?: { allowed: boolean }): string {
    if (!authz) return '';
    return authz.allowed ? 'authz--allowed' : 'authz--denied';
  }

  function toolInputPreview(name: string, input: Record<string, unknown>): string {
    const formatted = formatToolInput(name, input);
    if (formatted.length <= 120) return formatted;
    return formatted.slice(0, 120) + '...';
  }

  function hasExpandableInput(name: string, input: Record<string, unknown>): boolean {
    // Check if there's more detail to show (edit has old_string/new_string, write has content, etc.)
    if (name === 'edit') return !!(input.old_string || input.new_string);
    if (name === 'write') return !!(input.content);
    if (name === 'bash') return ((input.command as string) ?? '').length > 120;
    if (name === 'temper_action') return !!input.body && JSON.stringify(input.body).length > 2;
    return false;
  }

  function expandedContent(name: string, input: Record<string, unknown>): string {
    if (name === 'edit') {
      let out = '';
      if (input.old_string) out += `--- old\n${input.old_string}\n`;
      if (input.new_string) out += `+++ new\n${input.new_string}`;
      return out;
    }
    if (name === 'write') return (input.content as string) ?? '';
    if (name === 'bash') return (input.command as string) ?? '';
    if (name === 'temper_action' && input.body) return JSON.stringify(input.body, null, 2);
    return JSON.stringify(input, null, 2);
  }
</script>

<div class="terminal">
  {#if turns.length === 0}
    <div class="terminal-empty">
      <span class="comment"># No tool calls recorded</span>
    </div>
  {:else}
    {#each turns as turn (turn.number)}
      <div class="turn">
        <div class="turn-header">
          <span class="comment"># Turn {turn.number}</span>
          {#if turn.timestamp}
            <span class="comment"> — {formatTime(turn.timestamp)}</span>
          {/if}
          {#if turn.authz}
            <span class="comment"> — </span><span class={authzClass(turn.authz)}>{authzLabel(turn.authz)}</span>
          {/if}
        </div>

        {#if turn.toolCalls.length === 0}
          <div class="tool-line">
            <span class="comment"># (tool call data not available)</span>
          </div>
        {:else}
          {#each turn.toolCalls as tc, i}
            <div class="tool-block">
              <div class="tool-line">
                <span class="prompt">&gt;</span>
                <span class="tool-name">{tc.name}</span>
                <span class="tool-args">{toolInputPreview(tc.name, tc.input)}</span>
              </div>

              {#if hasExpandableInput(tc.name, tc.input)}
                <button class="expand-btn" onclick={() => toggleExpand(turn.number * 100 + i)}>
                  {expandedTurns.has(turn.number * 100 + i) ? '[-]' : '[+]'} details
                </button>
                {#if expandedTurns.has(turn.number * 100 + i)}
                  <pre class="tool-detail">{expandedContent(tc.name, tc.input)}</pre>
                {/if}
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    {/each}
  {/if}
</div>

<style>
  .terminal {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    line-height: 1.35;
    color: var(--text-secondary);
    background: var(--surface-base);
    overflow-y: auto;
  }

  .terminal-empty {
    padding: var(--space-4) 0;
    text-align: center;
  }

  .turn {
    padding: 6px 0;
    border-bottom: 1px solid var(--border);
  }

  .turn-header {
    margin-bottom: 4px;
  }

  .comment {
    color: var(--text-tertiary);
  }

  .authz--allowed {
    color: var(--status-success);
  }

  .authz--denied {
    color: var(--status-error);
  }

  .tool-block {
    margin: 2px 0;
  }

  .tool-line {
    display: flex;
    gap: 6px;
    align-items: baseline;
    flex-wrap: wrap;
  }

  .prompt {
    color: var(--text-tertiary);
    flex-shrink: 0;
  }

  .tool-name {
    color: var(--status-success);
    font-weight: 500;
    flex-shrink: 0;
  }

  .tool-args {
    color: var(--text-primary);
    word-break: break-all;
  }

  .expand-btn {
    font-family: var(--font-mono);
    font-size: 0.625rem;
    color: var(--text-tertiary);
    padding: 0;
    margin-left: 16px;
    cursor: pointer;
  }

  .expand-btn:hover {
    color: var(--text-secondary);
  }

  .tool-detail {
    margin: 2px 0 4px 16px;
    padding: 4px 8px;
    background: var(--surface-raised);
    color: var(--text-secondary);
    font-size: 0.6875rem;
    line-height: 1.3;
    max-height: 200px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-all;
  }

  @media (max-width: 768px) {
    .terminal { font-size: 0.6875rem; }
    .tool-detail { margin-left: 8px; }
  }
</style>
