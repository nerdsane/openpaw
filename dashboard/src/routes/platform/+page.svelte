<script lang="ts">
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import { queryEntities, fetchFileContent } from '$lib/api';
  import StatusBadge from '$lib/components/StatusBadge.svelte';

  let loaded = $state(false);
  let souls = $state<Record<string, unknown>[]>([]);
  let skills = $state<Record<string, unknown>[]>([]);
  let expandedSoul = $state<string | null>(null);
  let expandedSkill = $state<string | null>(null);
  let fileContentCache = $state<Record<string, string>>({});

  async function toggleContent(id: string, fileId: string, type: 'soul' | 'skill') {
    if (type === 'soul') {
      if (expandedSoul === id) { expandedSoul = null; return; }
      expandedSoul = id;
    } else {
      if (expandedSkill === id) { expandedSkill = null; return; }
      expandedSkill = id;
    }
    if (fileId && !fileContentCache[id]) {
      const content = await fetchFileContent(fileId);
      fileContentCache = { ...fileContentCache, [id]: content };
    }
  }

  function field(entity: Record<string, unknown>, key: string): string {
    return (entity[key] ?? entity[key.charAt(0).toUpperCase() + key.slice(1)] ?? '') as string;
  }

  onMount(async () => {
    const [so, sk] = await Promise.all([
      queryEntities('Souls').catch(() => []),
      queryEntities('Skills').catch(() => []),
    ]);
    souls = so;
    // Filter out ghost skills (no name — artifacts of failed Register actions)
    skills = sk.filter(s => {
      const name = (s.Name ?? s.name ?? '') as string;
      return name.length > 0;
    });
    loaded = true;
  });

  function humanScope(entity: Record<string, unknown>): string {
    const scope = ((entity.scope ?? entity.Scope ?? '') as string).trim();
    if (!scope || scope === 'global') return 'ALL AGENTS';
    if (scope === 'soul') return 'BROKEN (scope="soul")';
    return scope.toUpperCase();
  }
</script>

<div class="page">
  <header class="page-header">
    <span class="page-label">PLATFORM</span>
    <p class="page-desc">Base souls and skills bootstrapped with TemperPaw</p>
  </header>

  {#if !loaded}
    <div class="empty"><span class="empty-text">[LOADING...]</span></div>
  {:else}

    {#if souls.length > 0}
    <section class="section">
      <span class="section-label">SOULS ({souls.length})</span>
      <div class="card-grid">
        {#each souls as soul (field(soul, 'Id'))}
          {@const soulId = field(soul, 'Id')}
          {@const soulFileId = field(soul, 'ContentFileId') || field(soul, 'content_file_id')}
          <div class="card">
            <div class="card-header">
              <span class="card-dot"></span>
              <span class="card-name">{field(soul, 'Name') || field(soul, 'name') || 'Unnamed'}</span>
            </div>
            <p class="card-desc">{field(soul, 'Description') || field(soul, 'description') || '--'}</p>
            <StatusBadge status={field(soul, 'Status')} />
            {#if soulFileId}
              <button class="view-btn" onclick={() => toggleContent(soulId, soulFileId, 'soul')}>
                {expandedSoul === soulId ? '[-] HIDE' : '[+] VIEW SOUL'}
              </button>
            {/if}
            {#if expandedSoul === soulId && fileContentCache[soulId]}
              <pre class="file-content" transition:slide={{ duration: 150 }}>{fileContentCache[soulId]}</pre>
            {/if}
          </div>
        {/each}
      </div>
    </section>
    {/if}

    {#if skills.length > 0}
    <section class="section">
      <span class="section-label">SKILLS ({skills.length})</span>
      <div class="card-grid">
        {#each skills as skill (field(skill, 'Id'))}
          {@const skillId = field(skill, 'Id')}
          {@const skillFileId = field(skill, 'content_file_id') || field(skill, 'ContentFileId')}
          <div class="card">
            <div class="card-header">
              <span class="card-dot card-dot--accent"></span>
              <span class="card-name">{field(skill, 'Name') || field(skill, 'name') || 'Unnamed'}</span>
            </div>
            <p class="card-desc">{field(skill, 'Description') || field(skill, 'description') || '--'}</p>
            <div class="card-meta-row">
              <span class="meta-tag">{humanScope(skill)}</span>
            </div>
            {#if skillFileId}
              <button class="view-btn" onclick={() => toggleContent(skillId, skillFileId, 'skill')}>
                {expandedSkill === skillId ? '[-] HIDE' : '[+] VIEW'}
              </button>
            {/if}
            {#if expandedSkill === skillId && fileContentCache[skillId]}
              <pre class="file-content" transition:slide={{ duration: 150 }}>{fileContentCache[skillId]}</pre>
            {/if}
          </div>
        {/each}
      </div>
    </section>
    {/if}

    {#if souls.length === 0 && skills.length === 0}
      <div class="empty"><span class="empty-text">NO PLATFORM ENTITIES</span></div>
    {/if}

  {/if}
</div>

<style>
  .page { display: flex; flex-direction: column; gap: var(--sp-8); }

  .page-header { display: flex; flex-direction: column; gap: var(--sp-1); }
  .page-label {
    font-family: var(--font-mono); font-size: var(--text-xs);
    letter-spacing: 0.08em; color: var(--text-2);
  }
  .page-desc { font-size: var(--text-sm); color: var(--text-3); }

  .section { display: flex; flex-direction: column; gap: var(--sp-4); }
  .section-label {
    font-family: var(--font-mono); font-size: var(--text-xs);
    letter-spacing: 0.08em; color: var(--text-2);
  }

  .empty { display: flex; align-items: center; justify-content: center; padding: var(--sp-8) 0; }
  .empty-text {
    font-family: var(--font-mono); font-size: var(--text-xs);
    color: var(--text-3); letter-spacing: 0.04em;
  }

  .card-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: var(--sp-4); }
  .card {
    display: flex; flex-direction: column; gap: var(--sp-1);
    padding: var(--sp-4); background: var(--surface);
    border: 1px solid var(--border); border-radius: var(--radius);
  }
  .card-header { display: flex; align-items: center; gap: var(--sp-2); }
  .card-dot {
    width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0;
    background: var(--text-3);
  }
  .card-dot--accent { background: var(--accent); }
  .card-name { font-size: var(--text-sm); font-weight: 500; color: var(--text-1); }
  .card-desc { font-size: var(--text-xs); color: var(--text-2); line-height: 1.4; }
  .card-meta-row { display: flex; flex-wrap: wrap; gap: var(--sp-1); }
  .meta-tag {
    font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-2);
    letter-spacing: 0.04em;
    background: var(--surface-raised); padding: 1px var(--sp-2); border-radius: var(--radius);
  }
  .view-btn {
    font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-3);
    letter-spacing: 0.06em; padding: 2px 0;
    text-align: left; width: fit-content; cursor: pointer;
  }
  .view-btn:hover { color: var(--text-1); }
  .file-content {
    font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-2);
    background: var(--bg); border: 1px solid var(--border);
    padding: var(--sp-4); border-radius: var(--radius);
    white-space: pre-wrap; overflow-x: auto; max-height: 400px; overflow-y: auto;
  }
</style>
