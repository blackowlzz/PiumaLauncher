<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { fly } from 'svelte/transition'
  import { app, handleCli, init, installContentInto, installProject, notify, saveSettings, type Page } from './lib/store.svelte'
  import { api, inTauri, uiScales, type PackVersion } from './lib/api'
  import { t } from './lib/i18n.svelte'
  import LanguageMenu from './lib/LanguageMenu.svelte'
  import Welcome from './lib/Welcome.svelte'
  import ProjectDetail from './lib/ProjectDetail.svelte'
  import InstallTargetDialog from './lib/InstallTargetDialog.svelte'
  import Icon from './lib/Icon.svelte'
  import Home from './lib/Home.svelte'
  import Library from './lib/Library.svelte'
  import InstanceDetail from './lib/InstanceDetail.svelte'
  import PlayBar from './lib/PlayBar.svelte'
  import Accounts from './lib/Accounts.svelte'
  import Explore from './lib/Explore.svelte'
  import QuickPlay from './lib/QuickPlay.svelte'
  import Settings from './lib/Settings.svelte'
  import Performance from './lib/Performance.svelte'
  import ConfirmDialog from './lib/ConfirmDialog.svelte'
  import CrashDialog from './lib/CrashDialog.svelte'
  import InstanceChooser from './lib/InstanceChooser.svelte'
  import DropZone from './lib/DropZone.svelte'
  import ContextMenu from './lib/ContextMenu.svelte'
  import ShareModal from './lib/ShareModal.svelte'
  import Wardrobe from './lib/Wardrobe.svelte'
  import Stats from './lib/Stats.svelte'
  import { isCustomized, resetTheme } from './lib/palette.svelte'
  import UpdateBanner from './lib/UpdateBanner.svelte'
  import NoticeBanner from './lib/NoticeBanner.svelte'
  import JavaUpdateBanner from './lib/JavaUpdateBanner.svelte'
  import TitleBar from './lib/TitleBar.svelte'
  import { maybeBench } from './lib/bench'
  import { watchFades } from './lib/fades'
  import { smoothScroll } from './lib/smoothscroll'

  type Tab = { id: Page; label: string; icon: 'home' | 'grid' | 'feather' | 'play' | 'compass' | 'user' | 'settings' | 'cpu' | 'chart' }
  const groups: Tab[][] = [
    [
      { id: 'home', label: 'Home', icon: 'home' },
      { id: 'servers', label: 'Quick Play', icon: 'play' },
      { id: 'library', label: 'Library', icon: 'grid' },
      { id: 'explore', label: 'Explore', icon: 'compass' },
    ],
    [
      { id: 'accounts', label: 'Account', icon: 'user' },
      { id: 'wardrobe', label: 'Wardrobe', icon: 'feather' },
      { id: 'stats', label: 'Statistics', icon: 'chart' },
    ],
    [
      { id: 'performance', label: 'Performance', icon: 'cpu' },
      { id: 'settings', label: 'Settings', icon: 'settings' },
    ],
  ]
  const order: Page[] = groups.flatMap((g) => g.map((x) => x.id)).slice(0, 9)
  const section = $derived(app.page === 'instance' ? 'library' : app.page)

  let tabsEl = $state<HTMLElement>()
  let ind = $state<{ x: number; y: number; w: number } | null>(null)
  let glide = $state(false)
  function place() {
    const el = tabsEl?.querySelector<HTMLElement>('.tab.active')
    ind = el ? { x: el.offsetLeft, y: el.offsetTop, w: el.offsetWidth } : null
  }
  $effect(() => {
    void section
    tick().then(place)
  })
  $effect(() => {
    if (!tabsEl) return
    const ro = new ResizeObserver(place)
    ro.observe(tabsEl)
    return () => ro.disconnect()
  })

  const q = new URLSearchParams(location.search).get('theme')
  const forced = q === 'light' || q === 'dark' ? q : null
  const scheme = matchMedia('(prefers-color-scheme: light)')
  let systemDark = $state(!scheme.matches)
  const theme = $derived(forced ?? app.settings.theme)
  const dark = $derived(theme === 'dark' || (theme === 'system' && systemDark))

  $effect(() => {
    document.documentElement.toggleAttribute('data-reduce', app.settings.reduceEffects)
  })

  $effect(() => {
    if (!app.loaded) return
    if (theme === 'system') delete document.documentElement.dataset.theme
    else document.documentElement.dataset.theme = theme
    try {
      localStorage.setItem('piuma-mode', app.settings.theme)
    } catch {}
  })

  function hideDiscord() {
    app.settings.hideDiscordCard = true
    saveSettings()
  }

  function toggleTheme() {
    if (isCustomized()) resetTheme()
    app.settings.theme = dark ? 'light' : 'dark'
    saveSettings()
  }

  let zoom = 1
  let fitting = false
  let refit = false
  let settle: ReturnType<typeof setTimeout> | undefined
  let edgeUp = $state(false)
  let edgeDown = $state(false)
  function measureEdges(m: HTMLElement) {
    edgeUp = m.scrollTop > 2
    edgeDown = m.scrollTop + m.clientHeight < m.scrollHeight - 2
  }
  function edges(m: HTMLElement) {
    const ro = new ResizeObserver(() => measureEdges(m))
    const watch = () => {
      ro.disconnect()
      ro.observe(m)
      for (const c of m.children) ro.observe(c)
    }
    const mo = new MutationObserver(watch)
    mo.observe(m, { childList: true })
    watch()
    return { destroy: () => (ro.disconnect(), mo.disconnect()) }
  }

  function relayout() {
    const body = document.querySelector<HTMLElement>('.body')
    if (!body) return
    const scrollers = [...body.querySelectorAll<HTMLElement>('main, nav')].map((el) => [el, el.scrollTop] as const)
    const running = new Set(document.getAnimations())
    body.style.display = 'none'
    void body.offsetHeight
    body.style.removeProperty('display')
    for (const [el, top] of scrollers) el.scrollTop = top
    for (const a of document.getAnimations()) {
      if (!running.has(a) && a.effect?.getTiming().iterations !== Infinity) a.finish()
    }
  }
  async function fitScale() {
    if (!inTauri) return
    if (fitting) {
      refit = true
      return
    }
    fitting = true
    let changed = false
    try {
      do {
        refit = false
        const [{ getCurrentWindow }, { getCurrentWebview }] = await Promise.all([import('@tauri-apps/api/window'), import('@tauri-apps/api/webview')])
        const win = getCurrentWindow()
        const [size, factor] = await Promise.all([win.innerSize(), win.scaleFactor()])
        const physical = size.width / factor
        const s = app.settings.uiScale
        const z = s === 'auto' ? (physical < 1700 ? 1 : physical < 2200 ? 1.15 : physical < 2900 ? 1.3 : 1.5) : Number(s) / 100
        if (z !== zoom) {
          zoom = z
          await getCurrentWebview().setZoom(z)
          changed = true
        }
      } while (refit)
    } finally {
      fitting = false
    }
    if (changed) requestAnimationFrame(relayout)
  }
  const onResize = () => {
    void fitScale()
    clearTimeout(settle)
    settle = setTimeout(() => void fitScale().then(() => requestAnimationFrame(relayout)), 250)
  }
  $effect(() => {
    void app.settings.uiScale
    void fitScale()
  })

  function stepZoom(dir: number) {
    if (dir === 0) {
      app.settings.uiScale = 'auto'
      notify(`Zoom: ${t('Automatic')}`)
    } else {
      const current = Math.round(zoom * 100)
      const steps = dir > 0 ? uiScales.filter((s) => Number(s) > current) : uiScales.filter((s) => Number(s) < current).reverse()
      if (!steps.length) return
      app.settings.uiScale = steps[0]
      notify(`Zoom: ${steps[0]}%`)
    }
    saveSettings()
  }

  function nativeMenu(e: MouseEvent) {
    if (import.meta.env.DEV) return
    const el = e.target as HTMLElement | null
    const editable = !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable)
    if (!editable && !window.getSelection()?.toString()) e.preventDefault()
  }

  async function shortcuts(e: KeyboardEvent) {
    const el = e.target as HTMLElement | null
    const typing = !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable)
    const mod = e.ctrlKey || e.metaKey
    if (mod && !e.altKey && ['+', '=', '-', '0'].includes(e.key)) {
      e.preventDefault()
      stepZoom(e.key === '0' ? 0 : e.key === '-' ? -1 : 1)
      return
    }
    if (document.querySelector('[aria-modal="true"]')) return
    if ((mod && e.key.toLowerCase() === 'k') || (e.key === '/' && !typing && !mod)) {
      e.preventDefault()
      app.page = 'explore'
      await tick()
      document.querySelector<HTMLInputElement>('main input[type="search"]')?.focus()
    } else if (mod && /^[1-9]$/.test(e.key)) {
      e.preventDefault()
      app.page = order[Number(e.key) - 1]
    }
  }

  async function installFromDetail(version?: PackVersion) {
    const d = app.detail
    if (d && (await installProject(d.hit, d.type, version))) app.detail = null
  }

  function pickInstances(instanceIds: string[]) {
    const p = app.picker
    app.picker = null
    if (p) void installContentInto(instanceIds, p.hit, p.type, p.version?.id)
  }

  onMount(() => {
    const root = document.documentElement
    const idle = () => root.toggleAttribute('data-idle', document.hidden || !document.hasFocus())
    idle()
    document.addEventListener('visibilitychange', idle)
    addEventListener('focus', idle)
    addEventListener('blur', idle)
    const onScheme = () => (systemDark = !scheme.matches)
    scheme.addEventListener('change', onScheme)
    addEventListener('resize', onResize)
    requestAnimationFrame(() => (glide = true))
    init().then(maybeBench).then(handleCli).catch((e) => notify(t('Startup error: {error}', { error: e })))
    const stopFades = watchFades()
    return () => {
      stopFades()
      document.removeEventListener('visibilitychange', idle)
      removeEventListener('focus', idle)
      removeEventListener('blur', idle)
      scheme.removeEventListener('change', onScheme)
      removeEventListener('resize', onResize)
      clearTimeout(settle)
    }
  })
