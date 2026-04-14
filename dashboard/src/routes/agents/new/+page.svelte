<script lang="ts">
  import { base } from '$app/paths';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { getSoulTemplates, createAgent } from '$lib/api';
  import type { SoulTemplate } from '$lib/api';

  let templates = $state<SoulTemplate[]>([]);
  let submitting = $state(false);
  let error = $state('');

  let name = $state('');
  let role = $state('');
  let soulTemplate = $state('');
  let model = $state('claude-sonnet-4-6');
  let maxTurns = $state('20');

  // Tools
  let toolRead = $state(true);
  let toolWrite = $state(true);
  let toolEdit = $state(true);
  let toolBash = $state(true);

  onMount(async () => {
    templates = await getSoulTemplates();
    if (templates.length > 0) {
      soulTemplate = templates[0].name;
    }
  });

  async function handleSubmit() {
    if (!name.trim()) {
      error = 'Name is required';
      return;
    }
    error = '';
    submitting = true;

    const tools: string[] = [];
    if (toolRead) tools.push('read');
    if (toolWrite) tools.push('write');
    if (toolEdit) tools.push('edit');
    if (toolBash) tools.push('bash');

    try {
      await createAgent({
        name: name.trim(),
        role: role.trim() || undefined,
        soul_template: soulTemplate || undefined,
        model,
        tools_enabled: tools.join(','),
        max_turns: maxTurns,
      });
      goto(`${base}/agents`);
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : 'Failed to create agent';
    } finally {
      submitting = false;
    }
  }
</script>

<div class="page">
  <h1 class="page-title">NEW AGENT</h1>

  <form class="agent-form" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
    {#if error}
      <div class="error-msg">{error}</div>
    {/if}

    <label>
      <span class="label-text">NAME *</span>
      <input type="text" bind:value={name} placeholder="my-assistant" required />
    </label>

    <label>
      <span class="label-text">ROLE</span>
      <input type="text" bind:value={role} placeholder="Software Developer" />
    </label>

    <label>
      <span class="label-text">SOUL TEMPLATE</span>
      <select bind:value={soulTemplate}>
        <option value="">None</option>
        {#each templates as t}
          <option value={t.name}>{t.name} — {t.description || 'No description'}</option>
        {/each}
      </select>
    </label>

    <label>
      <span class="label-text">MODEL</span>
      <select bind:value={model}>
        <option value="claude-sonnet-4-6">Claude Sonnet 4.6</option>
        <option value="claude-opus-4-6">Claude Opus 4.6</option>
        <option value="claude-haiku-4-5">Claude Haiku 4.5</option>
      </select>
    </label>

    <fieldset class="tools-fieldset">
      <legend class="label-text">TOOLS</legend>
      <div class="tools-grid">
        <label class="tool-check">
          <input type="checkbox" bind:checked={toolRead} /> read
        </label>
        <label class="tool-check">
          <input type="checkbox" bind:checked={toolWrite} /> write
        </label>
        <label class="tool-check">
          <input type="checkbox" bind:checked={toolEdit} /> edit
        </label>
        <label class="tool-check">
          <input type="checkbox" bind:checked={toolBash} /> bash
        </label>
      </div>
    </fieldset>

    <label>
      <span class="label-text">MAX TURNS</span>
      <input type="number" bind:value={maxTurns} min="1" max="500" />
    </label>

    <div class="form-actions">
      <a href="{base}/agents" class="btn btn-secondary">CANCEL</a>
      <button class="btn btn-primary" type="submit" disabled={submitting}>
        {submitting ? 'CREATING...' : 'CREATE AGENT'}
      </button>
    </div>
  </form>
</div>

<style>
  .page { max-width: 600px; }
  .page-title {
    font-family: var(--font-sans);
    font-size: var(--text-xl);
    color: var(--text-1);
    margin-bottom: var(--sp-8);
    letter-spacing: 0.1em;
  }

  .agent-form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }

  .agent-form label {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .label-text {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-2);
    letter-spacing: 0.06em;
  }

  .agent-form input,
  .agent-form select {
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: var(--sp-2) var(--sp-3);
    font-family: var(--font-mono);
    font-size: var(--text-base);
    color: var(--text-1);
  }

  .agent-form input::placeholder { color: var(--text-3); }
  .agent-form input:focus,
  .agent-form select:focus {
    outline: none;
    border-color: var(--accent);
  }

  .agent-form select {
    appearance: none;
    cursor: pointer;
  }

  .tools-fieldset {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--sp-2) var(--sp-3);
  }

  .tools-fieldset legend { padding: 0 var(--sp-1); }

  .tools-grid {
    display: flex;
    gap: var(--sp-6);
    margin-top: var(--sp-1);
  }

  .tool-check {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text-1);
    cursor: pointer;
  }

  .tool-check input[type="checkbox"] {
    accent-color: var(--accent);
  }

  .error-msg {
    background: var(--authz-denied-bg);
    border: 1px solid var(--authz-denied-border);
    border-radius: var(--radius);
    padding: var(--sp-2) var(--sp-3);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--status-error);
  }

  .form-actions {
    display: flex;
    gap: var(--sp-4);
    justify-content: flex-end;
    margin-top: var(--sp-4);
  }

  .btn {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    letter-spacing: 0.1em;
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--radius);
    cursor: pointer;
    border: 1px solid transparent;
    text-decoration: none;
    transition: all var(--duration) var(--ease);
  }

  .btn-primary {
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
  }
  .btn-primary:hover { opacity: 0.9; }
  .btn-primary:disabled { opacity: 0.4; cursor: not-allowed; }

  .btn-secondary {
    background: transparent;
    color: var(--text-2);
    border-color: var(--border-strong);
  }
  .btn-secondary:hover {
    color: var(--text-1);
    border-color: var(--text-2);
  }
</style>
