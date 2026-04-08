<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import PawLogo from '$lib/components/PawLogo.svelte';
  import ThemeToggle from '$lib/components/ThemeToggle.svelte';
  import { queryEntities } from '$lib/api';
  import type { Project } from '$lib/types';
  import { page } from '$app/stores';

  let { children } = $props();
  let collapsed = $state(false);
  let projectsOpen = $state(true);
  let projects = $state<Project[]>([]);

  let isCanvas = $derived($page.url.pathname === '/');

  onMount(async () => {
    try {
      const data = await queryEntities('Projects');
      projects = data as unknown as Project[];
    } catch {}
  });
</script>

<div class="shell" class:sidebar-collapsed={collapsed}>
  <aside class="sidebar" class:collapsed>
    <div class="sidebar-top">
      <button class="sidebar-toggle" onclick={() => collapsed = !collapsed} title={collapsed ? 'Expand' : 'Collapse'}>
        {#if collapsed}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M9 18l6-6-6-6"/></svg>
        {:else}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M15 18l-6-6 6-6"/></svg>
        {/if}
      </button>
      {#if !collapsed}
        <span class="sidebar-title">OPEN PAW</span>
      {/if}
    </div>

    <nav class="sidebar-nav">
      <a href="/" class="nav-item" class:active={$page.url.pathname === '/'}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/></svg>
        {#if !collapsed}<span>Canvas</span>{/if}
      </a>
      <a href="/apps" class="nav-item" class:active={$page.url.pathname === '/apps'}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/></svg>
        {#if !collapsed}<span>Apps</span>{/if}
      </a>
      <a href="/sessions" class="nav-item" class:active={$page.url.pathname.startsWith('/sessions')}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>
        {#if !collapsed}<span>Sessions</span>{/if}
      </a>

      {#if !collapsed}
        <div class="nav-divider"></div>

        <button class="nav-item nav-folder" onclick={() => projectsOpen = !projectsOpen}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
          </svg>
          <span>Projects</span>
          <span class="folder-chevron">{projectsOpen ? '−' : '+'}</span>
        </button>

        {#if projectsOpen}
          <div class="tree">
            <a href="/?focus=platform" class="tree-item tree-item--dim">Platform</a>
            {#each projects as project (project.Id)}
              <a href="/?focus=project&id={project.Id}" class="tree-item">
                {project.name || 'Unnamed'}
              </a>
            {/each}
            {#if projects.length === 0}
              <span class="tree-item tree-item--empty">No projects</span>
            {/if}
          </div>
        {/if}
      {/if}
    </nav>

    <div class="sidebar-footer">
      <ThemeToggle />
    </div>
  </aside>

  <main class="main" class:main--canvas={isCanvas}>
    {@render children()}
  </main>
</div>

<style>
  .shell {
    display: flex;
    min-height: 100vh;
  }

  /* ---- Sidebar ---- */
  .sidebar {
    width: var(--sidebar-w);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    padding: var(--sp-3) var(--sp-2);
    border-right: 1px solid var(--border);
    background: var(--bg);
    position: fixed;
    top: 0; left: 0; bottom: 0;
    z-index: 20;
    transition: width var(--duration) var(--ease);
    overflow: hidden;
  }

  .sidebar.collapsed {
    width: 44px;
    padding: var(--sp-3) var(--sp-1);
  }

  .sidebar-top {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding-bottom: var(--sp-4);
    min-height: 20px;
  }

  .sidebar-toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px; height: 24px;
    color: var(--text-3);
    flex-shrink: 0;
    border-radius: var(--radius);
  }

  .sidebar-toggle:hover { color: var(--text-1); }

  .sidebar-title {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--text-2);
    white-space: nowrap;
  }

  /* ---- Nav items ---- */
  .sidebar-nav {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: 6px var(--sp-2);
    border-radius: var(--radius);
    font-size: var(--text-sm);
    color: var(--text-3);
    text-decoration: none;
    transition: color var(--duration) var(--ease), background var(--duration) var(--ease);
    white-space: nowrap;
  }

  .nav-item:hover { color: var(--text-1); background: var(--accent-subtle); text-decoration: none; }
  .nav-item.active { color: var(--text-1); }
  .nav-item svg { flex-shrink: 0; }

  .nav-folder {
    width: 100%;
    text-align: left;
    cursor: pointer;
  }

  .folder-chevron {
    margin-left: auto;
    font-size: var(--text-xs);
    color: var(--text-3);
  }

  .nav-divider {
    height: 1px;
    background: var(--border);
    margin: var(--sp-2) 0;
  }

  /* ---- Tree ---- */
  .tree {
    display: flex;
    flex-direction: column;
    padding-left: var(--sp-6);
    border-left: 1px solid var(--border);
    margin-left: 13px;
  }

  .tree-item {
    font-size: var(--text-sm);
    color: var(--text-3);
    padding: 2px var(--sp-2);
    text-decoration: none;
    transition: color var(--duration) var(--ease);
  }

  .tree-item:hover { color: var(--text-1); text-decoration: none; }
  .tree-item--dim { opacity: 0.5; }
  .tree-item--dim:hover { opacity: 1; }
  .tree-item--empty { opacity: 0.3; font-style: italic; }

  /* ---- Footer ---- */
  .sidebar-footer {
    margin-top: auto;
    padding-top: var(--sp-2);
  }

  /* ---- Main content ---- */
  .main {
    flex: 1;
    margin-left: var(--sidebar-w);
    padding: var(--sp-6) var(--sp-8);
    transition: margin-left var(--duration) var(--ease);
  }

  .sidebar-collapsed .main {
    margin-left: 44px;
  }

  .main--canvas {
    padding: 0;
  }

  /* ---- Responsive ---- */
  @media (max-width: 768px) {
    .sidebar {
      width: 44px;
      padding: var(--sp-3) var(--sp-1);
    }

    .sidebar-title, .nav-item span, .nav-divider,
    .nav-folder span, .folder-chevron, .tree { display: none; }

    .main {
      margin-left: 44px;
      padding: var(--sp-4) var(--sp-4);
    }

    .main--canvas { padding: 0; }
  }
</style>
