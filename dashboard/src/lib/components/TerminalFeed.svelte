<script lang="ts">
  import { base } from '$app/paths';
  import { slide } from 'svelte/transition';
  import type { SessionTurn } from '$lib/parse';
  import { formatToolInput } from '$lib/parse';
  import type { PolicyEntry } from '$lib/api';
  import { parseCedarPolicies, type ParsedPermission } from '$lib/utils/cedar-parse';
  import AuthzBadge from './AuthzBadge.svelte';

  let {
    turns,
    userMessage = null,
    stateTransitions = [],
    policies = [],
  }: {
    turns: SessionTurn[];
    userMessage?: string | null;
    stateTransitions?: Array<{ action: string; from: string; to: string; timestamp: string }>;
    policies?: PolicyEntry[];
  } = $props();

  let expandedItems = $state<Set<number>>(new Set());
  let expandedAuthz = $state<Set<number>>(new Set());
  let expandedPolicy = $state<Set<number>>(new Set());
  let terminalEl = $state<HTMLDivElement | null>(null);

  function toggleExpand(key: number) {
    const next = new Set(expandedItems);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    expandedItems = next;
  }

  function toggleAuthz(key: number) {
    const next = new Set(expandedAuthz);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    expandedAuthz = next;
  }

  function togglePolicy(key: number) {
    const next = new Set(expandedPolicy);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    expandedPolicy = next;
  }

  // Auto-scroll to bottom when turns change
  $effect(() => {
    turns;
    if (terminalEl) {
      requestAnimationFrame(() => {
        terminalEl!.scrollTop = terminalEl!.scrollHeight;
      });
    }
  });

  // Parse all policies once to find matching rules per action
  let parsedPolicies = $derived.by(() => {
    const all: Array<ParsedPermission & { source?: string; created_by?: string }> = [];
    for (const p of policies) {
      const parsed = parseCedarPolicies(p.cedar_text);
      for (const rule of parsed) {
        all.push({ ...rule, source: p.source, created_by: p.created_by });
      }
    }
    return all;
  });

  function findMatchingPolicies(actionName: string): typeof parsedPolicies {
    if (parsedPolicies.length === 0) return [];
    // Tool calls on sessions go through ProcessToolCalls Cedar action.
    // Match: exact action, wildcards, or actions containing the tool name.
    const matches = parsedPolicies.filter(p => {
      const a = p.action.toLowerCase();
      const r = p.resource.toLowerCase();
      const resourceMatch = r === '*' || r === 'session';
      if (!resourceMatch) return false;
      // Exact match on ProcessToolCalls or HandleToolResults (the dispatch pipeline)
      if (a === 'processtoolcalls' || a === 'handletoolresults') return true;
      // Wildcard action (principal is Admin, action, resource is Session)
      if (a === '*') return true;
      // Tool name match
      if (a === actionName.toLowerCase()) return true;
      return false;
    });
    // If no specific match, broaden: show all permit rules for Session
    if (matches.length === 0) {
      return parsedPolicies.filter(p => {
        const r = p.resource.toLowerCase();
        return (r === '*' || r === 'session') && p.verdict === 'permit';
      });
    }
    return matches;
  }

  function formatTime(ts: string): string {
    if (!ts) return '';
    const d = new Date(ts);
    if (isNaN(d.getTime())) return '';
    return d.toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }

  function toolInputPreview(name: string, input: Record<string, unknown>): string {
    const formatted = formatToolInput(name, input);
    if (formatted.length <= 120) return formatted;
    return formatted.slice(0, 120) + '...';
  }

  /** Full content for any tool call — always available */
  function fullContent(name: string, input: Record<string, unknown>): string {
    switch (name) {
      case 'edit': {
        let out = '';
        if (input.file_path) out += `file: ${input.file_path}\n`;
        if (input.old_string) out += `--- old\n${input.old_string}\n`;
        if (input.new_string) out += `+++ new\n${input.new_string}`;
        return out || JSON.stringify(input, null, 2);
      }
      case 'write':
        return `file: ${input.file_path ?? ''}\n\n${(input.content as string) ?? ''}`;
      case 'bash':
        return (input.command as string) ?? JSON.stringify(input, null, 2);
      case 'read':
        return `file: ${input.file_path ?? input.path ?? ''}\n${input.offset ? `offset: ${input.offset}` : ''}${input.limit ? ` limit: ${input.limit}` : ''}`;
      case 'temper_action':
        return JSON.stringify(input, null, 2);
      case 'temper_get':
      case 'temper_list':
      case 'temper_create':
        return JSON.stringify(input, null, 2);
      default:
        return JSON.stringify(input, null, 2);
    }
  }

  /** Check if full content is longer than the preview (always expandable for non-trivial inputs) */
  function isExpandable(name: string, input: Record<string, unknown>): boolean {
    const preview = formatToolInput(name, input);
    const full = fullContent(name, input);
    return full.length > preview.length || full.length > 120;
  }
