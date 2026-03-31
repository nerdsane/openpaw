<script lang="ts">
  import '../app.css';
  import PawLogo from '$lib/components/PawLogo.svelte';
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';
  import { activeAgents } from '$lib/stores/agents';

  let { children } = $props();
  let agentCount = $derived($activeAgents.length);
</script>

<div class="shell">
  <aside class="sidebar">
    <div class="sidebar-header">
      <PawLogo size={20} />
      <span class="sidebar-title">Open Paw</span>
    </div>

    <nav class="sidebar-nav">
      <a href="/" class="nav-link">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/></svg>
        Floor
      </a>
      <a href="/activity" class="nav-link">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
        Activity
      </a>
      <a href="/pipelines" class="nav-link">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/></svg>
        Pipelines
      </a>
      <a href="/permissions" class="nav-link">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
        Permissions
      </a>
    </nav>

    <div class="sidebar-roster">
      <div class="roster-label">Agents</div>
      <div class="roster-count">
        {#if agentCount > 0}
          <span class="status-dot" data-status="active"></span>
          <span>{agentCount} active</span>
        {:else}
          <span class="status-dot" data-status="idle"></span>
          <span>idle</span>
        {/if}
      </div>
    </div>

    <div class="sidebar-footer">
      <ThemeToggle />
    </div>
  </aside>

  <main class="main-content">
    {@render children()}
  </main>
</div>

<style>
  .shell {
    display: flex;
    min-height: 100vh;
  }

  .sidebar {
    width: var(--sidebar-width);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    padding: var(--space-3) var(--space-2);
    border-right: 1px solid var(--border);
    background: var(--surface-base);
    position: fixed;
    top: 0;
    left: 0;
    bottom: 0;
    z-index: 10;
  }

  .sidebar-header {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding-bottom: var(--space-3);
    color: var(--text-primary);
  }

  .sidebar-title {
    font-family: var(--font-serif);
    font-size: var(--text-base);
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .sidebar-nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
  }

  .nav-link {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: 6px var(--space-1);
    border-radius: var(--radius-sm);
    font-size: var(--text-sm);
    color: var(--text-secondary);
    text-decoration: none;
    transition: color var(--duration-fast) var(--ease),
                background var(--duration-fast) var(--ease);
  }

  .nav-link:hover {
    color: var(--text-primary);
    background: var(--brand-subtle);
    text-decoration: none;
  }

  .sidebar-roster {
    margin-top: auto;
    padding: var(--space-2) 0;
    border-top: 1px solid var(--border);
  }

  .roster-label {
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 4px;
  }

  .roster-count {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }

  .sidebar-footer {
    padding-top: var(--space-1);
  }

  .main-content {
    flex: 1;
    margin-left: var(--sidebar-width);
    max-width: var(--content-max);
    padding: var(--space-6) var(--space-4);
  }
</style>
