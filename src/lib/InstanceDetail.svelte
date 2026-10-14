<script lang="ts">
  import { api, type Instance, type JavaInstall, type World } from './api'
  import { locale, t } from './i18n.svelte'
  import { app, askConfirm, createShortcut, installedJavas, isBusy, notify, play, reloadInstances, stop } from './store.svelte'
  import { formatBytes, loaderNames, profileName, profiles } from './meta'
  import Cover from './Cover.svelte'
  import ContentTable from './ContentTable.svelte'
  import Select from './Select.svelte'
  import Sparkline from './Sparkline.svelte'
  import ServerList from './ServerList.svelte'
  import ShareModal from './ShareModal.svelte'
  import Icon from './Icon.svelte'
  import { opensWorlds } from './quickplay.svelte'

  type Tab = 'overview' | 'mods' | 'worlds' | 'servers' | 'settings'
  const tabs: { id: Tab; label: string }[] = [
    { id: 'overview', label: 'Overview' },
    { id: 'mods', label: 'Content' },
    { id: 'worlds', label: 'Worlds' },
    { id: 'servers', label: 'Servers' },
    { id: 'settings', label: 'Settings' },
  ]

  let tab: Tab = $state('overview')
  let worlds: World[] = $state([])
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
  const game = $derived(inst ? app.games[inst.id] : undefined)
  const running = $derived(game?.state === 'running')
  const busy = $derived(isBusy(inst?.id ?? null))
  const samples = $derived(inst ? (app.stats[inst.id] ?? []) : [])
  const current = $derived(running && samples.length ? samples[samples.length - 1] : null)
  const peak = $derived(samples.length ? Math.max(...samples) : null)

  $effect(() => {
    draft = inst ? { ...inst } : null
  })

  async function loadWorlds() {
    if (inst) worlds = await api.listWorlds(inst.id).catch(() => [])
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
      const copy = await api.duplicateInstance(inst!.id)
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
        <div class="num">{inst.memoryMaxMb} MB</div>
      </div>
      <div>
        <div class="eyebrow">{t('Profile')}</div>
        <div class="num sm">{profileName(inst.flagProfile)}</div>
      </div>
    </section>
    <div class="chart">
      <Sparkline values={samples} limit={inst.memoryMaxMb} />
      {#if !samples.length}<p class="muted small hint">{t('The chart fills in while the game is running.')}</p>{/if}
    </div>

  {:else if tab === 'mods'}
    <ContentTable {inst} {busy} />
  {:else if tab === 'worlds'}
    <div class="row toolbar">
      <p class="muted grow">{worlds.length ? (worlds.length === 1 ? t('1 world') : t('{n} worlds', { n: worlds.length })) : t('Worlds will appear here after your first game.')}</p>
      <button onclick={() => api.openFolder(inst.id, 'saves').catch(notify)}>{t('Open folder')}</button>
    </div>
    <div class="list">
      {#each worlds as w (w.folder)}
        <div class="item row">
          <span class="grow name">{w.name ?? w.folder}{#if w.name && w.name !== w.folder}<span class="muted small folder">{w.folder}</span>{/if}</span>
          <span class="muted small">{formatBytes(w.size)}</span>
          <span class="muted small when">{when(w.modifiedMs)}</span>
          {#if opensWorlds(inst.mcVersion)}
            <button class="wplay" onclick={() => play(inst.id, { kind: 'world', folder: w.folder })} disabled={busy || !app.accounts.active} aria-label={t('Play {name}', { name: w.name ?? w.folder })}><Icon name="play" size={12} />{t('Play')}</button>
          {/if}
        </div>
      {/each}
    </div>
  {:else if tab === 'servers'}
    <ServerList instanceId={inst.id} />
  {:else if draft}
    <form class="settings" onsubmit={(e) => { e.preventDefault(); save() }}>
      <label>{t('Name')}<input bind:value={draft.name} /></label>
      <div class="row">
        <label class="grow">{t('Max RAM (MB)')}<input type="number" min="512" step="256" bind:value={draft.memoryMaxMb} /></label>
        <label class="grow">{t('Initial RAM (MB)')}<input type="number" min="128" step="128" bind:value={draft.memoryMinMb} /></label>
      </div>
      <label>
        {t('JVM profile')}
        <Select bind:value={draft.flagProfile} options={profiles().map((p) => ({ value: p.id, label: p.label }))} label={t('JVM profile')} />
        <span class="muted">{profiles().find((p) => p.id === draft!.flagProfile)?.hint}</span>
      </label>
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
      <div class="row">
        <button class="primary" type="submit" disabled={busyAction}>{t('Save')}</button>
      </div>

      <div class="danger-zone">
        <div class="eyebrow">{t('More')}</div>
        <div class="row wrap">
          <button type="button" onclick={() => api.openFolder(inst.id, 'game').catch(notify)}>{t('Open instance folder')}</button>
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
  .swap { display: flex; flex-direction: column; gap: 22px; animation: swap 300ms var(--spring) both; }
  .swap > * { flex-shrink: 0; }
  @keyframes swap { from { opacity: 0; transform: translateY(8px); } }
  .tabs button { overflow: visible; border: 0; border-radius: 0; margin-bottom: -1px; color: var(--muted); padding: 10px 14px; background: transparent; }
  .tabs button:hover:not(:disabled) { color: var(--text); background: transparent; }
  .tabs button::after { content: ''; position: absolute; left: 10px; right: 10px; bottom: 0; height: 2px; background: var(--text); transform: scaleX(0); transition: transform 320ms var(--spring); }
  .tabs button.on { color: var(--text); }
  .tabs button.on::after { transform: none; }
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

</style>
