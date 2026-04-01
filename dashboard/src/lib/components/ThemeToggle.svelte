<script lang="ts">
  let theme = $state<'light' | 'dark'>('dark');
  let rotated = $state(false);

  function init() {
    const stored = localStorage.getItem('openpaw-theme');
    if (stored === 'light' || stored === 'dark') {
      theme = stored;
    } else {
      theme = document.documentElement.getAttribute('data-theme') === 'light' ? 'light' : 'dark';
    }
    document.documentElement.setAttribute('data-theme', theme);
  }

  function toggle() {
    theme = theme === 'dark' ? 'light' : 'dark';
    rotated = !rotated;
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('openpaw-theme', theme);
  }

  $effect(() => {
    init();
  });
</script>

<button
  onclick={toggle}
  aria-label="Toggle theme"
  class="theme-toggle"
  title={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
>
  <span class="toggle-icon" class:toggle-icon--rotated={rotated}>
    {#if theme === 'dark'}
      <!-- Sun icon -->
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="5" />
        <line x1="12" y1="1" x2="12" y2="3" />
        <line x1="12" y1="21" x2="12" y2="23" />
        <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
        <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
        <line x1="1" y1="12" x2="3" y2="12" />
        <line x1="21" y1="12" x2="23" y2="12" />
        <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
        <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
      </svg>
    {:else}
      <!-- Moon icon -->
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
      </svg>
    {/if}
  </span>
</button>

<style>
  .theme-toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    transition: color var(--duration-fast) var(--ease),
                background var(--duration-fast) var(--ease);
  }

  .theme-toggle:hover {
    color: var(--text-primary);
    background: var(--brand-subtle);
  }

  .toggle-icon {
    display: flex;
    transition: transform var(--duration-base) var(--ease);
  }

  .toggle-icon--rotated {
    transform: rotate(180deg);
  }
</style>