</script>

<div class="terminal" bind:this={terminalEl}>
  {#if userMessage}
    <div class="user-message">
      <span class="prompt">$</span>
      <span class="user-text">{userMessage}</span>
    </div>
  {/if}

  {#if turns.length === 0}
    <div class="terminal-empty">
      <span class="comment"># WAITING...</span>
      <span class="cursor"></span>
    </div>
  {:else}
    {#each turns as turn, ti (turn.number)}
      <div class="turn" style:animation-delay="{ti * 15}ms">
        <div class="turn-header">
          <span class="comment"># TURN {turn.number}</span>
          {#if turn.timestamp}
            <span class="comment"> — {formatTime(turn.timestamp)}</span>
          {/if}
          {#if turn.inputTokens > 0}
            <span class="comment"> — ctx:{turn.inputTokens.toLocaleString()} out:{turn.outputTokens.toLocaleString()}</span>
          {/if}
          {#if turn.authz}
            <span class="turn-authz">
              <AuthzBadge
                allowed={turn.authz.allowed}
                resource={turn.authz.denied_resource ?? null}
              />
            </span>
          {/if}
        </div>

        {#if turn.toolCalls.length === 0}
          <div class="tool-line-static">
            <span class="comment"># (tool call data not available)</span>
          </div>
        {:else}
          {#each turn.toolCalls as tc, i}
            {@const toolKey = turn.number * 100 + i}
            {@const matchingPolicies = findMatchingPolicies(tc.name)}
            <div class="tool-block" class:tool-block--denied={turn.authz && !turn.authz.allowed}>
              <!-- Tool call line (always clickable to expand details) -->
              <div class="tool-line-row">
                <span class="prompt">&gt;</span>
                <span class="tool-name">{tc.name}</span>
                <span class="tool-args">{toolInputPreview(tc.name, tc.input)}</span>
                {#if turn.authz}
                  <span class="tool-authz-inline">
                    <AuthzBadge
                      allowed={turn.authz.allowed}
                      resource={turn.authz.denied_resource ?? null}
                    />
                  </span>
                {/if}
              </div>

              <!-- Expand/collapse controls -->
              <div class="tool-actions">
                {#if isExpandable(tc.name, tc.input)}
                  <button class="expand-btn" onclick={() => toggleExpand(toolKey)}>
                    {expandedItems.has(toolKey) ? '[-]' : '[+]'} FULL OUTPUT
                  </button>
                {/if}
                <button class="expand-btn" onclick={() => toggleAuthz(toolKey)}>
                  {expandedAuthz.has(toolKey) ? '[-]' : '[+]'} AUTHORIZATION
                </button>
              </div>

              <!-- Full tool call content -->
              {#if expandedItems.has(toolKey)}
                <pre class="tool-detail" transition:slide={{ duration: 150 }}>{fullContent(tc.name, tc.input)}</pre>
              {/if}

              <!-- Authorization drill-down -->
              {#if expandedAuthz.has(toolKey)}
                <div class="authz-detail" transition:slide={{ duration: 150 }}>
                  <div class="authz-row">
                    <span class="authz-label">TOOL</span>
                    <span class="authz-value">{tc.name}</span>
                  </div>
                  <div class="authz-row">
                    <span class="authz-label">ACTION</span>
                    <span class="authz-value">ProcessToolCalls</span>
                  </div>
                  <div class="authz-row">
                    <span class="authz-label">RESOURCE</span>
                    <span class="authz-value">Session</span>
                  </div>
                  <div class="authz-row">
                    <span class="authz-label">VERDICT</span>
                    <span class="authz-value">
                      <AuthzBadge
                        allowed={turn.authz?.allowed ?? true}
                        resource={turn.authz?.denied_resource ?? null}
                      />
                    </span>
                  </div>
                  {#if turn.authz && !turn.authz.allowed && turn.authz.denied_resource}
                    <div class="authz-row">
                      <span class="authz-label">DENIED RESOURCE</span>
                      <span class="authz-value authz-value--denied">{turn.authz.denied_resource}</span>
                    </div>
                  {/if}

                  <!-- Matching policies -->
                  {#if matchingPolicies.length > 0}
                    <div class="authz-policies">
                      <span class="authz-policies-label">MATCHING POLICIES ({matchingPolicies.length})</span>
                      {#each matchingPolicies as mp, pi}
                        <div class="authz-policy-row">
                          <span class="policy-verdict" class:policy-verdict--permit={mp.verdict === 'permit'} class:policy-verdict--forbid={mp.verdict === 'forbid'}>
                            {mp.verdict.toUpperCase()}
                          </span>
                          <span class="policy-principal">{mp.principal === '*' ? 'any principal' : mp.principal}</span>
                          <span class="policy-action">{mp.actionDisplay}</span>
                          <span class="policy-resource">on {mp.resource}</span>
                          {#if mp.condition}
                            <span class="policy-condition">when {mp.condition}</span>
                          {/if}
                          <div class="policy-meta">
                            {#if mp.source}
                              <span class="policy-source">{mp.source}</span>
                            {/if}
                            {#if mp.created_by}
                              <span class="policy-author">by {mp.created_by}</span>
                            {/if}
                            <button class="policy-raw-btn" onclick={() => togglePolicy(toolKey * 1000 + pi)}>
                              {expandedPolicy.has(toolKey * 1000 + pi) ? '[-]' : '[+]'} CEDAR
                            </button>
                          </div>
                          {#if expandedPolicy.has(toolKey * 1000 + pi)}
                            <pre class="policy-raw" transition:slide={{ duration: 150 }}>{mp.rawLine}</pre>
                          {/if}
                        </div>
                      {/each}
                    </div>
                  {:else if policies.length > 0}
                    <div class="authz-row">
                      <span class="authz-label">POLICY</span>
                      <span class="authz-value authz-value--dim">Permitted by a policy not parseable client-side. <a href="{base}/permissions" class="policy-link">View all policies</a></span>
                    </div>
                  {:else}
                    <div class="authz-row">
                      <span class="authz-label">POLICY</span>
                      <span class="authz-value authz-value--dim">No policies loaded. <a href="{base}/permissions" class="policy-link">View policies</a></span>
                    </div>
                  {/if}
                </div>
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
    font-size: var(--text-xs);
    line-height: 1.4;
    color: var(--text-2);
    background: var(--terminal-bg);
    overflow-y: auto;
    padding: var(--sp-4);
    border-radius: var(--radius);
  }

  .user-message {
    display: flex;
    gap: var(--sp-1);
    align-items: baseline;
    padding: var(--sp-2) 0;
    border-bottom: 1px solid var(--terminal-border);
    margin-bottom: var(--sp-2);
  }

  .user-message .prompt {
    color: var(--terminal-text);
    font-weight: 700;
  }

  .user-text {
    color: var(--text-1);
    word-break: break-word;
  }

  .terminal-empty {
    padding: var(--sp-8) 0;
    text-align: center;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--sp-2);
  }

  .cursor {
    display: inline-block;
    width: 8px;
    height: 14px;
    background: var(--terminal-cursor);
    animation: blink 1s step-end infinite;
  }

  @keyframes blink {
    50% { opacity: 0; }
  }

  .turn {
    padding: var(--sp-2) 0;
    border-bottom: 1px solid var(--terminal-border);
    animation: turn-in 150ms var(--ease) both;
  }

  @keyframes turn-in {
    from { opacity: 0; }
  }

  .turn-header {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin-bottom: var(--sp-1);
  }

  .turn-authz {
    margin-left: auto;
  }

  .comment {
    color: var(--terminal-dim);
    font-size: var(--text-xs);
    letter-spacing: 0.04em;
  }

  .tool-block {
    margin: var(--sp-1) 0;
  }

  .tool-block--denied {
    background: var(--authz-denied-bg);
    border-left: 2px solid var(--authz-denied-border);
    padding-left: var(--sp-2);
    margin-left: calc(-1 * var(--sp-2));
  }

  .tool-line-row {
    display: flex;
    gap: var(--sp-1);
    align-items: baseline;
    flex-wrap: wrap;
  }

  .tool-line-static {
    display: flex;
    gap: var(--sp-1);
    align-items: baseline;
  }

  .prompt {
    color: var(--terminal-text);
    flex-shrink: 0;
  }

  .tool-name {
    color: var(--terminal-text);
    font-weight: 400;
    flex-shrink: 0;
  }

  .tool-args {
    color: var(--text-1);
    word-break: break-all;
  }

  .tool-authz-inline {
    margin-left: auto;
    flex-shrink: 0;
  }

  .tool-actions {
    display: flex;
    gap: var(--sp-4);
    margin-top: 2px;
    margin-left: var(--sp-4);
  }

  .expand-btn {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--terminal-dim);
    padding: 0;
    cursor: pointer;
    letter-spacing: 0.04em;
    transition: color var(--duration) var(--ease);
  }

  .expand-btn:hover { color: var(--terminal-text); }

  .tool-detail {
    margin: var(--sp-1) 0 var(--sp-1) var(--sp-4);
    padding: var(--sp-2);
    background: var(--bg);
    border: 1px solid var(--terminal-border);
    color: var(--text-1);
    font-size: var(--text-xs);
    line-height: 1.4;
    max-height: 300px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-all;
    border-radius: var(--radius);
  }

  .authz-detail {
    margin: var(--sp-1) 0 var(--sp-1) var(--sp-4);
    padding: var(--sp-4);
    background: var(--bg);
    border: 1px solid var(--terminal-border);
    border-radius: var(--radius);
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .authz-row {
    display: flex;
    gap: var(--sp-4);
    align-items: baseline;
  }

  .authz-label {
    font-size: var(--text-xs);
    letter-spacing: 0.08em;
    color: var(--terminal-dim);
    min-width: 130px;
    flex-shrink: 0;
  }

  .authz-value {
    font-size: var(--text-xs);
    color: var(--text-1);
  }

  .authz-value--denied {
    color: var(--status-error);
  }

  .authz-value--dim {
    color: var(--text-3);
  }

  /* Policy matching section */
  .authz-policies {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin-top: var(--sp-1);
    padding-top: var(--sp-2);
    border-top: 1px solid var(--terminal-border);
  }

  .authz-policies-label {
    font-size: var(--text-xs);
    letter-spacing: 0.08em;
    color: var(--terminal-dim);
  }

  .authz-policy-row {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--sp-2);
    padding: var(--sp-1) 0;
    border-bottom: 1px solid var(--terminal-border);
  }

  .authz-policy-row:last-child {
    border-bottom: none;
  }

  .policy-verdict {
    font-size: var(--text-xs);
    letter-spacing: 0.06em;
    font-weight: 700;
  }

  .policy-verdict--permit { color: var(--accent); }
  .policy-verdict--forbid { color: var(--status-error); }

  .policy-principal {
    font-size: var(--text-xs);
    color: var(--text-2);
  }

  .policy-action {
    font-size: var(--text-xs);
    color: var(--text-1);
  }

  .policy-resource {
    font-size: var(--text-xs);
    color: var(--text-2);
  }

  .policy-condition {
    font-size: var(--text-xs);
    color: var(--status-warning);
  }

  .policy-meta {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    width: 100%;
    margin-top: 2px;
  }

  .policy-source {
    font-size: var(--text-xs);
    color: var(--terminal-dim);
    background: var(--terminal-bg);
    padding: 1px var(--sp-1);
    border-radius: var(--radius);
  }

  .policy-author {
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .policy-raw-btn {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--terminal-dim);
    cursor: pointer;
    letter-spacing: 0.04em;
    margin-left: auto;
  }

  .policy-raw-btn:hover { color: var(--terminal-text); }

  .policy-link {
    color: var(--terminal-text);
    text-decoration: underline;
  }

  .policy-raw {
    width: 100%;
    font-size: var(--text-xs);
    color: var(--text-2);
    background: var(--terminal-bg);
    border: 1px solid var(--terminal-border);
    padding: var(--sp-2);
    border-radius: var(--radius);
    white-space: pre-wrap;
    word-break: break-all;
    margin-top: var(--sp-1);
  }

  @media (prefers-reduced-motion: reduce) {
    .turn { animation: none; }
    .cursor { animation: none; }
  }

  @media (max-width: 768px) {
    .terminal { font-size: var(--text-xs); }
    .tool-detail { margin-left: var(--sp-2); }
    .authz-detail { margin-left: var(--sp-2); }
  }
</style>
