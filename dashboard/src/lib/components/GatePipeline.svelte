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
      { key: 'has_plan', label: 'Plan', passed: !!workcycle.has_plan },
      { key: 'tests_passed', label: 'Tests', passed: !!workcycle.tests_passed },
    ];
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
    background: var(--status-success);
  }

  .gate-dot--failed {
    background: var(--status-error);
  }

  .gate-ring {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 1.5px solid var(--status-idle);
  }

  .gate-line {
    width: 32px;
    height: 1.5px;
    background: var(--status-idle);
    margin-bottom: 18px; /* align with the dot, not the label */
  }

  .gate-line--passed {
    background: var(--status-success);
  }

  .gate-label {
    font-size: 0.625rem;
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
</style>
