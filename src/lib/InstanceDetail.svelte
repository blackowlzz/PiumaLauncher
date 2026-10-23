<script lang="ts">
  import { api, modrinth, type Backup, type Instance, type JavaInstall, type PackDiff, type World } from './api'
  import { locale, stageText, t } from './i18n.svelte'
  import { app, askConfirm, cancelInstall, createShortcut, installedJavas, isBusy, notify, play, refreshEffective, refreshPackUpdate, refreshStartup, reloadInstances, seconds, stop, updateModpack } from './store.svelte'
  import { formatBytes, loaderNames, profileName, profiles } from './meta'
  import Cover from './Cover.svelte'
  import ContentTable from './ContentTable.svelte'
  import Select from './Select.svelte'
  import Sparkline from './Sparkline.svelte'
  import ServerList from './ServerList.svelte'
  import ShareModal from './ShareModal.svelte'
  import { copyText, ctx } from './contextmenu'
  import Icon from './Icon.svelte'
  import { opensWorlds } from './serverstatus.svelte'

  type Tab = 'overview' | 'mods' | 'worlds' | 'servers' | 'settings'
  const tabs: { id: Tab; label: string }[] = [
    { id: 'overview', label: 'Overview' },
    { id: 'mods', label: 'Content' },
    { id: 'worlds', label: 'Worlds' },
    { id: 'servers', label: 'Servers' },
    { id: 'settings', label: 'Settings' },
  ]

  let tab: Tab = $state('overview')
  const eff = $derived(app.detailId ? app.effective[app.detailId] : undefined)
  let worlds: World[] = $state([])
  let backups: Backup[] = $state([])
  let expanded: string | null = $state(null)
  let backingUp: string | null = $state(null)
  let restoring: string | null = $state(null)
  const backupsOf = (folder: string) => backups.filter((b) => b.world === folder)
  const orphans = $derived([...new Set(backups.map((b) => b.world))].filter((w) => !worlds.some((x) => x.folder === w)))
  const reasonLabel = (r: string) =>
    ({ manual: t('Manual'), launch: t('Before playing'), update: t('Before mod changes'), restore: t('Before restoring') })[r] ?? r
  let javas: JavaInstall[] = $state([])
  let draft: Instance | null = $state(null)
  let busyAction = $state(false)
  let linked = $state<'idle' | 'working' | 'done'>('idle')
  let sharing = $state(false)
  let linkTimer: ReturnType<typeof setTimeout> | undefined

  async function shortcut() {
    if (!inst || linked !== 'idle') return
    linked = 'working'
    const ok = await createShortcut(inst.id)
    linked = ok ? 'done' : 'idle'
    if (ok) {
      clearTimeout(linkTimer)
      linkTimer = setTimeout(() => (linked = 'idle'), 2600)
    }
  }

  const inst = $derived(app.instances.find((i) => i.id === app.detailId) ?? null)
  $effect(() => {
    if (!inst) return
    void [inst.customPerformance, inst.memoryMaxMb, inst.flagProfile, app.settings.autoOptimize, app.settings.defaultMemoryMaxMb, app.settings.defaultFlagProfile]
    refreshEffective(inst.id)
  })
  const game = $derived(inst ? app.games[inst.id] : undefined)
  const running = $derived(game?.state === 'running')
  const busy = $derived(isBusy(inst?.id ?? null))
  const samples = $derived(inst ? (app.stats[inst.id] ?? []) : [])
  const startup = $derived(inst ? app.startup[inst.id] : undefined)
  $effect(() => {
    if (inst?.id) refreshStartup(inst.id)
  })
  const current = $derived(running && samples.length ? samples[samples.length - 1] : null)
  const peak = $derived(samples.length ? Math.max(...samples) : null)

  $effect(() => {
    draft = inst ? { ...inst, launch: { ...inst.launch } } : null
  })

  const defaults = $derived(app.settings.defaultLaunch)
  const hasDefaults = $derived(!!(defaults?.wrapper.trim() || defaults?.env.trim() || defaults?.fullscreen || (defaults?.width && defaults?.height)))
  const inherited = $derived.by(() => {
    const d = draft as Instance | null
    return hasDefaults && d && !d.launch.skipDefaults ? defaults : null
  })
  const packUpdate = $derived(inst ? (app.packUpdates[inst.id] ?? null) : null)
  const updating = $derived(!!inst && app.installingId === inst.id)
  const updateStatus = $derived.by(() => {
    const p = app.install
    if (!p) return t('Updating…')
    return p.total > 0 ? `${stageText(p.stage)} · ${Math.round((p.done / p.total) * 100)}%` : stageText(p.stage)
  })
  const packKey = $derived(inst?.pack ? `${inst.id}\n${inst.pack.versionId}` : null)
  $effect(() => {
    if (packKey) refreshPackUpdate(packKey.split('\n')[0])
  })

  let diff: PackDiff | null = $state(null)
  let showDiff = $state(false)
  const diffKey = $derived(inst && packUpdate ? `${inst.id}\n${packUpdate.versionId}` : null)
  $effect(() => {
    const key = diffKey
    diff = null
    showDiff = false
    if (!key) return
    const [id, version] = key.split('\n')
    let alive = true
    modrinth
      .modpackDiff(id, version)
      .then((d) => alive && (diff = d))
      .catch(() => {})
    return () => (alive = false)
  })
  const diffSummary = $derived.by(() => {
    const d = diff as PackDiff | null
    if (!d) return ''
    return [
      d.updated.length ? t('{n} updated', { n: d.updated.length }) : '',
      d.added.length ? t('{n} added', { n: d.added.length }) : '',
      d.removed.length ? t('{n} removed', { n: d.removed.length }) : '',
    ]
      .filter(Boolean)
      .join(' · ')
  })

  async function runUpdate() {
    const u = packUpdate
    if (!inst || !u) return
    const mc = u.mcVersion && u.mcVersion !== inst.mcVersion ? u.mcVersion : null
    if (mc && !(await askConfirm(t('Update "{name}" to {version}? It also moves to Minecraft {mc}: worlds opened on the new version may not work on the old one anymore.', { name: inst.name, version: u.versionNumber, mc }), t('Update')))) return
    await updateModpack(inst.id, u.versionId)
  }

  async function loadWorlds() {
    if (!inst) return
    const [w, b] = await Promise.all([api.listWorlds(inst.id).catch(() => [] as World[]), api.listBackups(inst.id).catch(() => [] as Backup[])])
    worlds = w
    backups = b
  }

  async function backupNow(folder: string) {
    if (!inst || backingUp) return
    backingUp = folder
    try {
      await api.backupWorld(inst.id, folder)
      expanded = folder
      notify(t('Backup created'))
      await loadWorlds()
    } catch (e) {
      notify(e)
    } finally {
      backingUp = null
    }
  }

  async function restore(b: Backup, asCopy: boolean) {
    if (!inst || restoring) return
    const date = when(b.createdMs)
    const exists = worlds.some((w) => w.folder === b.world)
    if (!asCopy && exists && !(await askConfirm(t('Bring "{name}" back to {date}? The current world is backed up first, so you can undo this.', { name: b.world, date }), t('Restore')))) return
    restoring = b.id
    try {
      const folder = await api.restoreBackup(inst.id, b.world, b.id, asCopy)
      notify(asCopy ? t('Restored as "{name}"', { name: folder }) : t('"{name}" restored', { name: folder }))
      await loadWorlds()
    } catch (e) {
      notify(e)
    } finally {
      restoring = null
    }
  }

  async function removeBackup(b: Backup) {
    if (!inst || !(await askConfirm(t('Delete the backup of {date}?', { date: when(b.createdMs) })))) return
    try {
      await api.deleteBackup(inst.id, b.world, b.id)
      await loadWorlds()
    } catch (e) {
      notify(e)
    }
  }

  $effect(() => {
    void inst?.id
    if (tab === 'worlds') loadWorlds()
    if (tab === 'settings') installedJavas().then((j) => (javas = j))
  })

  async function attempt(fn: () => Promise<unknown>) {
    busyAction = true
    try {
      await fn()
    } catch (e) {
      notify(e)
    } finally {
      busyAction = false
    }
  }

  const save = () =>
    attempt(async () => {
      if (!draft) return
      await api.updateInstance({ ...draft, javaPath: draft.javaPath || null })
      await reloadInstances()
      notify(t('Settings saved'))
    })

  const duplicate = () =>
    attempt(async () => {
      const copy = await api.duplicateInstance(inst!.id, t('{name} (copy)', { name: inst!.name }))
      await reloadInstances()
      app.detailId = copy.id
      app.selectedId = copy.id
      tab = 'overview'
      notify(t('Created "{name}"', { name: copy.name }))
    })

  const remove = () =>
    attempt(async () => {
      if (!inst || !(await askConfirm(t('Delete "{name}" with all its worlds and mods?', { name: inst.name })))) return
      await api.deleteInstance(inst.id)
      app.detailId = null
      app.page = 'library'
      await reloadInstances()
    })

  const when = (ms: number) =>
    new Date(ms).toLocaleString(locale(), { day: 'numeric', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit' })
</script>

{#snippet backupList(world: string, list: Backup[], exists: boolean)}
  <div class="backups">
    {#if exists}
      <div class="row bhead">
        <span class="muted small grow">{list.length ? t('Restoring brings the world back to that moment.') : t('No backups yet.')}</span>
        <button onclick={() => backupNow(world)} disabled={running || backingUp !== null}>{backingUp === world ? t('Backing up…') : t('Back up now')}</button>
      </div>
    {/if}
    {#each list as b (b.id)}
      <div class="row brow">
        <span class="grow">{when(b.createdMs)}<span class="tag reason">{reasonLabel(b.reason)}</span></span>
        <span class="muted small">{formatBytes(b.size)}</span>
        {#if exists}<button class="ghost" onclick={() => restore(b, true)} disabled={running || restoring !== null}>{t('Restore as copy')}</button>{/if}
        <button onclick={() => restore(b, false)} disabled={running || restoring !== null}>{restoring === b.id ? t('Restoring…') : t('Restore')}</button>
        <button class="ghost iconbtn" onclick={() => removeBackup(b)} disabled={restoring !== null} aria-label={t('Delete')}><Icon name="trash" size={14} /></button>
      </div>
    {/each}
  </div>
{/snippet}

{#if !inst}
  <p class="muted">{t('Instance not found.')}</p>
  <button onclick={() => (app.page = 'library')}>{t('Back to the library')}</button>
{:else}
  <header class="head">
    <button class="ghost back" onclick={() => (app.page = 'library')}>← {t('Library')}</button>
    <div class="banner">
      <Cover {inst} big />
      <div class="shade"></div>
      <div class="bin">
        <h1>{inst.name}</h1>
        <div class="chips">
          <span class="tag">Minecraft {inst.mcVersion}</span>
          {#if inst.loader !== 'vanilla'}<span class="tag">{loaderNames[inst.loader]} {inst.loaderVersion ?? ''}</span>{/if}
          {#if inst.pack}<span class="tag">{t('Modpack {version}', { version: inst.pack.versionNumber })}</span>{/if}
          <span class="tag">{profileName(inst.flagProfile)}</span>
          {#if running}<span class="tag live">{t('Running')}</span>{/if}
        </div>
      </div>
      <div class="acts">
        <button class="stopbtn sharebtn" onclick={() => (sharing = true)}><Icon name="share" size={15} />{t('Share')}</button>
        {#if app.settings.liveConsole}
          <button class="stopbtn" onclick={() => api.openConsole(inst.id).catch(notify)}>{t('Console')}</button>
        {/if}
        {#if running}
          <button class="stopbtn" onclick={() => stop(inst.id)}>{t('Stop')}</button>
        {/if}
      </div>
    </div>
    <div class="tabs" role="tablist" aria-label={t('Instance sections')}>
      {#each tabs as tb}
        <button role="tab" aria-selected={tab === tb.id} class:on={tab === tb.id} onclick={() => (tab = tb.id)}>{t(tb.label)}</button>
      {/each}
    </div>
  </header>

  {#if sharing}<ShareModal {inst} onClose={() => (sharing = false)} />{/if}

  {#if packUpdate || updating}
    <section class="upd" aria-live="polite">
      <span class="uic"><Icon name="download" size={18} /></span>
      <div class="grow">
        <div class="ut">{t('Update available')}{#if packUpdate}<span class="muted">{` · ${inst.pack?.versionNumber} → ${packUpdate.versionNumber}`}</span>{/if}</div>
        <div class="muted small">
          {#if updating}{updateStatus}{:else if packUpdate?.mcVersion && packUpdate.mcVersion !== inst.mcVersion}{t('This version moves to Minecraft {mc}.', { mc: packUpdate.mcVersion })}{:else}{t('Your worlds, the settings you changed and the mods you added stay as they are.')}{/if}
        </div>
      </div>
      {#if diffSummary && !updating}
        <button class="ghost difft" onclick={() => (showDiff = !showDiff)} aria-expanded={showDiff}>{diffSummary} · {showDiff ? t('Hide changes') : t('Show changes')}</button>
      {/if}
      {#if updating}
        <button onclick={cancelInstall} disabled={app.cancelling}>{app.cancelling ? t('Cancelling…') : t('Cancel')}</button>
      {:else}
        <button class="primary" onclick={runUpdate} disabled={running || app.installingId !== null}>{t('Update')}</button>
      {/if}
    </section>
    {#if showDiff && diff && !updating}
      <div class="diff">
        {#if diff.mcTo}<div class="dmc"><Icon name="box" size={15} />Minecraft {diff.mcFrom} → {diff.mcTo}</div>{/if}
        {#each [{ kind: 'updated', label: t('Updated'), list: diff.updated, mark: '↑' }, { kind: 'added', label: t('Added'), list: diff.added, mark: '+' }, { kind: 'removed', label: t('Removed'), list: diff.removed, mark: '−' }] as group}
          {#if group.list.length}
            <section class="dgroup {group.kind}">
              <header><span class="dbadge" aria-hidden="true">{group.mark}</span>{group.label}<span class="dcount">{group.list.length}</span></header>
              <ul>
                {#each group.list as c}
                  <li title={c.name}>
                    <span class="dname">{c.name}</span>
                    <span class="dver">{#if c.from && c.to}<span class="dold">{c.from}</span> → {c.to}{:else}{c.to ?? c.from}{/if}</span>
                  </li>
                {/each}
              </ul>
            </section>
          {/if}
        {/each}
      </div>
    {/if}
  {/if}

  {#key tab}
  <div class="swap">
  {#if tab === 'overview'}
    <section class="stats">
      <div>
        <div class="eyebrow">{t('Memory now')}</div>
        <div class="num">{current !== null ? `${current} MB` : '—'}</div>
      </div>
      <div>
        <div class="eyebrow">{t('Peak')}</div>
        <div class="num">{peak !== null ? `${peak} MB` : '—'}</div>
      </div>
      <div>
        <div class="eyebrow">{t('Limit')}</div>
        <div class="num">{eff?.memoryMaxMb ?? inst.memoryMaxMb} MB</div>
      </div>
      <div>
        <div class="eyebrow">{t('Profile')}</div>
        <div class="num sm">{profileName(eff?.flagProfile ?? inst.flagProfile)}</div>
      </div>
      <div>
        <div class="eyebrow">{t('Startup')}</div>
        <div class="num">{startup?.lastMs ? seconds(startup.lastMs) : '—'}</div>
        {#if startup?.usualMs && startup.lastMs && Math.abs(startup.usualMs - startup.lastMs) >= 2000}<div class="muted small">{t('usually {time}', { time: seconds(startup.usualMs) })}</div>{/if}
      </div>
    </section>
    {#if startup?.slower}
      <div class="slow">
        <Icon name="clock" size={18} />
        <div class="grow">
          <div class="st">{t('It now takes {now} to start, it used to take {before}.', { now: seconds(startup.slower.nowMs), before: seconds(startup.slower.beforeMs) })}</div>
          <div class="muted small">
            {#if startup.slower.added.length}{t('Added since then: {mods}', { mods: startup.slower.added.join(', ') })}{:else}{t('No mods were added in the meantime, so the cause is something else (an update, resource packs or the system).')}{/if}
          </div>
        </div>
        <button class="ghost" onclick={() => (tab = 'mods')}>{t('Content')}</button>
      </div>
    {:else if !startup?.lastMs}
      <p class="muted small">{t('The launcher measures how long the game takes to reach the main menu, and tells you if it gets slower.')}</p>
    {/if}
    <div class="chart">
      <Sparkline values={samples} limit={eff?.memoryMaxMb ?? inst.memoryMaxMb} />
      {#if !samples.length}<p class="muted small hint">{t('The chart fills in while the game is running.')}</p>{/if}
    </div>

  {:else if tab === 'mods'}
    <ContentTable {inst} {busy} />
  {:else if tab === 'worlds'}
    <div class="row toolbar">
      <p class="muted grow">{worlds.length ? (worlds.length === 1 ? t('1 world') : t('{n} worlds', { n: worlds.length })) : t('Worlds will appear here after your first game.')}</p>
      {#if backups.length}<button class="ghost" onclick={() => api.openBackups(inst.id).catch(notify)}>{t('Backups folder')}</button>{/if}
      <button onclick={() => api.openFolder(inst.id, 'saves').catch(notify)}>{t('Open folder')}</button>
    </div>
    <div class="list">
      {#each worlds as w (w.folder)}
        {@const own = backupsOf(w.folder)}
        <div class="item wrow">
        <div class="row" use:ctx={() => [
          { label: t('Play'), icon: 'play', onclick: () => void play(inst.id, { kind: 'world', folder: w.folder }), disabled: busy || !app.accounts.active || !opensWorlds(inst.mcVersion) },
          { label: t('Back up now'), icon: 'clock', onclick: () => void backupNow(w.folder), disabled: running || backingUp !== null },
          { label: t('Copy name'), icon: 'copy', onclick: () => void copyText(w.name ?? w.folder).then((ok) => ok && notify(t('Copied'))) },
          { label: t('Open instance folder'), icon: 'folder', onclick: () => void api.openFolder(inst.id, 'saves').catch(notify) },
        ]}>
          <span class="grow name">{w.name ?? w.folder}{#if w.name && w.name !== w.folder}<span class="muted small folder">{w.folder}</span>{/if}</span>
          <span class="muted small">{formatBytes(w.size)}</span>
          <span class="muted small when">{when(w.modifiedMs)}</span>
          <button class="ghost bk" class:on={expanded === w.folder} onclick={() => (expanded = expanded === w.folder ? null : w.folder)} aria-expanded={expanded === w.folder}>
            <Icon name="clock" size={13} />{own.length === 1 ? t('1 backup') : t('{n} backups', { n: own.length })}
          </button>
          {#if opensWorlds(inst.mcVersion)}
            <button class="wplay" onclick={() => play(inst.id, { kind: 'world', folder: w.folder })} disabled={busy || !app.accounts.active} aria-label={t('Play {name}', { name: w.name ?? w.folder })}><Icon name="play" size={12} />{t('Play')}</button>
          {/if}
        </div>
        {#if expanded === w.folder}
          {@render backupList(w.folder, own, true)}
        {/if}
        </div>
      {/each}
      {#each orphans as world (world)}
        <div class="item wrow">
          <div class="row">
            <span class="grow name">{world}<span class="muted small folder">{t('Deleted world')}</span></span>
            <button class="ghost bk" class:on={expanded === world} onclick={() => (expanded = expanded === world ? null : world)} aria-expanded={expanded === world}>
              <Icon name="clock" size={13} />{backupsOf(world).length === 1 ? t('1 backup') : t('{n} backups', { n: backupsOf(world).length })}
            </button>
          </div>
          {#if expanded === world}
            {@render backupList(world, backupsOf(world), false)}
          {/if}
        </div>
      {/each}
    </div>
  {:else if tab === 'servers'}
    <ServerList instanceId={inst.id} />
  {:else if draft}
    <form class="settings" onsubmit={(e) => { e.preventDefault(); save() }}>
      <label>{t('Name')}<input bind:value={draft.name} /></label>
      <div class="perf">
        <label class="own">
          <span>
            <strong>{t('Custom memory and profile for this instance')}</strong>
            <span class="muted">{draft.customPerformance ? t('This instance uses its own values below.') : t('This instance uses the values from Performance: {mem} MB · {profile}.', { mem: app.settings.defaultMemoryMaxMb, profile: profileName(app.settings.defaultFlagProfile) })}</span>
          </span>
          <span class="switch">
            <input type="checkbox" bind:checked={draft.customPerformance} aria-label={t('Custom memory and profile for this instance')} />
            <span class="track"></span>
          </span>
        </label>
        {#if app.settings.autoOptimize}<span class="muted">{t('Automatic optimization is on, so the launcher raises these values at each launch if your computer needs it.')}</span>{/if}
        {#if draft.customPerformance}
          <div class="row">
            <label class="grow">{t('Max RAM (MB)')}<input type="number" min="512" step="256" bind:value={draft.memoryMaxMb} /></label>
            <label class="grow">{t('Initial RAM (MB)')}<input type="number" min="128" step="128" bind:value={draft.memoryMinMb} /></label>
          </div>
          <label>
            {t('JVM profile')}
            <Select bind:value={draft.flagProfile} options={profiles().map((p) => ({ value: p.id, label: p.label }))} label={t('JVM profile')} />
            <span class="muted">{profiles().find((p) => p.id === draft!.flagProfile)?.hint}</span>
          </label>
        {/if}
      </div>
      <label>
        Java
        <Select
          bind:value={draft.javaPath}
          label="Java"
          options={[
            { value: null, label: t('Automatic (the launcher downloads GraalVM/Temurin)') },
            ...javas.map((j) => ({ value: j.path, label: `Java ${j.major} · ${j.isGraal ? 'GraalVM' : 'Temurin/OpenJDK'}`, hint: j.path })),
          ]}
        />
      </label>
      <label>{t('Extra JVM arguments')}<input bind:value={draft.extraJvmArgs} placeholder="-Dprop=value" /></label>
      <details class="adv" open={!!(draft.launch.wrapper || draft.launch.env || draft.launch.width || draft.launch.fullscreen || draft.launch.skipDefaults)}>
        <summary>{t('Launch options')}</summary>
        <div class="advbody">
          {#if hasDefaults || draft.launch.skipDefaults}
            <label class="own">
              <span>
                <strong>{t('Use the default launch options')}</strong>
                <span class="muted">{t('Fields you leave empty here take the values from {page}.', { page: t('Performance') })}</span>
              </span>
              <span class="switch">
                <input type="checkbox" checked={!draft.launch.skipDefaults} onchange={(e) => (draft!.launch.skipDefaults = !e.currentTarget.checked)} aria-label={t('Use the default launch options')} />
                <span class="track"></span>
              </span>
            </label>
          {/if}
          <label class="own">
            <span>
              <strong>{t('Start in fullscreen')}</strong>
              {#if inherited?.fullscreen && !draft.launch.fullscreen && !(draft.launch.width && draft.launch.height)}<span class="inh">{t('Default: {value}', { value: t('Start in fullscreen') })}</span>{/if}
              <span class="muted">{t('Otherwise the game window opens at the size below, or at the default size if you leave it empty.')}</span>
            </span>
            <span class="switch">
              <input type="checkbox" bind:checked={draft.launch.fullscreen} aria-label={t('Start in fullscreen')} />
              <span class="track"></span>
            </span>
          </label>
          <div class="row">
            <label class="grow">{t('Window width')}<input type="number" min="320" max="16384" step="1" placeholder={String(inherited?.width ?? 854)} bind:value={draft.launch.width} disabled={draft.launch.fullscreen} /></label>
            <label class="grow">{t('Window height')}<input type="number" min="320" max="16384" step="1" placeholder={String(inherited?.height ?? 480)} bind:value={draft.launch.height} disabled={draft.launch.fullscreen} /></label>
          </div>
          <label>
            {t('Launch wrapper')}
            <input bind:value={draft.launch.wrapper} placeholder="gamemoderun mangohud" spellcheck="false" />
            <span class="muted">{t('A command that starts the game for you, such as gamemoderun, mangohud or prime-run. Leave it empty to start Java directly.')}</span>
            {#if inherited?.wrapper.trim() && !draft.launch.wrapper.trim()}<span class="inh">{t('Default: {value}', { value: inherited.wrapper })}</span>{/if}
          </label>
          <label>
            {t('Environment variables')}
            <textarea bind:value={draft.launch.env} rows="3" placeholder="MESA_GL_VERSION_OVERRIDE=4.6" spellcheck="false"></textarea>
            <span class="muted">{t('One per line, as NAME=value.')}</span>
            {#if inherited?.env.trim()}<span class="inh">{t('The default variables come first; yours replace them when the name is the same.')}</span>{/if}
          </label>
        </div>
      </details>
      <div class="row">
        <button class="primary" type="submit" disabled={busyAction}>{t('Save')}</button>
      </div>

      <div class="danger-zone">
        <div class="eyebrow">{t('More')}</div>
        <div class="row wrap">
          <button type="button" onclick={() => api.openFolder(inst.id, 'game').catch(notify)}>{t('Open instance folder')}</button>
          <button type="button" onclick={() => api.openFolder(inst.id, 'screenshots').catch(notify)}>{t('Screenshots')}</button>
          <button type="button" class="linkbtn" class:done={linked === 'done'} onclick={shortcut} disabled={linked === 'working'} aria-live="polite">
            {#key linked}
              <span class="lk">
                {#if linked === 'done'}<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12.5 4.5 4.5L19 7.5" pathLength="1" /></svg>{t('Shortcut created')}{:else}<Icon name="shortcut" size={15} />{t('Create shortcut')}{/if}
              </span>
            {/key}
          </button>
          <button type="button" onclick={duplicate} disabled={busyAction || busy}>{t('Duplicate')}</button>
          <button type="button" class="ghost" onclick={remove} disabled={busyAction || busy}>{t('Delete instance')}</button>
        </div>
      </div>
    </form>
  {/if}
  </div>
  {/key}
{/if}

<style>
  .head { display: grid; gap: 14px; }
  .back { justify-self: start; padding-left: 0; }
  .banner { position: relative; overflow: hidden; border-radius: 18px; border: 1px solid var(--line); min-height: 150px; display: flex; align-items: flex-end; justify-content: space-between; gap: 16px; padding: 22px 24px; }
  .shade { position: absolute; inset: 0; background: linear-gradient(90deg, rgba(0,0,0,.8), rgba(0,0,0,.2)); }
  .banner { color: #fff; }
  .bin { position: relative; display: grid; gap: 10px; }
  .banner h1 { font-size: 34px; letter-spacing: -0.04em; }
  .chips { display: flex; flex-wrap: wrap; gap: 8px; }
  .tag { background: rgba(0,0,0,.5); color: #fff; border-color: rgba(255,255,255,.3); }
  .live { border-color: #fff; }
  .stopbtn { position: relative; }
  .acts { position: relative; display: flex; gap: 8px; }
  .tabs { display: flex; gap: 4px; border-bottom: 1px solid var(--line); }
  .upd { display: flex; align-items: center; gap: 14px; padding: 14px 16px; border: 1px solid var(--line-strong); border-radius: var(--radius); background: var(--surface); animation: swapin 320ms var(--spring) backwards; }
  .uic { display: grid; place-items: center; width: 38px; height: 38px; flex: none; border-radius: 50%; background: var(--inv-bg); color: var(--inv-text); }
  .ut { font-weight: 600; margin-bottom: 2px; }
  .difft { flex: none; font-size: 13px; padding: 7px 12px; }
  .diff { display: grid; gap: 18px; margin-top: -8px; padding: 16px; border: 1px solid var(--line); border-radius: var(--radius); background: var(--surface); max-height: 420px; overflow-y: auto; animation: swapin 260ms var(--spring) backwards; }
  .dmc { display: inline-flex; align-items: center; gap: 8px; justify-self: start; padding: 6px 12px; border-radius: 999px; background: var(--surface-2); border: 1px solid var(--line-strong); font-size: 13px; font-weight: 600; }
  .dgroup header { display: flex; align-items: center; gap: 8px; margin-bottom: 10px; font-size: 13px; font-weight: 600; }
  .dbadge { display: grid; place-items: center; width: 20px; height: 20px; border-radius: 6px; background: var(--surface-2); color: var(--text); font-size: 13px; line-height: 1; }
  .added .dbadge { background: rgba(62, 207, 110, 0.16); color: #3ecf6e; }
  .removed .dbadge { background: rgba(229, 72, 77, 0.16); color: #e5484d; }
  .dcount { color: var(--muted); font-weight: 500; }
  .dgroup ul { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 8px; }
  .dgroup li { display: grid; gap: 3px; min-width: 0; padding: 10px 12px; border-radius: 10px; background: var(--surface-2); border: 1px solid var(--line); }
  .dname { font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .removed .dname { color: var(--muted); }
  .dver { font-size: 12px; color: var(--text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dold, .removed .dver, .added .dver { color: var(--muted); }
  .swap { display: flex; flex-direction: column; gap: 22px; animation: swap 300ms var(--spring) both; }
  .swap > * { flex-shrink: 0; }
  @keyframes swap { from { opacity: 0; transform: translateY(8px); } }
  .tabs button { overflow: visible; border: 0; border-radius: 0; margin-bottom: -1px; color: var(--muted); padding: 10px 14px; background: transparent; }
  .tabs button:hover:not(:disabled) { color: var(--text); background: transparent; }
  .tabs button::after { content: ''; position: absolute; left: 10px; right: 10px; bottom: 0; height: 2px; background: var(--text); transform: scaleX(0); transition: transform 320ms var(--spring); }
  .tabs button.on { color: var(--text); }
  .tabs button.on::after { transform: none; }
  .slow { display: flex; align-items: center; gap: 14px; padding: 12px 16px; border: 1px solid var(--line-strong); border-radius: var(--radius); background: var(--surface-2); }
  .slow .st { font-weight: 600; }
  .stats { display: grid; grid-template-columns: repeat(auto-fit, minmax(130px, 1fr)); gap: 24px; }
  .num { font-size: 26px; font-weight: 600; letter-spacing: -0.03em; margin-top: 4px; }
  .num.sm { font-size: 18px; line-height: 1.65; }
  .chart { position: relative; border: 1px solid var(--line); border-radius: var(--radius); padding: 12px; background: var(--surface); }
  .hint { position: absolute; inset: 0; display: grid; place-items: center; }
  .console { border-top: 1px solid var(--line); padding-top: 12px; }
  .toolbar { gap: 8px; }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .when { min-width: 150px; text-align: right; }
  .folder { margin-left: 8px; }
  .wrow { display: grid; gap: 10px; }
  .bk { display: inline-flex; align-items: center; gap: 6px; padding: 5px 12px; font-size: 13px; color: var(--muted); }
  .bk.on { color: var(--text); background: var(--surface-2); }
  .backups { display: grid; gap: 2px; padding: 10px 14px; border-radius: 14px; background: var(--surface-2); animation: swapin 260ms var(--spring) backwards; }
  .bhead { padding-bottom: 6px; }
  .brow { gap: 8px; padding: 6px 0; border-top: 1px solid var(--line); font-size: 14px; }
  .reason { margin-left: 10px; }
  .wplay { display: inline-flex; align-items: center; gap: 6px; padding: 5px 14px; font-size: 13px; opacity: 0; transform: translateX(6px); transition: opacity 200ms ease, transform 300ms var(--spring), background var(--ease), border-color var(--ease); }
  .item:hover .wplay, .wplay:focus-visible { opacity: 1; transform: none; }
  @media (hover: none) { .wplay { opacity: 1; transform: none; } }
  .settings { display: grid; gap: 16px; max-width: 560px; }
  .danger-zone { display: grid; gap: 10px; border-top: 1px solid var(--line); padding-top: 16px; margin-top: 8px; }
  .wrap { flex-wrap: wrap; }
  .sharebtn { display: inline-flex; align-items: center; gap: 8px; }
  .linkbtn { transition: background var(--ease), border-color var(--ease), color var(--ease), transform 120ms var(--spring); }
  .linkbtn.done { background: var(--inv-bg); color: var(--inv-text); border-color: var(--inv-bg); }
  .linkbtn.done:hover:not(:disabled) { background: var(--inv-bg); border-color: var(--inv-bg); }
  .linkbtn:disabled .lk { animation: breathe 900ms ease-in-out infinite; }
  .lk { display: inline-flex; align-items: center; gap: 8px; animation: swapin 320ms var(--spring) backwards; }
  .lk svg { width: 15px; height: 15px; fill: none; stroke: currentColor; stroke-width: 2.2; stroke-linecap: round; stroke-linejoin: round; }
  .lk path { stroke-dasharray: 1; stroke-dashoffset: 1; animation: tick 420ms var(--spring) 80ms forwards; }
  .linkbtn.done { animation: ripple 700ms ease-out; }
  @keyframes tick { to { stroke-dashoffset: 0; } }
  @keyframes ripple { from { outline: 2px solid rgba(var(--glow), 0.5); outline-offset: 0; } to { outline: 2px solid rgba(var(--glow), 0); outline-offset: 9px; } }
  @keyframes breathe { 50% { opacity: 0.45; } }
  :global(:root[data-reduce]) .lk path { stroke-dashoffset: 0; }
  @media (prefers-reduced-motion: reduce) { .lk path { stroke-dashoffset: 0; } }

  .perf { display: grid; gap: 14px; }
  .own { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
  .adv { border: 1px solid var(--line); border-radius: 14px; padding: 4px 14px; }
  .adv summary { cursor: pointer; padding: 10px 0; color: var(--text); font-weight: 600; }
  .advbody { display: grid; gap: 14px; padding: 4px 0 14px; }
  .inh { color: var(--text); font-size: 12px; font-weight: 500; }
  .adv textarea { resize: vertical; font: 12.5px/1.5 ui-monospace, 'JetBrains Mono', monospace; letter-spacing: 0; }
  .own > span:first-child { display: grid; gap: 2px; }
</style>
