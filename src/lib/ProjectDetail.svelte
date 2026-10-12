<script lang="ts">
  import { thumb } from './img'
  import { onMount } from 'svelte'
  import { api, modrinth, type PackDetails, type PackFile, type PackVersion, type ProjectHit, type ProjectPage } from './api'
  import Icon from './Icon.svelte'
  import LoaderCover from './LoaderCover.svelte'
  import { locale, stageText, t } from './i18n.svelte'
  import Select from './Select.svelte'
  import { renderMarkdown } from './markdown'
  import { formatBytes, relativeTime } from './meta'
  import { portal } from './actions'
  import { app, notify, photoFor } from './store.svelte'

  let { hit, busy = false, disabled = false, onInstall, onClose }: {
    hit: ProjectHit
    busy?: boolean
    disabled?: boolean
    onInstall: (versionId?: string) => void
    onClose: () => void
  } = $props()

  type Tab = 'contents' | 'about' | 'versions' | 'gallery'
  let tab: Tab = $state('contents')
  let details = $state<PackDetails | null>(null)
  let failure = $state<string | null>(null)
  let loading = $state(true)
  let page = $state<ProjectPage | null>(null)
  let versionId = $state<string | undefined>(undefined)
  let kind: PackFile['kind'] = $state('mod')
  let query = $state('')
  let closing = $state(false)
  let closeBtn: HTMLButtonElement | undefined = $state()
  let token = 0

  const fmt = (n: number) => new Intl.NumberFormat(locale(), { notation: 'compact' }).format(n)
  const cover = $derived(page?.gallery[0]?.url ?? hit.bannerUrl ?? photoFor(hit.latestMc ?? undefined, hit.title))
  const title = (f: PackFile) => f.project?.title ?? f.fileName.replace(/\.(jar|zip)$/, '')
  const latest = $derived(page?.versions.find((v) => v.kind === 'release') ?? page?.versions[0])
  const chosen = $derived(page?.versions.find((v) => v.id === versionId) ?? latest)
  const isLatest = $derived(!chosen || chosen.id === latest?.id)
  let vquery = $state('')
  let vmc = $state('')
  let vloader = $state('')
  let vkind = $state('')
  const cmpVer = (a: string, b: string) => {
    const x = a.split(/[.\-]/).map((s) => parseInt(s) || 0)
    const y = b.split(/[.\-]/).map((s) => parseInt(s) || 0)
    for (let i = 0; i < Math.max(x.length, y.length); i++) if ((y[i] ?? 0) !== (x[i] ?? 0)) return (y[i] ?? 0) - (x[i] ?? 0)
    return 0
  }
  const mcOptions = $derived([...new Set(page?.versions.flatMap((v) => v.mcVersions) ?? [])].sort(cmpVer))
  const loaderOptions = $derived([...new Set(page?.versions.flatMap((v) => v.loaders) ?? [])].sort())
  const kindOptions = $derived([...new Set(page?.versions.map((v) => v.kind) ?? [])])
  const shownVersions = $derived.by(() => {
    const q = vquery.trim().toLowerCase()
    return (page?.versions ?? []).filter(
      (v) =>
        (!q || `${v.number} ${v.name} ${v.mcVersions.join(' ')} ${v.loaders.join(' ')}`.toLowerCase().includes(q)) &&
        (!vmc || v.mcVersions.includes(vmc)) &&
        (!vloader || v.loaders.includes(vloader)) &&
        (!vkind || v.kind === vkind),
    )
  })
  const filtering = $derived(!!(vquery.trim() || vmc || vloader || vkind))
  const clearFilters = () => {
    vquery = ''
    vmc = ''
    vloader = ''
    vkind = ''
  }
  const body = $derived(page ? renderMarkdown(page.body) : '')

  async function load() {
    const my = ++token
    loading = true
    failure = null
    try {
      const d = await modrinth.modpackDetails(hit.projectId, versionId)
      if (my !== token) return
      details = d
      if (!d.files.some((f) => f.kind === kind)) kind = (['mod', 'resourcepack', 'shader', 'other'] as const).find((k) => d.files.some((f) => f.kind === k)) ?? 'mod'
    } catch (e) {
      if (my === token) failure = e instanceof Error ? e.message : String(e)
    } finally {
      if (my === token) loading = false
    }
  }

  function pick(v: PackVersion) {
    versionId = v.id
    tab = 'contents'
    load()
  }

  function links(node: HTMLElement) {
    const onClick = (e: MouseEvent) => {
      const a = (e.target as HTMLElement).closest('a[data-ext]') as HTMLAnchorElement | null
      if (!a) return
      e.preventDefault()
      api.openUrl(a.href).catch(notify)
    }
    node.addEventListener('click', onClick)
    return { destroy: () => node.removeEventListener('click', onClick) }
  }

  onMount(() => {
    load()
    modrinth.projectPage(hit.projectId).then((p) => (page = p)).catch(() => {})
    closeBtn?.focus()
    return () => token++
  })

  const labels: Record<PackFile['kind'], () => string> = {
    mod: () => t('Mods'),
    resourcepack: () => t('Resource packs'),
    shader: () => t('Shaders'),
    other: () => t('Other'),
  }
  const counts = $derived.by(() => {
    const c = { mod: 0, resourcepack: 0, shader: 0, other: 0 }
    for (const f of details?.files ?? []) c[f.kind]++
    return c
  })
  const kinds = $derived((['mod', 'resourcepack', 'shader', 'other'] as const).filter((k) => counts[k] > 0))
  const rows = $derived.by(() => {
    const q = query.trim().toLowerCase()
    return (details?.files ?? []).filter(
      (f) => f.kind === kind && (!q || `${title(f)} ${f.fileName} ${f.project?.description ?? ''}`.toLowerCase().includes(q)),
    )
  })
  const progress = $derived(busy ? app.install : null)
  const percent = $derived(progress && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : null)
  const loaderName = (l: string) => l[0].toUpperCase() + l.slice(1)

  function close() {
    if (closing) return
    closing = true
    setTimeout(onClose, 180)
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') close()
  }