</script>

<svelte:window onkeydown={shortcuts} oncontextmenu={nativeMenu} />

<ContextMenu />
{#if app.sharingId}
  {@const shared = app.instances.find((i) => i.id === app.sharingId)}
  {#if shared}<ShareModal inst={shared} onClose={() => (app.sharingId = null)} />{/if}
{/if}
{#if app.inviting}
  {@const invited = app.instances.find((i) => i.id === app.inviting?.instanceId)}
  {#if invited}<ShareModal inst={invited} server={{ name: app.inviting.name, address: app.inviting.address }} onClose={() => (app.inviting = null)} />{/if}
{/if}

<div class="shell">
  <TitleBar />
  <div class="body">
  <nav aria-label={t('Sections')}>
    <div class="tabs" bind:this={tabsEl}>
      {#if ind}
        <span class="ind" class:glide aria-hidden="true" style:width="{ind.w}px" style:transform="translate({ind.x}px, {ind.y}px)"></span>
      {/if}
      {#each groups as g}
        <div class="group">
          {#each g as tb}
            <button class="tab" class:active={section === tb.id} aria-current={section === tb.id ? 'page' : undefined} onclick={() => (app.page = tb.id)}>
              <Icon name={tb.icon} />{t(tb.label)}
            </button>
          {/each}
        </div>
      {/each}
    </div>
    <div class="foot">
      {#if !app.settings.hideDiscordCard}
        <div class="community">
          <div class="ch">
            <span class="dc"><Icon name="discord" size={20} /></span>
            <span class="ct">Discord</span>
            <button class="ghost hide" onclick={hideDiscord} aria-label={t('Close')} title={t('Close')}><Icon name="x" size={14} /></button>
          </div>
          <span class="cs">{t('News, help and community')}</span>
          <button class="primary join" onclick={() => api.openUrl('https://discord.gg/nNQHjEVYzF').catch(notify)}>{t('Join')}</button>
        </div>
      {/if}
      <div class="quickrow">
        <button class="modebtn" onclick={toggleTheme} aria-pressed={dark} aria-label={t('Dark mode')} title={t('Dark mode')}><Icon name={dark ? 'moon' : 'sun'} size={17} /></button>
        <div class="grow"><LanguageMenu /></div>
      </div>
    </div>
  </nav>

  <div class="content">
    <NoticeBanner />
    <UpdateBanner />
    <JavaUpdateBanner />
    <div class="scrollwrap">
    <main use:smoothScroll={app.settings.smoothScroll && !app.settings.reduceEffects} use:edges onscroll={(e) => measureEdges(e.currentTarget)}>
      {#key `${app.page}:${app.detailId ?? ''}`}
        <div class="page">
          {#if app.page === 'home'}
            <Home />
          {:else if app.page === 'library'}
            <Library />
          {:else if app.page === 'instance'}
            <InstanceDetail />
          {:else if app.page === 'servers'}
            <QuickPlay />
          {:else if app.page === 'accounts'}
            <Accounts />
          {:else if app.page === 'wardrobe'}
            <Wardrobe />
          {:else if app.page === 'performance'}
            <Performance />
          {:else if app.page === 'settings'}
            <Settings />
          {:else if app.page === 'stats'}
            <Stats />
          {:else}
            <Explore />
          {/if}
        </div>
      {/key}
    </main>
    <div class="edge top" class:on={edgeUp} aria-hidden="true"></div>
    <div class="edge bottom" class:on={edgeDown} aria-hidden="true"></div>
    </div>
  </div>
  <PlayBar />
  </div>

  {#if app.toast}
    <div class="toastwrap"><div class="toast" role="alert" transition:fly={{ y: 16, duration: 260 }}>{app.toast}</div></div>
  {/if}
</div>

{#if app.detail}
  {#key app.detail.hit.projectId}
    <ProjectDetail hit={app.detail.hit} type={app.detail.type} busy={app.installingId === app.detail.hit.projectId} disabled={app.installingId !== null} onInstall={installFromDetail} onClose={() => (app.detail = null)} />
  {/key}
{/if}

{#if app.picker}
  {#key app.picker.hit.projectId}
    <InstallTargetDialog hit={app.picker.hit} type={app.picker.type} version={app.picker.version} onConfirm={pickInstances} onClose={() => (app.picker = null)} />
  {/key}
{/if}

{#if app.joining}
  {#key app.joining.address}<InstanceChooser server={app.joining} onClose={() => (app.joining = null)} />{/key}
{/if}
<CrashDialog />
<DropZone />
<ConfirmDialog />
<Welcome />

<style>
  .shell { display: grid; grid-template-rows: auto 1fr; height: 100%; }
  .body { grid-template-rows: minmax(0, 1fr) auto; }
  .body { display: grid; grid-template-columns: 216px minmax(0, 1fr); min-height: 0; }
  nav { grid-row: 1; }
  .content { grid-row: 1; }
  .body > :global(.bar) { grid-column: 1 / -1; grid-row: 2; }
  nav { display: flex; flex-direction: column; gap: 4px; padding: 16px 12px 14px; min-height: 0; overflow-y: auto; scrollbar-width: none; }
  .tabs { position: relative; display: flex; flex-direction: column; gap: 10px; flex: 1; }
  .group { display: flex; flex-direction: column; gap: 2px; }
  .ind { position: absolute; left: 0; top: 0; height: 40px; border-radius: 12px; background: var(--surface-2); border: 1px solid var(--line); }
  .ind.glide { transition: transform 420ms cubic-bezier(0.3, 0.8, 0.2, 1), width 420ms cubic-bezier(0.3, 0.8, 0.2, 1); }
  .tab { position: relative; display: flex; gap: 12px; align-items: center; text-align: left; border-color: transparent; color: var(--muted); padding: 0 16px; height: 40px; width: 100%; }
  .tab :global(svg) { transition: transform 360ms var(--spring); }
  .tab:hover :global(svg) { transform: translateX(3px) scale(1.08); }
  .tab.active :global(svg) { transform: scale(1.1); }
  .tab:hover:not(:disabled) { color: var(--text); background: transparent; border-color: transparent; }
  .tab.active { color: var(--text); background: transparent; }
  .content { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
  .foot { display: grid; gap: 14px; padding: 0 6px; }
  .quickrow { display: flex; gap: 8px; }
  .modebtn { display: grid; place-items: center; width: 38px; height: 38px; flex: none; padding: 0; border-radius: 12px; border-color: var(--line); background: var(--surface); color: var(--muted); }
  .modebtn:hover:not(:disabled) { color: var(--text); background: var(--surface-2); border-color: var(--line-strong); }
  .community { display: grid; gap: 4px; padding: 12px 12px 12px 14px; border: 1px solid var(--line); border-radius: 16px; background: var(--surface); transition: border-color var(--ease); }
  .ch { display: flex; align-items: center; gap: 8px; }
  .community:hover { border-color: var(--line-strong); }
  .dc { display: inline-grid; color: #5865f2; }
  .ct { flex: 1; font-size: 13.5px; font-weight: 600; color: var(--text); }
  .hide { display: grid; place-items: center; width: 24px; height: 24px; padding: 0; margin: -4px -4px -4px 0; border-radius: 8px; color: var(--muted); opacity: 0; transition: opacity var(--ease); }
  .community:hover .hide, .hide:focus-visible { opacity: 1; }
  .cs { font-size: 12px; line-height: 1.4; color: var(--muted); }
  .join { width: 100%; height: 32px; margin-top: 8px; padding: 0 12px; border-radius: 999px; font-size: 13px; transition: opacity var(--ease), transform 300ms var(--spring); }
  .join:hover:not(:disabled) { opacity: 0.88; transform: translateY(-1px); }
  @media (max-height: 820px) {
    .community { display: flex; align-items: center; gap: 8px; padding: 8px 8px 8px 12px; }
    .ch { flex: 1; min-width: 0; }
    .cs { display: none; }
    .join { width: auto; height: 28px; margin-top: 0; padding: 0 14px; font-size: 12.5px; }
  }
  .scrollwrap { position: relative; flex: 1; display: flex; flex-direction: column; min-height: 0; }
  .edge { position: absolute; left: 0; right: 12px; z-index: 5; height: 40px; pointer-events: none; opacity: 0; transition: opacity 180ms ease; }
  .edge.top { top: 0; background: linear-gradient(var(--bg), transparent); }
  .edge.bottom { bottom: 0; background: linear-gradient(transparent, var(--bg)); }
  .edge.on { opacity: 1; }
  main { flex: 1; padding: 34px 40px 48px; overflow-x: hidden; overflow-y: auto; display: flex; flex-direction: column; gap: 20px; min-height: 0; }
  .toastwrap { position: fixed; z-index: 40; left: 0; right: 0; bottom: 100px; display: flex; justify-content: center; pointer-events: none; }
  .toast { max-width: min(520px, calc(100vw - 32px)); background: var(--inv-bg); color: var(--inv-text); border-radius: 999px; padding: 11px 20px; box-shadow: 0 8px 30px rgba(0, 0, 0, 0.35); pointer-events: auto; }

  @media (max-width: 700px) {
    .body { grid-template-columns: 1fr; grid-template-rows: auto minmax(0, 1fr) auto; }
    nav, .content { grid-row: auto; }
    .body > :global(.bar) { grid-row: auto; }
    nav { flex-direction: row; align-items: center; overflow-x: auto; border-bottom: 1px solid var(--line); padding: 10px 12px; }
    .tabs, .group { flex-direction: row; }
    .tabs { gap: 2px; }
    .community { display: none; }
    .tab { width: auto; white-space: nowrap; }
      main { padding: 20px 16px; }
  }
</style>
