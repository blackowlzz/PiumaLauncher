<script lang="ts">
  import { thumb } from './img'
  import { api, modrinth, type ContentEntry, type ContentKind, type Instance, type ProjectType } from './api'
  import { fade } from 'svelte/transition'
  import Icon from './Icon.svelte'
  import Select from './Select.svelte'
  import { t } from './i18n.svelte'
  import { formatBytes } from './meta'
  import { app, notify } from './store.svelte'

  let { inst, busy = false }: { inst: Instance; busy?: boolean } = $props()

  const kinds: { id: ContentKind; label: string; type: ProjectType }[] = [
    { id: 'mods', label: 'Mod', type: 'mod' },
    { id: 'resourcepacks', label: 'Resource pack', type: 'resourcepack' },
    { id: 'shaderpacks', label: 'Shader', type: 'shader' },
  ]

  let kind: ContentKind = $state('mods')
  let entries: ContentEntry[] = $state([])
  let checking = $state(false)
  let query = $state('')
  let sortBy = $state('name-asc')
  let filter: 'all' | 'enabled' | 'disabled' | 'updates' = $state('all')
  let selected: Record<string, boolean> = $state({})
  let working: Record<string, boolean> = $state({})
  let token = 0

  const vanillaMods = $derived(kind === 'mods' && inst.loader === 'vanilla')
  const title = (e: ContentEntry) => e.project?.title ?? e.fileName.replace(/\.(jar|zip)(\.disabled)?$/, '')
  const baseName = (n: string) => n.replace(/\.disabled$/, '')

  async function load() {
    const my = ++token
    selected = {}
    const files = await modrinth.listContent(inst.id, kind).catch(() => [])
    if (my !== token) return
    entries = files.map((f) => ({ ...f, project: null, version: null, update: null }))
    checking = files.length > 0
    modrinth
      .contentDetails(inst.id, kind)
      .then((d) => my === token && (entries = d))
      .catch(() => {})
      .finally(() => my === token && (checking = false))
  }

  $effect(() => {
    void [inst.id, kind]
    load()
  })

  const rows = $derived.by(() => {
    const q = query.trim().toLowerCase()
    const list = entries.filter((e) => {
      if (q && !`${title(e)} ${e.fileName} ${e.project?.description ?? ''}`.toLowerCase().includes(q)) return false
      return filter === 'all' || (filter === 'enabled' && e.enabled) || (filter === 'disabled' && !e.enabled) || (filter === 'updates' && !!e.update)
    })
    const dir = sortBy.endsWith('desc') ? -1 : 1
    return list.sort((a, b) => (sortBy.startsWith('size') ? a.size - b.size : title(a).localeCompare(title(b))) * dir)
  })
  const chosen = $derived(rows.filter((e) => selected[e.fileName]))
  const allChosen = $derived(rows.length > 0 && chosen.length === rows.length)
  const updatable = $derived(entries.filter((e) => e.update).length)

  async function guard(name: string, fn: () => Promise<void>) {
    working[name] = true
    try {
      await fn()
    } catch (e) {
      notify(e)
    } finally {
      delete working[name]
    }
  }
  const rename = (from: string, patch: Partial<ContentEntry>) => {
    entries = entries.map((e) => (e.fileName === from ? { ...e, ...patch } : e))
    if (patch.fileName && patch.fileName !== from) {
      selected[patch.fileName] = selected[from]
      delete selected[from]
    }
  }

  const setEnabled = (e: ContentEntry, enabled: boolean) =>
    guard(e.fileName, async () => {
      await modrinth.setModEnabled(inst.id, e.fileName, enabled)
      rename(e.fileName, { fileName: enabled ? baseName(e.fileName) : `${baseName(e.fileName)}.disabled`, enabled })
    })

  const remove = (e: ContentEntry) =>
    guard(e.fileName, async () => {
      await modrinth.removeContent(inst.id, kind, e.fileName)
      entries = entries.filter((x) => x.fileName !== e.fileName)
      delete selected[e.fileName]
    })

  const update = (e: ContentEntry) =>
    guard(e.fileName, async () => {
      const next = e.update
      const name = await modrinth.updateContent(inst.id, kind, e.fileName)
      rename(e.fileName, { fileName: name, version: next, update: null })
    })

  async function bulk(fn: (e: ContentEntry) => Promise<void>) {
    for (const e of [...chosen]) await fn(e)
  }
  const updateAll = async () => {
    for (const e of entries.filter((x) => x.update)) await update(e)
  }
  const toggleAll = (on: boolean) => rows.forEach((e) => (selected[e.fileName] = on))

  let transfer: 'copy' | 'move' | null = $state(null)
  const targets = $derived(app.instances.filter((i) => i.id !== inst.id && !(kind === 'mods' && i.loader === 'vanilla')))

  async function sendTo(target: Instance) {
    const mode = transfer
    const files = chosen.map((e) => e.fileName)
    transfer = null
    if (!mode || !files.length) return
    try {
      const r = await modrinth.transferContent(inst.id, target.id, kind, files, mode === 'move')
      if (mode === 'move' && r.done) await load()
      notify(
        r.skipped
          ? t('{done} sent to "{name}", {skipped} already there', { done: r.done, name: target.name, skipped: r.skipped })
          : t('{done} sent to "{name}"', { done: r.done, name: target.name }),
      )
    } catch (e) {
      notify(e)
    }
  }

  function browse() {
    app.exploreIntent = { kind: kinds.find((k) => k.id === kind)!.type, targetId: inst.id }
    app.page = 'explore'
  }

  let dragging = $state(false)

  async function importFiles(paths: string[]) {
    if (vanillaMods || !paths.length) return
    try {
      const r = await modrinth.importContent(inst.id, kind, paths)
      if (r.done) await load()
      notify(r.skipped ? t('{done} added, {skipped} skipped', r) : t('{done} added', r))
    } catch (e) {
      notify(e)
    }
  }

  $effect(() => {
    let off: (() => void) | undefined
    let gone = false
    import('@tauri-apps/api/webview').then(async ({ getCurrentWebview }) => {
      const un = await getCurrentWebview().onDragDropEvent((ev) => {
        const p = ev.payload
        if (p.type === 'enter' || p.type === 'over') dragging = !vanillaMods
        else if (p.type === 'leave') dragging = false
        else if (p.type === 'drop') {
          dragging = false
          importFiles(p.paths)
        }
      })
      if (gone) un()
      else off = un
    })
    return () => {
      gone = true
      off?.()
    }
  })

  const filters = [
    { id: 'all', label: () => t('All') },
    { id: 'enabled', label: () => t('Enabled') },
    { id: 'disabled', label: () => t('Disabled') },
  ] as const
