<script lang="ts">
  import { fade } from 'svelte/transition'
  import { api, type Backup, type WorldEntry } from './api'
  import Art from './Art.svelte'
  import Icon from './Icon.svelte'
  import Select from './Select.svelte'
  import { ctx, type MenuItem } from './contextmenu'
  import { locale, t } from './i18n.svelte'
  import { formatBytes, relativeTime } from './meta'
  import { opensWorlds } from './serverstatus.svelte'
  import { app, askConfirm, isBusy, notify, openInstance, photoFor, play } from './store.svelte'
  import { thumb } from './img'

  let worlds: WorldEntry[] = $state([])
  let loaded = $state(false)
  let query = $state('')
  let sort: 'recent' | 'name' | 'size' = $state('recent')
  let only: string = $state('all')
  let open: string | null = $state(null)
  let backups: Record<string, Backup[]> = $state({})
  let working: string | null = $state(null)
  let picking: { kind: 'copy' | 'move' | 'import'; world?: WorldEntry; bytes?: Uint8Array; file?: string } | null = $state(null)
  let picker: HTMLInputElement | undefined = $state()

  const keyOf = (w: WorldEntry) => `${w.instanceId}/${w.folder}`
  const instOf = (id: string) => app.instances.find((i) => i.id === id)
  const modes = $derived([t('Survival'), t('Creative'), t('Adventure'), t('Spectator')])

  async function load() {
    worlds = await api.allWorlds().catch(() => [])
    loaded = true
  }
  $effect(() => {
    void app.sessions
    void app.instances
    load()
  })

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase()
    return worlds
      .filter((w) => only === 'all' || w.instanceId === only)
      .filter((w) => !q || `${w.name} ${w.folder} ${instOf(w.instanceId)?.name ?? ''}`.toLowerCase().includes(q))
      .sort((a, b) => (sort === 'name' ? a.name.localeCompare(b.name) : sort === 'size' ? b.size - a.size : b.modifiedMs - a.modifiedMs))
  })
  const instanceCount = $derived(new Set(worlds.map((w) => w.instanceId)).size)

  async function loadBackups(w: WorldEntry) {
    const list = await api.listBackups(w.instanceId).catch(() => [] as Backup[])
    backups[keyOf(w)] = list.filter((b) => b.world === w.folder)
  }

  async function toggleBackups(w: WorldEntry) {
    const key = keyOf(w)
    open = open === key ? null : key
    if (open) await loadBackups(w)
  }

  async function run(w: WorldEntry, fn: () => Promise<unknown>) {
    if (working) return
    working = keyOf(w)
    try {
      await fn()
    } catch (e) {
      notify(e)
    } finally {
      working = null
    }
  }

  const backupNow = (w: WorldEntry) =>
    run(w, async () => {
      await api.backupWorld(w.instanceId, w.folder)
      notify(t('Backup created'))
      open = keyOf(w)
      await loadBackups(w)
    })

  const restore = (w: WorldEntry, b: Backup, asCopy: boolean) =>
    run(w, async () => {
      if (!asCopy && !(await askConfirm(t('Bring "{name}" back to {date}? The current world is backed up first, so you can undo this.', { name: w.name, date: when(b.createdMs) }), t('Restore')))) return
      const folder = await api.restoreBackup(w.instanceId, w.folder, b.id, asCopy)
      notify(asCopy ? t('Restored as "{name}"', { name: folder }) : t('"{name}" restored', { name: w.name }))
      await load()
      await loadBackups(w)
    })

  const removeBackup = (w: WorldEntry, b: Backup) =>
    run(w, async () => {
      if (!(await askConfirm(t('Delete the backup of {date}?', { date: when(b.createdMs) })))) return
      await api.deleteBackup(w.instanceId, w.folder, b.id)
      await loadBackups(w)
    })

  const exportZip = (w: WorldEntry) =>
    run(w, async () => {
      const path = await api.exportWorld(w.instanceId, w.folder)
      notify(t('Saved in Downloads'))
      api.revealFile(path).catch(() => {})
    })

  function enter(w: WorldEntry) {
    const inst = instOf(w.instanceId)
    if (!inst) return
    if (opensWorlds(inst.mcVersion)) play(inst.id, { kind: 'world', folder: w.folder })
    else play(inst.id)
  }

  function menu(w: WorldEntry): MenuItem[] {
    const busy = isBusy(w.instanceId)
    return [
      { label: t('Play'), icon: 'play', onclick: () => enter(w), disabled: busy || !app.accounts.active },
      { label: t('Back up now'), icon: 'clock', onclick: () => void backupNow(w), disabled: busy },
      'sep',
      { label: t('Copy to another instance'), icon: 'copy', onclick: () => (picking = { kind: 'copy', world: w }), disabled: busy },
      { label: t('Move to another instance'), icon: 'share', onclick: () => (picking = { kind: 'move', world: w }), disabled: busy || app.instances.length < 2 },
      { label: t('Export as .zip'), icon: 'package', onclick: () => void exportZip(w), disabled: busy },
      'sep',
      { label: t('Open folder'), icon: 'folder', onclick: () => void api.openFolder(w.instanceId, 'saves').catch(notify) },
      { label: t('Open instance'), icon: 'settings', onclick: () => openInstance(w.instanceId) },
    ]
  }

  function moreMenu(e: MouseEvent, w: WorldEntry) {
    e.stopPropagation()
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
    app.ctx = { x: r.left, y: r.bottom + 6, items: menu(w) }
  }

  async function chooseFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement
    const file = input.files?.[0]
    input.value = ''
    if (!file) return
    picking = { kind: 'import', bytes: new Uint8Array(await file.arrayBuffer()), file: file.name }
  }

  async function confirmTarget(targetId: string) {
    const p = picking
    if (!p) return
    picking = null
    const target = instOf(targetId)
    if (!target) return
    if (p.kind === 'import' && p.bytes) {
      working = 'import'
      try {
        const name = await api.importWorldBytes(target.id, p.bytes)
        notify(t('"{name}" imported into {instance}', { name, instance: target.name }))
        await load()
      } catch (e) {
        notify(e)
      } finally {
        working = null
      }
      return
    }
    const w = p.world
    if (!w) return
    await run(w, async () => {
      await api.transferWorld(w.instanceId, w.folder, target.id, p.kind === 'move')
      notify(p.kind === 'move' ? t('"{name}" moved to {instance}', { name: w.name, instance: target.name }) : t('"{name}" copied to {instance}', { name: w.name, instance: target.name }))
      await load()
    })
  }

  const targets = $derived.by(() => {
    const p = picking
    if (!p) return []
    return app.instances.filter((i) => p.kind !== 'move' || i.id !== p.world?.instanceId)
  })

  const reasonLabel = (r: string) =>
    ({ manual: t('Manual'), launch: t('Before playing'), update: t('Before mod changes'), restore: t('Before restoring'), move: t('Before moving') })[r] ?? r
  const when = (ms: number) => new Date(ms).toLocaleString(locale(), { day: 'numeric', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit' })
</script>

<header class="head">
  <div class="grow">
    <h1>{t('Worlds')}</h1>
    <p class="muted">
      {loaded && worlds.length ? t('{n} worlds in {m} instances', { n: worlds.length, m: instanceCount }) : t('Every singleplayer world from all your instances, in one place.')}
    </p>
  </div>
  <div class="hacts">
    <button onclick={() => picker?.click()} disabled={working === 'import'}><Icon name="download" size={15} />{working === 'import' ? t('Importing…') : t('Import world')}</button>
  </div>
  <input bind:this={picker} type="file" accept=".zip" hidden onchange={chooseFile} />
</header>

{#if worlds.length}
  <div class="toolbar">
    <label class="search grow">
      <span class="field"><Icon name="search" size={16} /><input type="search" bind:value={query} placeholder={t('Search worlds…')} aria-label={t('Search worlds…')} /></span>
    </label>
    <Select bind:value={only} width="auto" label={t('Instance')} options={[{ value: 'all', label: t('All instances') }, ...app.instances.filter((i) => worlds.some((w) => w.instanceId === i.id)).map((i) => ({ value: i.id, label: i.name }))]} />
    <Select
      bind:value={sort}
      width="auto"
      label={t('Sort by')}
      options={[
        { value: 'recent', label: t('Last played') },
        { value: 'name', label: t('Name') },
        { value: 'size', label: t('Size') },
      ]}
    />
  </div>
{/if}

{#if loaded && !worlds.length}
  <div class="empty">
    <div class="eart">{#if photoFor(undefined, 'worlds')}<img class="photo" data-bare use:thumb={{ src: photoFor(undefined, 'worlds') ?? '', w: 640 }} alt="" decoding="async" />{:else}<Art seed="worlds" />{/if}</div>
    <div class="etxt">
      <h2>{t('No singleplayer worlds yet')}</h2>
      <p class="muted">{t('Worlds you create in any instance show up here, with their backups. You can also import one from a .zip file.')}</p>
      <button class="primary" onclick={() => picker?.click()}><Icon name="download" size={15} />{t('Import world')}</button>
    </div>
  </div>
{:else if loaded && !shown.length}
  <p class="muted">{t('No world matches "{query}".', { query })}</p>
{:else}
  <div class="grid">
    {#each shown as w, n (keyOf(w))}
      {@const inst = instOf(w.instanceId)}
      {@const key = keyOf(w)}
      {@const list = backups[key] ?? []}
      <article class="card" class:wide={open === key} style:--d="{Math.min(n, 12) * 40}ms" use:ctx={() => menu(w)}>
        <div class="cover">
          {#if w.icon}
            <img class="bg" src={w.icon} alt="" />
            <img class="wicon" src={w.icon} alt="" />
          {:else if photoFor(w.version ?? undefined, w.name)}
            <img class="photo" data-bare use:thumb={{ src: photoFor(w.version ?? undefined, w.name) ?? '', w: 640 }} alt="" decoding="async" />
          {:else}
            <Art seed={w.name} />
          {/if}
          <span class="shade"></span>
          {#if w.mode !== null && modes[w.mode]}<span class="badge">{modes[w.mode]}</span>{/if}
          {#if w.hardcore}<span class="badge hc">Hardcore</span>{/if}
        </div>
        <div class="body">
          <div class="name" title={w.name}>{w.name}</div>
          <div class="muted small inst" title={inst?.name}>{inst?.name ?? w.instanceId}{w.version ? ` · ${w.version}` : ''}</div>
          <div class="meta muted small">
            <span><Icon name="clock" size={13} />{relativeTime(w.modifiedMs)}</span>
            <span>{formatBytes(w.size)}</span>
          </div>
        </div>
        <div class="acts">
          <button class="primary" onclick={() => enter(w)} disabled={isBusy(w.instanceId) || !app.accounts.active} title={inst && !opensWorlds(inst.mcVersion) ? t('Opening a world directly needs Minecraft 1.20 or newer') : undefined}><Icon name="play" size={12} />{t('Play')}</button>
          <button class:on={open === key} onclick={() => toggleBackups(w)} aria-expanded={open === key}><Icon name="clock" size={13} />{t('Backups')}</button>
          <button class="ghost iconbtn more" onclick={(e) => moreMenu(e, w)} aria-label={t('More')} title={t('More')}><Icon name="dots" size={16} /></button>
        </div>
        {#if open === key}
          <div class="backups" transition:fade={{ duration: 140 }}>
            <div class="bhead">
              <span class="muted small grow">{list.length ? t('Restoring brings the world back to that moment.') : t('No backups yet.')}</span>
              <button class="sm" onclick={() => backupNow(w)} disabled={working !== null || isBusy(w.instanceId)}>{working === key ? t('Backing up…') : t('Back up now')}</button>
            </div>
            {#each list as b (b.id)}
              <div class="brow">
                <span class="grow">{when(b.createdMs)}<span class="tag reason">{reasonLabel(b.reason)}</span></span>
                <span class="muted small">{formatBytes(b.size)}</span>
                <button class="ghost sm" onclick={() => restore(w, b, true)} disabled={working !== null || isBusy(w.instanceId)}>{t('Restore as copy')}</button>
                <button class="sm" onclick={() => restore(w, b, false)} disabled={working !== null || isBusy(w.instanceId)}>{t('Restore')}</button>
                <button class="ghost iconbtn sm" onclick={() => removeBackup(w, b)} disabled={working !== null} aria-label={t('Delete')}><Icon name="trash" size={14} /></button>
              </div>
            {/each}
          </div>
        {/if}
      </article>
    {/each}
  </div>
{/if}

{#if picking}
  {@const p = picking}
  <div class="backdrop" role="presentation" transition:fade={{ duration: 140 }} onclick={() => (picking = null)}></div>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="pick-title">
    <h2 id="pick-title">
      {p.kind === 'import' ? t('Import "{name}" into…', { name: p.file ?? '' }) : p.kind === 'move' ? t('Move "{name}" to…', { name: p.world?.name ?? '' }) : t('Copy "{name}" to…', { name: p.world?.name ?? '' })}
    </h2>
    {#if p.kind === 'move'}<p class="muted small">{t('A backup of the world is made first, then it leaves its current instance.')}</p>{/if}
    <div class="targets">
      {#each targets as i (i.id)}
        {@const differs = !!p.world?.version && p.world.version !== i.mcVersion}
        <button class="target" onclick={() => confirmTarget(i.id)}>
          <span class="grow">
            <span class="tn">{i.name}{i.id === p.world?.instanceId ? ` · ${t('same instance')}` : ''}</span>
            <span class="muted small">Minecraft {i.mcVersion}</span>
          </span>
          {#if differs}<span class="warnv" title={t('This world was last played on {version}. Opening it on another version can damage it, so keep a backup.', { version: p.world?.version ?? '' })}>{t('Different version')}</span>{/if}
          <Icon name="chevron" size={15} />
        </button>
      {/each}
    </div>
    <div class="dfoot"><button class="ghost" onclick={() => (picking = null)}>{t('Cancel')}</button></div>
  </div>
{/if}

<style>
  .head { display: flex; align-items: flex-start; gap: 16px; }
  .head p { margin-top: 6px; }
  .hacts { display: flex; gap: 8px; }
  .hacts button { display: inline-flex; align-items: center; gap: 8px; }
  .toolbar { display: flex; flex-wrap: wrap; gap: 12px; align-items: center; }
  .search { min-width: 220px; }
  .field { position: relative; display: block; }
  .field :global(svg) { position: absolute; left: 11px; top: 50%; transform: translateY(-50%); color: var(--faint); pointer-events: none; }
  .field input { padding-left: 34px; }
  .empty { display: grid; grid-template-columns: 220px minmax(0, 1fr); gap: 28px; align-items: center; padding: 28px; border: 1px solid var(--line); border-radius: 22px; background: var(--surface); }
  .eart { position: relative; height: 160px; border-radius: 16px; overflow: hidden; }
  .etxt { display: grid; gap: 10px; justify-items: start; }
  .etxt h2 { margin: 0; color: var(--text); font-size: 20px; font-weight: 600; letter-spacing: -0.03em; }
  .etxt button { display: inline-flex; align-items: center; gap: 8px; margin-top: 6px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 16px; align-items: start; }
  .card { position: relative; display: grid; grid-template-rows: auto 1fr auto auto; border-radius: 22px; border: 1px solid var(--line); background: var(--surface); overflow: hidden; transition: border-color var(--ease), transform 380ms var(--spring); animation: cardin 460ms var(--spring) var(--d) backwards; min-width: 0; }
  .card:hover { border-color: var(--line-strong); transform: translateY(-3px); }
  .card.wide { grid-column: 1 / -1; transform: none; }
  @keyframes cardin { from { opacity: 0; transform: translateY(12px); } }
  .cover { position: relative; aspect-ratio: 16 / 7; display: grid; place-items: center; overflow: hidden; background: #000; }
  .card.wide .cover { aspect-ratio: auto; height: 140px; }
  .cover :global(.art) { position: absolute; inset: 0; }
  .bg { position: absolute; inset: -10%; width: 120%; height: 120%; object-fit: cover; opacity: 0.55; image-rendering: auto; }
  .photo { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; transition: transform 900ms var(--spring); }
  .card:hover .photo { transform: scale(1.05); }
  .wicon { position: relative; z-index: 1; width: 64px; height: 64px; border-radius: 14px; image-rendering: pixelated; box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45); }
  .shade { position: absolute; inset: 0; background: linear-gradient(to bottom, rgba(0, 0, 0, 0.3), transparent 45%, rgba(0, 0, 0, 0.35)); }
  .badge { position: absolute; z-index: 2; top: 10px; left: 10px; display: inline-flex; align-items: center; height: 24px; padding: 0 10px; border-radius: 999px; font-size: 11.5px; color: #fff; background: rgba(0, 0, 0, 0.55); }
  .badge.hc { left: auto; right: 10px; background: #c4262b; font-weight: 600; }
  .body { display: grid; gap: 4px; padding: 14px 16px 4px; min-width: 0; align-content: start; }
  .name { font-weight: 600; font-size: 15px; letter-spacing: -0.02em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .inst { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .meta { display: flex; justify-content: space-between; gap: 12px; margin-top: 8px; }
  .meta span { display: inline-flex; align-items: center; gap: 6px; }
  .acts { display: flex; gap: 8px; align-items: center; padding: 12px 16px 16px; }
  .acts button { display: inline-flex; align-items: center; justify-content: center; gap: 7px; height: 36px; padding: 0 16px; font-size: 13.5px; line-height: 1; }
  .acts button.on { background: var(--surface-2); border-color: var(--faint); }
  .acts .more { width: 36px; padding: 0; margin-left: auto; }
  .sm { display: inline-flex; align-items: center; gap: 6px; padding: 6px 14px; font-size: 13px; }
  .backups { display: grid; gap: 4px; margin: 0 16px 16px; padding: 12px 14px; border-radius: 14px; background: var(--surface-2); }
  .bhead { display: flex; align-items: center; gap: 10px; padding-bottom: 6px; }
  .brow { display: flex; align-items: center; gap: 10px; padding: 6px 0; border-top: 1px solid var(--line); }
  .reason { margin-left: 8px; }
  .backdrop { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.62); z-index: 50; }
  .dialog { position: fixed; z-index: 51; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(480px, calc(100vw - 32px)); max-height: calc(100vh - 48px); display: grid; grid-template-rows: auto auto minmax(0, 1fr) auto; gap: 12px; padding: 22px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 20px; box-shadow: 0 30px 90px rgba(0, 0, 0, 0.55); animation: gopop 260ms var(--spring) both; }
  .dialog h2 { margin: 0; color: var(--text); font-size: 17px; font-weight: 600; letter-spacing: -0.02em; overflow-wrap: anywhere; }
  .targets { display: grid; gap: 6px; overflow-y: auto; min-height: 0; }
  .target { display: flex; align-items: center; gap: 12px; text-align: left; padding: 10px 14px; border-radius: 14px; border-color: var(--line); }
  .target .grow { display: grid; gap: 2px; min-width: 0; }
  .tn { font-weight: 500; }
  .warnv { flex: none; font-size: 11px; padding: 2px 8px; border-radius: 999px; color: #e5a03d; border: 1px solid rgba(229, 160, 61, 0.5); }
  .dfoot { display: flex; justify-content: flex-end; }
  @media (max-width: 760px) { .empty { grid-template-columns: 1fr; } }
</style>