</script>

<svelte:window onkeydown={key} />

<div use:portal class:closing>
<button class="backdrop" onclick={close} aria-label={t('Close')} tabindex="-1"></button>
<div class="modal" role="dialog" aria-modal="true" aria-label={hit.title}>
  <header class="hero">
    {#if cover}<img class="photo" use:thumb={{ src: cover, w: 1280 }} alt="" decoding="async" />{:else}<LoaderCover loader="vanilla" />{/if}
    <div class="veil"></div>
    <button class="x" bind:this={closeBtn} onclick={close} aria-label={t('Close')} title={t('Close')}><Icon name="x" size={17} /></button>
    <div class="id">
      {#if hit.iconUrl}<img class="icon" use:thumb={{ src: hit.iconUrl, w: 128 }} alt="" width="64" height="64" />{/if}
      <div class="tt">
        <h2>{hit.title}</h2>
        <div class="by">{hit.author} · {t('{n} downloads', { n: fmt(hit.downloads) })}</div>
      </div>
      <button class="primary go" onclick={() => onInstall(versionId)} {disabled}>{busy ? t('Installing…') : isLatest ? t('Install') : t('Install {v}', { v: chosen?.number ?? '' })}</button>
    </div>
  </header>

  {#if progress}
    <div class="progress">
      <div class="row small"><span class="grow">{stageText(progress.stage)}</span><span class="muted">{percent !== null ? `${percent}%` : ''}</span></div>
      <div class="line" class:indeterminate={percent === null}><div style:width={percent !== null ? `${percent}%` : undefined}></div></div>
    </div>
  {/if}

  <div class="meta">
    <p class="desc">{hit.description}</p>
    {#if details}
      <div class="chips">
        <span class="tag">{t('Version {v}', { v: details.version })}</span>
        {#if details.mcVersion}<span class="tag">Minecraft {details.mcVersion}</span>{/if}
        {#if details.loader}<span class="tag">{loaderName(details.loader)}</span>{/if}
        <span class="tag">{t('{n} files', { n: details.files.length })}</span>
        {#each hit.categories.filter((c) => !['fabric', 'quilt', 'forge', 'neoforge'].includes(c)).slice(0, 4) as c}<span class="tag soft">{c.replace(/-/g, ' ')}</span>{/each}
      </div>
    {/if}
  </div>

  <div class="tabs" role="tablist">
    <button role="tab" aria-selected={tab === 'contents'} class:on={tab === 'contents'} onclick={() => (tab = 'contents')}>{t('Contents')}{details ? ` · ${details.files.length}` : ''}</button>
    <button role="tab" aria-selected={tab === 'about'} class:on={tab === 'about'} onclick={() => (tab = 'about')}>{t('Description')}</button>
    <button role="tab" aria-selected={tab === 'versions'} class:on={tab === 'versions'} onclick={() => (tab = 'versions')}>{t('Versions')}{page ? ` · ${page.versions.length}` : ''}</button>
    {#if page?.gallery.length}<button role="tab" aria-selected={tab === 'gallery'} class:on={tab === 'gallery'} onclick={() => (tab = 'gallery')}>{t('Gallery')}</button>{/if}
  </div>

  <div class="pane">
    {#key tab}
    <div class="swap">
    {#if tab === 'contents'}
      {#if loading}
        <div class="state">
          <div class="line indeterminate"><div></div></div>
          <p class="muted">{t('Reading the modpack…')}</p>
        </div>
      {:else if failure}
        <div class="state">
          <p class="muted">{t('Could not read the modpack')}: {failure}</p>
          <button onclick={load}>{t('Retry')}</button>
        </div>
      {:else if details}
        <div class="bar">
          <label class="search grow">
            <span class="sr">{t('Search contents…')}</span>
            <span class="field"><Icon name="search" size={16} /><input type="search" bind:value={query} placeholder={t('Search contents…')} /></span>
          </label>
          <div class="seg" role="group" aria-label={t('Type')}>
            {#each kinds as k}
              <button aria-pressed={kind === k} onclick={() => (kind = k)}>{labels[k]()} · {counts[k]}</button>
            {/each}
          </div>
        </div>

        <div class="table">
          {#each rows as f, n (f.path)}
            <div class="tr" class:anim={n < 12} style:--d="{n * 30}ms">
              {#if f.project?.iconUrl}
                <img use:thumb={{ src: f.project.iconUrl, w: 96 }} alt="" width="40" height="40" decoding="async" />
              {:else}
                <span class="ico"><Icon name="file" size={18} /></span>
              {/if}
              <div class="pt">
                <div class="n">{title(f)}{#if f.optional}<span class="opt">{t('Optional')}</span>{/if}</div>
                <div class="d">{f.project ? f.project.description : f.fileName}</div>
              </div>
              <div class="ver">
                <div class="vn">{f.version?.number ?? formatBytes(f.size)}</div>
                <div class="vf">{f.version ? formatBytes(f.size) : ''}</div>
              </div>
            </div>
          {:else}
            <div class="empty muted">{t('Nothing matches your search.')}</div>
          {/each}
        </div>
        {#if details.overrides}<p class="muted small note">{t('Also includes {n} configuration files.', { n: details.overrides })}</p>{/if}
      {/if}
    {:else if tab === 'about'}
      {#if page}
        <div class="md" use:links>{@html body}</div>
      {:else}
        <div class="state"><p class="muted">{t('Loading…')}</p></div>
      {/if}
    {:else if tab === 'versions'}
      {#if page}
        <div class="vfilters">
          <label class="vsearch grow">
            <span class="sr">{t('Search versions')}</span>
            <span class="field"><Icon name="search" size={16} /><input type="search" bind:value={vquery} placeholder={t('Search versions…')} /></span>
          </label>
          <Select bind:value={vmc} width="170px" label={t('Minecraft version')} options={[{ value: '', label: t('All versions') }, ...mcOptions.map((m) => ({ value: m, label: m }))]} />
          {#if loaderOptions.length > 1}
            <Select bind:value={vloader} width="150px" label={t('Loader')} options={[{ value: '', label: t('All loaders') }, ...loaderOptions.map((l) => ({ value: l, label: loaderName(l) }))]} />
          {/if}
          {#if kindOptions.length > 1}
            <Select bind:value={vkind} width="140px" label={t('Release type')} options={[{ value: '', label: t('All types') }, ...kindOptions.map((k) => ({ value: k, label: k[0].toUpperCase() + k.slice(1) }))]} />
          {/if}
        </div>
        {#if filtering}<p class="muted small vcount">{t('{n} of {total} versions', { n: shownVersions.length, total: page.versions.length })} · <button class="link" onclick={clearFilters}>{t('Clear filters')}</button></p>{/if}
        {#if !shownVersions.length}
          <div class="state"><p class="muted">{t('No version matches.')}</p></div>
        {/if}
        <div class="table" class:none={!shownVersions.length}>
          {#each shownVersions as v (v.id)}
            <div class="tr vrow" class:cur={chosen?.id === v.id}>
              <div class="pt">
                <div class="n">{v.number}<span class="opt" class:rel={v.kind === 'release'}>{v.kind}</span></div>
                <div class="d">{v.mcVersions.slice(0, 4).join(', ')}{v.mcVersions.length > 4 ? '…' : ''} · {v.loaders.map(loaderName).join(', ')}</div>
              </div>
              <div class="ver">
                <div class="vn">{relativeTime(new Date(v.date).getTime())}</div>
                <div class="vf">{t('{n} downloads', { n: fmt(v.downloads) })}</div>
              </div>
              <div class="vact">
                <button class="sm" onclick={() => pick(v)}>{t('Contents')}</button>
                <button class="sm primary" onclick={() => onInstall(v.id)} {disabled}>{t('Install')}</button>
              </div>
            </div>
          {/each}
        </div>
      {:else}
        <div class="state"><p class="muted">{t('Loading…')}</p></div>
      {/if}
    {:else if page?.gallery.length}
      <div class="gallery">
        {#each page.gallery as g}
          <figure><img use:thumb={{ src: g.url, w: 960 }} alt={g.title ?? ''} decoding="async" />{#if g.title}<figcaption>{g.title}</figcaption>{/if}</figure>
        {/each}
      </div>
    {:else}
      <div class="state"><p class="muted">{t('Nothing here.')}</p></div>
    {/if}
    </div>
    {/key}
  </div>
</div>
</div>

<style>
  .backdrop { position: fixed; top: 40px; left: 0; right: 0; bottom: 0; padding: 0; border: 0; border-radius: 0; cursor: default; background: rgba(0, 0, 0, 0.66); z-index: 20; animation: fade 200ms ease both; }
  .backdrop:hover:not(:disabled) { background: rgba(0, 0, 0, 0.66); }
  .modal { position: fixed; z-index: 21; top: calc(50% + 20px); left: 50%; transform: translate(-50%, -50%); width: min(880px, calc(100vw - 40px)); height: min(740px, calc(100vh - 40px - 32px)); display: flex; flex-direction: column; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 26px; box-shadow: 0 30px 90px rgba(0, 0, 0, 0.55); overflow: hidden; animation: enter 320ms var(--spring) both; }
  .closing .backdrop { animation: unfade 180ms ease both; }
  .closing .modal { animation: leave 180ms ease-in both; }
  @keyframes unfade { to { opacity: 0; } }
  @keyframes leave { to { opacity: 0; transform: translate(-50%, calc(-50% + 14px)) scale(0.97); } }
  @keyframes enter { from { opacity: 0; transform: translate(-50%, calc(-50% + 18px)) scale(0.97); } }
  .hero { position: relative; flex: none; height: 200px; color: #fff; isolation: isolate; background: #0a0a0a; }
  .photo { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; z-index: -2; }
  .veil { position: absolute; inset: 0; z-index: -1; background: linear-gradient(to top, rgba(0, 0, 0, 0.92), rgba(0, 0, 0, 0.25) 80%); }
  .x { position: absolute; top: 14px; right: 14px; width: 36px; height: 36px; padding: 0; border-radius: 50%; background: rgba(0, 0, 0, 0.55); border-color: rgba(255, 255, 255, 0.25); color: #fff; display: grid; place-items: center; }
  .x:hover:not(:disabled) { background: rgba(0, 0, 0, 0.8); border-color: rgba(255, 255, 255, 0.6); }
  .id { position: absolute; left: 24px; right: 24px; bottom: 20px; display: flex; align-items: center; gap: 16px; }
  .icon { border-radius: 16px; border: 2px solid rgba(255, 255, 255, 0.9); background: #111; object-fit: cover; }
  .tt { min-width: 0; flex: 1; }
  h2 { margin: 0; color: #fff; font-weight: 650; font-size: 28px; letter-spacing: -0.04em; line-height: 1.1; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .by { font-size: 13px; opacity: 0.75; margin-top: 4px; }
  .go { flex: none; }

  .progress { display: grid; gap: 8px; padding: 14px 24px 0; }
  .meta { padding: 18px 24px 4px; display: grid; gap: 12px; }
  .desc { margin: 0; color: var(--muted); line-height: 1.5; }
  .tag { border: 1px solid var(--line-strong); border-radius: 999px; padding: 4px 12px; font-size: 12px; }
  .tag.soft { color: var(--muted); border-color: var(--line); text-transform: capitalize; }

  .tabs { display: flex; gap: 6px; padding: 8px 24px 0; border-bottom: 1px solid var(--line); }
  .tabs button { overflow: visible; border: 0; border-radius: 0; background: transparent; color: var(--muted); padding: 10px 6px; margin-right: 14px; }
  .tabs button:hover:not(:disabled) { background: transparent; color: var(--text); }
  .tabs button::after { content: ''; position: absolute; left: 0; right: 0; bottom: -1px; height: 2px; background: var(--text); transform: scaleX(0); transition: transform 320ms var(--spring); }
  .tabs button.on { color: var(--text); }
  .tabs button.on::after { transform: none; }

  .pane { padding: 18px 24px 24px; overflow-y: auto; flex: 1; min-height: 0; display: flex; flex-direction: column; gap: 14px; }
  .pane > * { flex-shrink: 0; }
  .swap { display: flex; flex-direction: column; gap: 14px; animation: swap 280ms var(--spring) both; }
  .swap > * { flex-shrink: 0; }
  .tr.anim { animation: rowin 460ms var(--spring) backwards var(--d); }
  @keyframes rowin { from { opacity: 0; transform: translateY(10px); } }
  @keyframes swap { from { opacity: 0; transform: translateY(8px); } }
  .state { display: grid; gap: 14px; justify-items: center; padding: 48px 0; text-align: center; }
  .state .line { width: min(260px, 60%); }
  .bar { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; }
  .search { min-width: 200px; }
  .field { position: relative; display: block; }
  .field :global(svg) { position: absolute; left: 13px; top: 50%; transform: translateY(-50%); color: var(--faint); pointer-events: none; }
  .field input { padding-left: 38px; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }

  .table { border: 1px solid var(--line); border-radius: 18px; overflow: hidden; background: var(--bg); }
  .tr { display: grid; grid-template-columns: 40px minmax(0, 1fr) minmax(0, 150px); align-items: center; gap: 14px; padding: 10px 16px; border-bottom: 1px solid var(--line); content-visibility: auto; contain-intrinsic-size: auto 62px; }
  .tr:last-child { border-bottom: 0; }
  .tr img, .ico { width: 40px; height: 40px; border-radius: 11px; background: var(--surface-2); object-fit: cover; }
  .ico { display: grid; place-items: center; color: var(--muted); }
  .pt { min-width: 0; }
  .n { font-weight: 600; letter-spacing: -0.02em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .opt { margin-left: 8px; font-size: 11px; font-weight: 500; color: var(--muted); border: 1px solid var(--line-strong); border-radius: 999px; padding: 1px 8px; vertical-align: 1px; }
  .d { color: var(--muted); font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .ver { text-align: right; min-width: 0; }
  .vn { font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .vf { color: var(--muted); font-size: 12px; }
  .empty { padding: 30px; text-align: center; }
  .note { margin: 0; }
  .gallery { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px; }
  .gallery figure { margin: 0; display: grid; gap: 6px; }
  .gallery figcaption { font-size: 12px; color: var(--muted); }
  .gallery img { width: 100%; aspect-ratio: 16 / 9; object-fit: cover; border-radius: 16px; border: 1px solid var(--line); background: var(--surface-2); }
  .vfilters { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; margin-bottom: 12px; }
  .vsearch { min-width: 200px; }
  .vcount { margin: 0 0 10px; }
  .link { display: inline; padding: 0; border: 0; background: none; color: var(--text); text-decoration: underline; text-underline-offset: 3px; border-radius: 0; }
  .link:hover:not(:disabled) { background: none; }
  .table.none { display: none; }
  .vrow { grid-template-columns: minmax(0, 1fr) minmax(0, 150px) auto; }
  .vrow.cur { background: var(--surface-2); }
  .vact { display: flex; gap: 8px; }
  .sm { padding: 6px 14px; font-size: 13px; }
  .opt.rel { color: var(--text); }
  .md { line-height: 1.6; color: var(--muted); overflow-wrap: anywhere; }
  .md :global(h2), .md :global(h3), .md :global(h4), .md :global(h5) { color: var(--text); font-size: 17px; font-weight: 650; letter-spacing: -0.03em; margin: 22px 0 8px; }
  .md :global(p) { margin: 0 0 12px; }
  .md :global(ul), .md :global(ol) { margin: 0 0 12px; padding-left: 22px; }
  .md :global(strong) { color: var(--text); font-weight: 600; }
  .md :global(a) { color: var(--text); text-decoration: underline; text-underline-offset: 3px; cursor: pointer; }
  .md :global(code) { background: var(--surface-2); border-radius: 6px; padding: 1px 6px; font-size: 0.92em; }
  .md :global(pre) { background: var(--bg); border: 1px solid var(--line); border-radius: 12px; padding: 12px 14px; overflow-x: auto; }
  .md :global(pre code) { background: none; padding: 0; }
  .md :global(img) { max-width: 100%; height: auto; border-radius: 12px; }
  .md :global(blockquote) { margin: 0 0 12px; padding-left: 14px; border-left: 2px solid var(--line-strong); }
  .md :global(hr) { border: 0; border-top: 1px solid var(--line); margin: 18px 0; }
  @keyframes fade { from { opacity: 0; } }
  @media (max-width: 700px) { .vrow { grid-template-columns: minmax(0, 1fr) auto; } .tr { grid-template-columns: 40px minmax(0, 1fr); } .ver { display: none; } .id { flex-wrap: wrap; } }
</style>