</script>

{#if dragging}
  <div class="drop" transition:fade={{ duration: 120 }}>{t('Drop files to add them')}</div>
{/if}

<div class="bar">
  <label class="search grow">
    <span class="sr">{t('Search {n} items…', { n: entries.length })}</span>
    <span class="field"><Icon name="search" size={16} /><input type="search" bind:value={query} placeholder={t('Search {n} items…', { n: entries.length })} /></span>
  </label>
  <button onclick={() => api.openFolder(inst.id, kind === 'mods' ? 'mods' : 'game').catch(notify)}><Icon name="folder" size={15} />{t('Open folder')}</button>
  <button class="primary" onclick={browse} disabled={vanillaMods}>{t('Add from Modrinth')}</button>
</div>

<div class="bar sub">
  <div class="seg" role="group" aria-label={t('Type')}>
    {#each kinds as k}
      <button aria-pressed={kind === k.id} onclick={() => (kind = k.id)}>{k.label}</button>
    {/each}
  </div>
  <Select
    bind:value={sortBy}
    width="auto"
    label={t('Sort by')}
    options={[
      { value: 'name-asc', label: t('Name (A–Z)') },
      { value: 'name-desc', label: t('Name (Z–A)') },
      { value: 'size-desc', label: t('Size') },
    ]}
  />
  <div class="chips">
    {#each filters as f}
      <button class="chip" aria-pressed={filter === f.id} onclick={() => (filter = f.id)}>{f.label()}</button>
    {/each}
    {#if updatable}<button class="chip" aria-pressed={filter === 'updates'} onclick={() => (filter = 'updates')}>{t('Updates')} · {updatable}</button>{/if}
  </div>
  <span class="grow"></span>
  {#if updatable}<button onclick={updateAll}><Icon name="refresh" size={15} />{t('Update all')}</button>{/if}
  <button class="ghost" onclick={load} disabled={checking} aria-label={t('Refresh')} title={t('Refresh')}><Icon name="refresh" size={15} />{checking ? t('Checking…') : t('Refresh')}</button>
</div>

{#if vanillaMods}
  <p class="muted">{t('This instance is Vanilla: to use mods, create a Fabric or Quilt instance.')}</p>
{:else}
  <div class="table">
    {#if chosen.length}
      <div class="th sel">
        <label class="chk"><input type="checkbox" checked={allChosen} onchange={(e) => toggleAll(e.currentTarget.checked)} aria-label={t('Select all')} /></label>
        <span class="grow">{t('{n} selected', { n: chosen.length })}</span>
        <button class="sm" onclick={() => bulk((e) => setEnabled(e, true))} disabled={busy}>{t('Enable')}</button>
        <button class="sm" onclick={() => bulk((e) => setEnabled(e, false))} disabled={busy}>{t('Disable')}</button>
        {#if chosen.some((e) => e.update)}<button class="sm" onclick={() => bulk((e) => (e.update ? update(e) : Promise.resolve()))} disabled={busy}>{t('Update')}</button>{/if}
        <div class="pop">
          <button class="sm" onclick={() => (transfer = transfer === 'copy' ? null : 'copy')} aria-expanded={transfer === 'copy'}>{t('Copy to…')}</button>
          <button class="sm" onclick={() => (transfer = transfer === 'move' ? null : 'move')} disabled={busy} aria-expanded={transfer === 'move'}>{t('Move to…')}</button>
          {#if transfer}
            <ul class="menu" role="menu" transition:fade={{ duration: 120 }}>
              {#each targets as i (i.id)}
                <li><button role="menuitem" onclick={() => sendTo(i)}>{i.name}<span class="muted">{i.mcVersion}</span></button></li>
              {:else}
                <li class="none muted">{t('No other instance available.')}</li>
              {/each}
            </ul>
          {/if}
        </div>
        <button class="sm" onclick={() => bulk(remove)} disabled={busy}>{t('Remove')}</button>
      </div>
    {:else}
      <div class="th">
        <label class="chk"><input type="checkbox" checked={allChosen} onchange={(e) => toggleAll(e.currentTarget.checked)} aria-label={t('Select all')} disabled={!rows.length} /></label>
        <span>{t('Project')}</span>
        <span>{t('Version')}</span>
        <span class="r">{t('Actions')}</span>
      </div>
    {/if}

    {#each rows as e, n (e.fileName)}
      <div class="tr" class:off={!e.enabled} class:wip={working[e.fileName]} class:anim={n < 12} style:--d="{n * 30}ms" out:fade={{ duration: 160 }}>
        <label class="chk"><input type="checkbox" bind:checked={selected[e.fileName]} aria-label={title(e)} /></label>

        <div class="proj">
          {#if e.project?.iconUrl}
            <img use:thumb={{ src: e.project.iconUrl, w: 96 }} alt="" width="42" height="42" decoding="async" />
          {:else}
            <span class="ico"><Icon name="file" size={20} /></span>
          {/if}
          <div class="pt">
            <div class="n">{title(e)}</div>
            <div class="d">{e.project ? e.project.description : t('Uploaded')}</div>
          </div>
        </div>

        <div class="ver">
          <div class="vn">{e.version?.number ?? formatBytes(e.size)}</div>
          <div class="vf">{baseName(e.fileName)}</div>
        </div>

        <div class="act">
          {#if e.update}
            <button class="iconbtn upd" title={t('Update to {version}', { version: e.update.number })} aria-label={t('Update to {version}', { version: e.update.number })} onclick={() => update(e)} disabled={busy || working[e.fileName]}>
              <Icon name="refresh" size={16} />
            </button>
          {/if}
          {#if kind === 'mods'}
            <label class="switch" title={e.enabled ? t('Disable') : t('Enable')}>
              <input type="checkbox" checked={e.enabled} disabled={busy || working[e.fileName]} aria-label={t('Enable {name}', { name: title(e) })} onchange={(ev) => setEnabled(e, ev.currentTarget.checked)} />
              <span class="track"></span>
            </label>
          {/if}
          <button class="iconbtn ghost" title={t('Remove')} aria-label={t('Remove')} onclick={() => remove(e)} disabled={busy || working[e.fileName]}><Icon name="trash" size={16} /></button>
        </div>
      </div>
    {:else}
      <div class="empty muted">{entries.length ? t('Nothing matches your search.') : t('Nothing installed.')}</div>
    {/each}
  </div>
{/if}

<style>
  .bar { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .bar button { display: inline-flex; align-items: center; gap: 8px; }
  .bar.sub { margin-top: 2px; }
  .search { min-width: 220px; }
  .field { position: relative; display: block; }
  .field :global(svg) { position: absolute; left: 13px; top: 50%; transform: translateY(-50%); color: var(--faint); pointer-events: none; }
  .field input { padding-left: 38px; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }

  .drop { position: fixed; inset: 56px 8px 8px 8px; z-index: 50; display: grid; place-items: center; border: 2px dashed var(--line-strong); border-radius: 20px; background: color-mix(in srgb, var(--surface) 88%, transparent); font-size: 18px; font-weight: 600; pointer-events: none; }
  .table { border: 1px solid var(--line); border-radius: 20px; overflow: hidden; background: var(--surface); }
  .th, .tr { display: grid; grid-template-columns: 44px minmax(0, 1.7fr) minmax(0, 1fr) 132px; align-items: center; gap: 12px; padding: 0 18px 0 14px; }
  .th { height: 48px; border-bottom: 1px solid var(--line); font-size: 13px; font-weight: 600; color: var(--muted); }
  .th.sel { display: flex; gap: 8px; color: var(--text); }
  .th .r { text-align: right; padding-right: 8px; }
  .tr { min-height: 72px; border-bottom: 1px solid var(--line); content-visibility: auto; contain-intrinsic-size: auto 72px; transition: opacity var(--ease); }
  .tr.anim { animation: rowin 460ms var(--spring) backwards var(--d); }
  @keyframes rowin { from { opacity: 0; transform: translateY(10px); } }
  .tr:last-child { border-bottom: 0; }
  .tr.off .proj, .tr.off .ver { opacity: 0.45; }
  .tr.wip { opacity: 0.6; pointer-events: none; }
  .chk { display: grid; place-items: center; }
  .chk input { cursor: pointer; }
  .proj { display: flex; align-items: center; gap: 14px; min-width: 0; }
  .proj img, .ico { width: 42px; height: 42px; border-radius: 12px; flex: none; background: var(--surface-2); object-fit: cover; }
  .ico { display: grid; place-items: center; color: var(--muted); }
  .pt { min-width: 0; }
  .n { font-weight: 600; letter-spacing: -0.02em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .d { color: var(--muted); font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .ver { min-width: 0; }
  .vn { font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .vf { color: var(--muted); font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .act { display: flex; align-items: center; justify-content: flex-end; gap: 6px; }
  .upd { background: var(--inv-bg); color: var(--inv-text); border-color: var(--inv-bg); }
  .upd:hover:not(:disabled) { background: var(--inv-bg); border-color: var(--inv-bg); opacity: 0.85; }
  .pop { position: relative; display: flex; gap: 8px; }
  .menu { position: absolute; z-index: 30; top: calc(100% + 8px); right: 0; min-width: 240px; max-height: 280px; overflow: auto; margin: 0; padding: 6px; list-style: none; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 14px; box-shadow: 0 18px 50px rgba(0, 0, 0, 0.45); }
  .menu button { display: flex; width: 100%; justify-content: space-between; gap: 16px; border-color: transparent; background: transparent; text-align: left; border-radius: 10px; padding: 9px 12px; }
  .menu button:hover { background: var(--surface-2); }
  .menu .none { padding: 10px 12px; font-size: 13px; }
  .table { overflow: visible; }
  .sm { padding: 6px 14px; font-size: 13px; }
  .empty { padding: 36px 18px; text-align: center; }
  @media (max-width: 900px) { .th, .tr { grid-template-columns: 40px minmax(0, 1fr) auto; } .ver, .th span:nth-child(3) { display: none; } }
</style>
