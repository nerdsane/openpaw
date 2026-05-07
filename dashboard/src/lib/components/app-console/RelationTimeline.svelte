<script lang="ts">
  let {
    links = []
  } = $props<{
    links: Array<{ from: string; to: string; via: string }>;
  }>();
</script>

<section class="timeline">
  <h2>State Map</h2>
  <div class="rail">
    {#each links as link}
      <div class="step">
        <span class="node">{link.from}</span>
        <span class="edge">{link.via}</span>
        <span class="node">{link.to}</span>
      </div>
    {/each}
  </div>
</section>

<style>
  .timeline {
    border-top: 1px solid var(--border);
    padding: var(--sp-4) 0;
  }

  h2 {
    font-size: var(--text-base);
    margin: 0 0 var(--sp-3) 0;
  }

  .rail {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
    gap: var(--sp-2);
  }

  .step {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: var(--sp-2);
    min-width: 0;
    border: 1px solid var(--border);
    padding: var(--sp-2);
  }

  .node,
  .edge {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .node {
    color: var(--text-1);
  }

  .edge {
    color: var(--text-3);
  }
</style>
