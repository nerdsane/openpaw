<script lang="ts">
  import type { WorkCycle } from '$lib/types';

  let { workcycle }: { workcycle: WorkCycle } = $props();

  let isFailed = $derived(workcycle.Status === 'Failed');

  interface Gate {
    key: string;
    label: string;
    passed: boolean;
  }

  /**
   * Derive gates GENERICALLY from any boolean field ending in _ok, _passed,
   * or matching known gate patterns like has_plan. No hardcoded gate names.
   */
  let gates = $derived.by((): Gate[] => {
    const wc = workcycle as unknown as Record<string, unknown>;
    const result: Gate[] = [];
    for (const [key, val] of Object.entries(wc)) {
      if (key.startsWith('_') || key === 'Id' || key === 'Status') continue;
      // Match boolean gate fields: *_ok, *_passed, has_*
      if (typeof val === 'boolean' && (key.endsWith('_ok') || key.endsWith('_passed') || key.startsWith('has_'))) {
        // Derive a human-readable label from the key
        const label = key
          .replace(/_ok$/, '')
          .replace(/_passed$/, '')
          .replace(/^has_/, '')
          .replace(/_/g, ' ')
          .replace(/\b\w/g, (c) => c.toUpperCase());
        result.push({ key, label, passed: !!val });
      }
    }
    return result;
  });
</script>

<div class="pipeline">
  {#each gates as gate, i}
    <div class="gate">
      {#if gate.passed}
        <span class="gate-dot gate-dot--passed"></span>
      {:else if isFailed}
        <span class="gate-dot gate-dot--failed"></span>
      {:else}
        <span class="gate-ring"></span>
      {/if}
      <span class="gate-label">{gate.label}</span>
    </div>
    {#if i < gates.length - 1}
      <span
        class="gate-line"
        class:gate-line--passed={gate.passed}
      ></span>
    {/if}
  {/each}
</div>

<style>
  .pipeline {
    display: flex;
    align-items: center;
    gap: 0;
  }

  .gate {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }

  .gate-dot {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }

  .gate-dot--passed {
    background: #4ade80;
  }

  .gate-dot--failed {
    background: #f87171;
  }

  .gate-ring {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 1.5px solid #6b7280;
  }

  .gate-line {
    width: 32px;
    height: 1.5px;
    background: #6b7280;
    margin-bottom: 18px; /* align with the dot, not the label */
  }

  .gate-line--passed {
    background: #4ade80;
  }

  .gate-label {
    font-size: 0.625rem;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
</style>
