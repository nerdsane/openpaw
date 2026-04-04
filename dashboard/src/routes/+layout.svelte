<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import PawLogo from '$lib/components/PawLogo.svelte';
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';
  import { queryTeams } from '$lib/api';
  import type { Team } from '$lib/types';

  let { children } = $props();
  let projectsOpen = $state(true);
  let teams = $state<Team[]>([]);

  onMount(async () => {
    try {
      const data = await queryTeams();
      teams = data as unknown as Team[];
    } catch { /* no teams */ }
  });
</script>

<div class="shell">
  <aside class="sidebar">
    <div class="sidebar-header">
      <PawLogo size={20} />
      <span class="sidebar-title">OPEN PAW</span>
    </div>

    <nav class="sidebar-nav">
      <a href="/" class="nav-link">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/></svg>
        FLOOR
      </a>
      <a href="/agents" class="nav-link">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>
        AGENTS
      </a>
      <a href="/permissions" class="nav-link">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
        PERMISSIONS
      </a>

      <!-- PROJECTS folder tree -->
      <button class="nav-folder" onclick={() => projectsOpen = !projectsOpen}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          {#if projectsOpen}
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
            <line x1="9" y1="14" x2="15" y2="14"/>
          {:else}
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
            <line x1="12" y1="11" x2="12" y2="17"/>
            <line x1="9" y1="14" x2="15" y2="14"/>
          {/if}
        </svg>
        PROJECTS
      </button>

      {#if projectsOpen}
        {@const items = [{ id: '_platform', label: 'PLATFORM', href: '/platform', platform: true }, ...teams.map(t => ({ id: t.Id, label: (t.name || 'Unnamed').replace(/ Team$/i, '').toUpperCase(), href: `/project/${t.Id}`, platform: false }))]}
        <div class="nav-tree">
          {#each items as item, i (item.id)}
            <a href={item.href} class="nav-tree-item" class:nav-tree-item--platform={item.platform}>
              <span class="tree-branch">{i === items.length - 1 ? '└' : '├'}</span>
              {item.label}
            </a>
          {/each}
          {#if teams.length === 0}
            <span class="nav-tree-item nav-tree-empty">
              <span class="tree-branch">└</span>
              NO PROJECTS
            </span>
          {/if}
        </div>
      {/if}
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
    padding: var(--space-lg) var(--space-md);
    border-right: 1px solid var(--terminal-border);
    background: var(--black);
    position: fixed;
    top: 0;
    left: 0;
    bottom: 0;
    z-index: 10;
  }

  .sidebar-header {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    padding-bottom: var(--space-xl);
    color: var(--accent);
  }

  .sidebar-title {
    font-family: var(--font-mono);
    font-size: var(--label);
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--text-display);
  }

  .sidebar-nav {
    display: flex;
    flex-direction: column;
    gap: var(--space-2xs);
    flex: 1;
  }

  .nav-link {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    padding: var(--space-sm) var(--space-sm);
    border-radius: var(--radius-sm);
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.08em;
    color: var(--text-disabled);
    text-decoration: none;
    transition: color var(--duration-fast) var(--ease);
  }

  .nav-link:hover {
    color: var(--terminal-text);
    text-decoration: none;
  }

  /* Folder toggle button */
  .nav-folder {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    padding: var(--space-sm) var(--space-sm);
    border-radius: var(--radius-sm);
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.08em;
    color: var(--text-disabled);
    cursor: pointer;
    transition: color var(--duration-fast) var(--ease);
    width: 100%;
    text-align: left;
  }

  .nav-folder:hover {
    color: var(--terminal-text);
  }

  /* Tree children */
  .nav-tree {
    display: flex;
    flex-direction: column;
    gap: 0;
    margin-left: var(--space-lg);
  }

  .nav-tree-item {
    display: flex;
    align-items: center;
    gap: var(--space-xs);
    padding: 3px 0;
    font-family: var(--font-mono);
    font-size: var(--label);
    letter-spacing: 0.06em;
    color: var(--text-disabled);
    text-decoration: none;
    transition: color var(--duration-fast) var(--ease);
  }

  .nav-tree-item:hover {
    color: var(--terminal-text);
    text-decoration: none;
  }

  .tree-branch {
    color: var(--terminal-border);
    font-size: var(--caption);
    line-height: 1;
    flex-shrink: 0;
    user-select: none;
  }

  /* Platform styled differently — muted, system-level */
  .nav-tree-item--platform {
    color: var(--text-disabled);
    opacity: 0.6;
  }

  .nav-tree-item--platform:hover {
    color: var(--text-secondary);
    opacity: 1;
  }

  .nav-tree-empty {
    color: var(--text-disabled);
    opacity: 0.4;
  }

  .sidebar-footer {
    margin-top: auto;
    padding-top: var(--space-sm);
  }

  .main-content {
    flex: 1;
    margin-left: var(--sidebar-width);
    padding: var(--space-2xl) var(--space-xl);
  }
</style>
