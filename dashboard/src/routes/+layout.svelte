<script lang="ts">
  import '../app.css';
  import PawLogo from '$lib/components/PawLogo.svelte';
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';
  let { children } = $props();
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
      <a href="/project" class="nav-link">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
        Project
      </a>
    </nav>

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
    padding: var(--space-2) var(--space-2);
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
    padding-bottom: var(--space-2);
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
    border-left: 2px solid transparent;
    transition: border-color var(--duration-fast) var(--ease),
                color var(--duration-fast) var(--ease),
                background var(--duration-fast) var(--ease);
  }

  .nav-link:hover {
    color: var(--text-primary);
    background: var(--brand-subtle);
    border-left-color: var(--text-tertiary);
    text-decoration: none;
  }

  .sidebar-footer {
    margin-top: auto;
    padding-top: var(--space-1);
  }

  .main-content {
    flex: 1;
    margin-left: var(--sidebar-width);
    max-width: var(--content-max);
    padding: var(--space-4) var(--space-3);
  }
</style>
