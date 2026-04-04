<script lang="ts">
  import type { WorkCycle } from '$lib/types';

  let { workcycle }: { workcycle: WorkCycle } = $props();

  let isFailed = $derived(workcycle.Status === 'Failed');

  interface Gate {
    key: string;
    label: string;
    passed: boolean;
  }

  let gates = $derived.by((): Gate[] => {
    return [
      { key: 'has_plan', label: 'PLAN', passed: !!workcycle.has_plan },
      { key: 'tests_passed', label: 'TESTS', passed: !!workcycle.tests_passed },
    ];
  });
</script>

<div class="pipeline">
  {#each gates as gate, i}
    <div class="gate">
      {#if gate.passed}
        <span class="gate-block gate-block--passed"></span>
      {:else if isFailed}
        <span class="gate-block gate-block--failed"></span>
      {:else}
        <span class="gate-block gate-block--empty"></span>
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
    gap: var(--space-xs);
  }

  .gate-block {
    display: inline-block;
    width: 24px;
    height: 6px;
    border-radius: 0;
  }

  .gate-block--passed {
    background: var(--status-success);
  }

  .gate-block--failed {
    background: var(--status-error);
  }

  .gate-block--empty {
    background: var(--border);
  }

  .gate-line {
    width: 24px;
    height: 1px;
    background: var(--border);
    margin-bottom: 18px;
  }

  .gate-line--passed {
    background: var(--status-success);
  }

  .gate-label {
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.08em;
    color: var(--text-disabled);
  }
</style>
