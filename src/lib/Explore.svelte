<script module lang="ts">
  type Saved = { type: string; query: string; sort: string; mcVersion: string; loaders: string[]; categories: string[]; view: 'grid' | 'list' }
  let saved: Saved | null = null
  const cache = new Map<string, { at: number; res: SearchResult }>()
  const CACHE_MS = 5 * 60_000
</script>

<script lang="ts">
  import { thumb } from './img'
  import { onMount } from 'svelte'
  import { modrinth, type InstalledProject, type ModFile, type ProjectHit, type ProjectType, type SearchResult } from './api'
  import { locale, stageText, t } from './i18n.svelte'
  import { app, installProject, notify, openProject, updateInstalled } from './store.svelte'
  import Icon from './Icon.svelte'
  import ProjectCard from './ProjectCard.svelte'
  import Select from './Select.svelte'
  import { contentKindOf, formatBytes } from './meta'

  const types: { id: ProjectType; label: string }[] = [
    { id: 'modpack', label: 'Modpack' },
    { id: 'mod', label: 'Mod' },
    { id: 'resourcepack', label: 'Resource pack' },
    { id: 'shader', label: 'Shader' },
  ]
  const loaderChoices = ['fabric', 'quilt', 'forge', 'neoforge']

  const restore = !app.exploreIntent ? saved : null
  let first = !!restore
  let type: ProjectType = $state(app.exploreIntent?.kind ?? (restore?.type as ProjectType) ?? 'modpack')
  let targetId = $state(app.exploreIntent?.targetId ?? '')
  let query = $state(restore?.query ?? '')
  let sort = $state(restore?.sort ?? 'downloads')
  let mcVersion = $state(restore?.mcVersion ?? '')
  let loaders: string[] = $state(restore?.loaders ?? [])
  let categories: string[] = $state(restore?.categories ?? [])
  let view: 'grid' | 'list' = $state(restore?.view ?? 'grid')
  let showFilters = $state(false)
  let hits: ProjectHit[] = $state([])
  let total = $state(0)
  let loading = $state(false)
  let catalog: Partial<Record<ProjectType, string[]>> = $state({})
  let installed: ModFile[] = $state([])
  let installedMap: Map<string, InstalledProject> = $state(new Map())

  onMount(() => (app.exploreIntent = null))

  const releases = $derived(app.versions.filter((v) => v.kind === 'release').slice(0, 80))
  const targets = $derived(type === 'mod' ? app.instances.filter((i) => i.loader !== 'vanilla') : app.instances)
  const target = $derived(targets.find((i) => i.id === targetId) ?? targets[0] ?? null)
  const cats = $derived(catalog[type] ?? [])
  const cantInstall = $derived(type !== 'modpack' && !target)

  $effect(() => {
    if (target && targetId !== target.id) targetId = target.id
  })

  $effect(() => {
    const ty = type
    const inst = target
    if (first) {
      first = false
      return
    }
    categories = []
    if (ty === 'modpack' || !inst) {
      mcVersion = ty === 'modpack' ? '' : mcVersion
      loaders = []
      return
    }
    mcVersion = inst.mcVersion
    loaders = ty === 'mod' ? (inst.loader === 'quilt' ? ['quilt', 'fabric'] : [inst.loader]) : []
  })

  $effect(() => {
    const ty = type
    if (!catalog[ty]) modrinth.categories(ty).then((c) => (catalog[ty] = c)).catch(() => {})
  })

  let requestSeq = 0
  let timer: ReturnType<typeof setTimeout> | undefined

  async function load(reset: boolean) {
    const seq = ++requestSeq
    const args = {
      query,
      projectType: type,
      mcVersion: mcVersion || undefined,
      loaders: [...loaders],
      categories: [...categories],
      sort,
      offset: reset ? 0 : hits.length,
    }
    const key = JSON.stringify(args)
    const hit = cache.get(key)
    if (hit && Date.now() - hit.at < CACHE_MS) {
      hits = reset ? hit.res.hits : [...hits, ...hit.res.hits]
      total = hit.res.total
      loading = false
      return
    }
    loading = true
    try {
      const res = await modrinth.search(args)
      cache.set(key, { at: Date.now(), res })
      if (cache.size > 40) cache.delete(cache.keys().next().value!)
      if (seq !== requestSeq) return
      hits = reset ? res.hits : [...hits, ...res.hits]
      total = res.total
    } catch (e) {
      if (seq === requestSeq) notify(e)
    } finally {
      if (seq === requestSeq) loading = false
    }
  }

  $effect(() => {
    saved = { type, query, sort, mcVersion, loaders: [...loaders], categories: [...categories], view }
  })

  $effect(() => {
    void [query, sort, type, mcVersion, loaders.join(), categories.join()]
    clearTimeout(timer)
    timer = setTimeout(() => load(true), 350)
    return () => clearTimeout(timer)
  })

  let refreshSeq = 0
  async function refreshInstalled() {
    const seq = ++refreshSeq
    const inst = target
    if (type === 'modpack' || !inst) {
      installed = []
      installedMap = new Map()
      return
    }
    const kind = contentKindOf(type)
    const files = modrinth.listContent(inst.id, kind).catch(() => [])
    const ids = modrinth.installedProjects(inst.id, kind, true).catch((): InstalledProject[] => [])
    const list = await files
    if (seq !== refreshSeq) return
    installed = list
    const found = await ids
    if (seq === refreshSeq) installedMap = new Map(found.map((p) => [p.projectId, p]))
  }
  $effect(() => {
    void [type, target?.id, app.contentRev]
    refreshInstalled()
  })

  function act(h: ProjectHit) {
    const entry = installedMap.get(h.projectId)
    if (entry?.update && target) void updateInstalled(target.id, type, entry.fileName, h.projectId)
    else void installProject(h, type)
  }
  const label = (h: ProjectHit) => {
    const entry = installedMap.get(h.projectId)
    return app.installingId === h.projectId ? t('Installing…') : entry?.update ? t('Update') : entry ? t('Installed') : t('Install')
  }

  const remove = async (f: ModFile) => {
    if (!target) return
    try {
      await modrinth.removeContent(target.id, contentKindOf(type), f.fileName)
      await refreshInstalled()
    } catch (e) {
      notify(e)
    }
  }

  const toggle = (list: string[], v: string) => (list.includes(v) ? list.filter((x) => x !== v) : [...list, v])
  const fmt = (n: number) => new Intl.NumberFormat(locale(), { notation: 'compact' }).format(n)
  const progress = $derived(app.install && app.installingId ? app.install : null)
  const percent = $derived(progress && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : null)

  function reset() {
    query = ''
    categories = []
    if (type === 'modpack') {
      mcVersion = ''
      loaders = []
    }
  }
</script>

<header class="head">
  <h1>{t('Explore')}</h1>
  <p class="muted">{t('Modpacks, mods, resource packs and shaders from Modrinth.')}</p>
</header>

<div class="layout">
  <section class="results">
    <div class="toolbar">
      <label class="search grow">
        <span class="sr">{t('Search Modrinth')}</span>
        <span class="field"><Icon name="search" size={16} /><input type="search" bind:value={query} placeholder={t('Search {type}…', { type: (types.find((x) => x.id === type)?.label ?? '').toLowerCase() })} /></span>
      </label>
      <Select
        bind:value={sort}
        width="auto"
        label={t('Sort by')}
        options={[
          { value: 'downloads', label: t('Most downloaded') },
          { value: 'relevance', label: t('Relevance') },
          { value: 'newest', label: t('Newest') },
          { value: 'updated', label: t('Recently updated') },
        ]}
      />
      <div class="seg" role="group" aria-label={t('View')}>
        <button class="iconbtn" aria-pressed={view === 'grid'} aria-label={t('Grid')} onclick={() => (view = 'grid')}><Icon name="layout" size={16} /></button>
        <button class="iconbtn" aria-pressed={view === 'list'} aria-label={t('List')} onclick={() => (view = 'list')}><Icon name="list" size={16} /></button>
      </div>
      <button class="fbtn" onclick={() => (showFilters = !showFilters)} aria-expanded={showFilters}><Icon name="filter" size={16} /> {t('Filters')}</button>
    </div>

    {#if progress}
      <div class="progress">
        <div class="row small"><span class="grow">{stageText(progress.stage)}</span><span class="muted">{percent !== null ? `${percent}%` : ''}</span></div>
        <div class="line" class:indeterminate={percent === null}><div style:width={percent !== null ? `${percent}%` : undefined}></div></div>
      </div>
    {/if}


    {#if cantInstall}
      <p class="muted note">{type === 'mod' ? t('To install mods you need a Fabric or Quilt instance: create one from the Library.') : t('To install this you need an instance: create one from the Library.')}</p>
    {/if}

    <p class="muted small count" aria-live="polite">{loading ? t('Searching…') : t('{n} results', { n: new Intl.NumberFormat(locale()).format(total) })}</p>

    {#if view === 'grid'}
      <div class="cards" aria-busy={loading}>
        {#each hits as h, n (h.projectId)}
          <ProjectCard index={n % 20} hit={h} busy={app.installingId === h.projectId} disabled={app.installingId !== null} installed={installedMap.has(h.projectId)} update={!!installedMap.get(h.projectId)?.update} onInstall={() => act(h)} onOpen={() => openProject(h, type)} />
        {/each}
        {#if loading && !hits.length}
          {#each Array(8) as _, n}<div class="sk" style:--d="{n * 40}ms"></div>{/each}
        {/if}
      </div>
    {:else}
      <div class="list" aria-busy={loading}>
        {#each hits as h, n (h.projectId)}
          <div class="item row rise" style:--d="{(n % 20) * 30}ms">
            {#if h.iconUrl}<img use:thumb={{ src: h.iconUrl, w: 96 }} alt="" width="48" height="48" decoding="async" />{:else}<div class="ph"></div>{/if}
            <div class="grow">
              <div>{h.title} <span class="muted small">· {h.author}</span></div>
              <p class="muted desc">{h.description}</p>
              <div class="muted small">{t('{n} downloads', { n: fmt(h.downloads) })}{h.latestMc ? ` · ${t('up to {version}', { version: h.latestMc })}` : ''}</div>
            </div>
            <button onclick={() => openProject(h, type)}>{t('Details')}</button>
            <button class:primary={!installedMap.has(h.projectId) || installedMap.get(h.projectId)?.update} onclick={() => act(h)} disabled={app.installingId !== null}>{label(h)}</button>
          </div>
        {/each}
      </div>
    {/if}

    {#if !loading && !hits.length}<p class="muted">{t('No results with these filters.')}</p>{/if}

    {#if hits.length < total}
      <button class="more" onclick={() => load(false)} disabled={loading}>{loading ? t('Loading…') : t('Show more ({n})', { n: total - hits.length })}</button>
    {/if}

    {#if type !== 'modpack' && target}
      <h2>{t('In "{name}"', { name: target.name })} · {installed.length}</h2>
      <div class="list">
        {#each installed as f (f.fileName)}
          <div class="item row">
            <span class="grow nm">{f.fileName.replace(/\.disabled$/, '')}</span>
            <span class="muted small">{formatBytes(f.size)}</span>
            <button class="ghost" onclick={() => remove(f)}>{t('Remove')}</button>
          </div>
        {:else}
          <p class="muted">{t('Nothing installed.')}</p>
        {/each}
      </div>
    {/if}
  </section>

  <aside class="filters" class:open={showFilters} aria-label={t('Filters')}>
    <div class="fhead">
      <span class="ft"><Icon name="filter" size={15} /> {t('Filters')}</span>
      <button class="ghost small" onclick={reset}>{t('Reset')}</button>
    </div>

    <div class="fsec">
      <h3>{t('Type')}</h3>
      <div class="chips">
        {#each types as ty}
          <button class="chip" aria-pressed={type === ty.id} onclick={() => (type = ty.id)}>{ty.label}</button>
        {/each}
      </div>
    </div>

    {#if type !== 'modpack'}
      <div class="fsec">
        <h3>{t('Install into')}</h3>
        <Select bind:value={targetId} disabled={!targets.length} label={t('Target instance')} options={targets.map((i) => ({ value: i.id, label: `${i.name} · ${i.mcVersion}` }))} />
      </div>
    {/if}

    <div class="fsec">
      <h3>{t('Game version')}</h3>
      <Select bind:value={mcVersion} label={t('Game version')} options={[{ value: '', label: t('Any') }, ...releases.map((v) => ({ value: v.id, label: v.id }))]} />
    </div>

    {#if type === 'modpack' || type === 'mod'}
      <div class="fsec">
        <h3>{t('Loader')}</h3>
        <div class="chips">
          {#each loaderChoices as l}
            <button class="chip" aria-pressed={loaders.includes(l)} onclick={() => (loaders = toggle(loaders, l))}>{l[0].toUpperCase() + l.slice(1)}</button>
          {/each}
        </div>
      </div>
    {/if}

    {#if cats.length}
      <div class="fsec">
        <h3>{t('Categories')}</h3>
        <div class="chips">
          {#each cats as c}
            <button class="chip" aria-pressed={categories.includes(c)} onclick={() => (categories = toggle(categories, c))}>{c.replace(/-/g, ' ')}</button>
          {/each}
        </div>
      </div>
    {/if}
  </aside>
</div>

<style>
  .head { display: grid; gap: 6px; }
  .layout { display: grid; grid-template-columns: minmax(0, 1fr) 270px; gap: 28px; align-items: start; }
  .results { display: grid; gap: 14px; min-width: 0; }
  .toolbar { display: flex; flex-wrap: wrap; gap: 12px; align-items: center; }
  .search { min-width: 200px; }
  .field { position: relative; display: block; }
  .field :global(svg) { position: absolute; left: 11px; top: 50%; transform: translateY(-50%); color: var(--faint); pointer-events: none; }
  .field input { padding-left: 34px; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
  .fbtn { display: none; align-items: center; gap: 6px; }
  .count, .note { margin: 0; }
  .progress { display: grid; gap: 8px; max-width: 560px; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 16px; }
  .sk { position: relative; overflow: hidden; aspect-ratio: 4 / 5; border-radius: 24px; background: var(--surface); border: 1px solid var(--line); animation: skin 400ms ease both var(--d); }
  .sk::after { content: ''; position: absolute; inset: 0; background: linear-gradient(100deg, transparent 30%, rgba(128, 128, 128, 0.14) 50%, transparent 70%); animation: shimmer 1.5s infinite; }
  .rise { animation: rowrise 450ms var(--spring) backwards var(--d); }
  @keyframes rowrise { from { opacity: 0; transform: translateY(10px); } }
  @keyframes skin { from { opacity: 0; } }
  .list > .item { content-visibility: auto; contain-intrinsic-size: auto 96px; }
  .item img, .ph { width: 48px; height: 48px; border-radius: 10px; flex: none; background: var(--surface-2); object-fit: cover; }
  .desc { margin: 2px 0 6px; font-size: 13px; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .more { justify-self: center; }

  .filters { position: sticky; top: 0; display: grid; gap: 20px; padding: 18px; border: 1px solid var(--line); border-radius: 16px; background: var(--surface); max-height: calc(100vh - 170px); overflow: auto; }
  .fhead { display: flex; align-items: center; justify-content: space-between; }
  .ft { display: inline-flex; align-items: center; gap: 8px; font-weight: 500; }
  .fsec { display: grid; gap: 10px; }
  .fsec h3 { margin: 0; font-size: 13px; font-weight: 500; color: var(--muted); }
  .chip { text-transform: none; }

  @media (max-width: 1050px) {
    .layout { grid-template-columns: minmax(0, 1fr); }
    .fbtn { display: inline-flex; }
    .filters { display: none; position: static; max-height: none; }
    .filters.open { display: grid; order: -1; }
  }
</style>
