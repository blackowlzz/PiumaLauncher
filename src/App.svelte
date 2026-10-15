<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { fly } from 'svelte/transition'
  import { app, handleCli, init, installContentInto, installProject, notify, saveSettings, type Page } from './lib/store.svelte'
  import { inTauri, type PackVersion } from './lib/api'
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
  import Intro from './lib/Intro.svelte'
  import ConfirmDialog from './lib/ConfirmDialog.svelte'
  import CrashDialog from './lib/CrashDialog.svelte'
  import InstanceChooser from './lib/InstanceChooser.svelte'
  import DropZone from './lib/DropZone.svelte'
  import ContextMenu from './lib/ContextMenu.svelte'
  import ShareModal from './lib/ShareModal.svelte'
  import Themes from './lib/Themes.svelte'
  import Wardrobe from './lib/Wardrobe.svelte'
  import { applyTheme, isCustomized, resetTheme } from './lib/palette.svelte'
  import UpdateBanner from './lib/UpdateBanner.svelte'
  import TitleBar from './lib/TitleBar.svelte'
  import { maybeBench } from './lib/bench'
  import { smoothScroll } from './lib/smoothscroll'

  const tabs: { id: Page; label: string; icon: 'home' | 'grid' | 'feather' | 'server' | 'compass' | 'user' | 'settings' | 'palette' | 'cpu' }[] = [
    { id: 'home', label: 'Home', icon: 'home' },
    { id: 'quickplay', label: 'Quick Play', icon: 'server' },
    { id: 'library', label: 'Library', icon: 'grid' },
    { id: 'explore', label: 'Explore', icon: 'compass' },
    { id: 'accounts', label: 'Account', icon: 'user' },
    { id: 'wardrobe', label: 'Wardrobe', icon: 'feather' },
    { id: 'themes', label: 'Themes', icon: 'palette' },
    { id: 'performance', label: 'Performance', icon: 'cpu' },
    { id: 'settings', label: 'Settings', icon: 'settings' },
  ]
  const section = $derived(app.page === 'instance' ? 'library' : app.page)
  const activeIndex = $derived(Math.max(0, tabs.findIndex((x) => x.id === section)))

  const q = new URLSearchParams(location.search).get('theme')
  const forced = q === 'light' || q === 'dark' ? q : null
  const scheme = matchMedia('(prefers-color-scheme: light)')
  let systemDark = $state(!scheme.matches)
  const theme = $derived(forced ?? app.settings.theme)
  const dark = $derived(theme === 'dark' || (theme === 'system' && systemDark))

  onMount(applyTheme)

  $effect(() => {
    document.documentElement.toggleAttribute('data-reduce', app.settings.reduceEffects)
  })

  $effect(() => {
    if (theme === 'system') delete document.documentElement.dataset.theme
    else document.documentElement.dataset.theme = theme
  })

  function toggleTheme() {
    if (isCustomized()) resetTheme()
    app.settings.theme = dark ? 'light' : 'dark'
    saveSettings()
  }

  let zoom = 1
  let fitting = false
  let refit = false
  let settle: ReturnType<typeof setTimeout> | undefined
  async function fitScale() {
    if (!inTauri) return
    if (fitting) {
      refit = true
      return
    }
    fitting = true
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
        }
      } while (refit)
    } finally {
      fitting = false
    }
  }
  const onResize = () => {
    void fitScale()
    clearTimeout(settle)
    settle = setTimeout(() => void fitScale(), 250)
  }
  $effect(() => {
    void app.settings.uiScale
    void fitScale()
  })

  function nativeMenu(e: MouseEvent) {
    if (import.meta.env.DEV) return
    const el = e.target as HTMLElement | null
    const editable = !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable)
    if (!editable && !window.getSelection()?.toString()) e.preventDefault()
  }

  const order: Page[] = ['home', 'quickplay', 'library', 'explore', 'accounts', 'wardrobe', 'themes', 'performance', 'settings']
  async function shortcuts(e: KeyboardEvent) {
    const el = e.target as HTMLElement | null
    const typing = !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable)
    const mod = e.ctrlKey || e.metaKey
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
    init().then(maybeBench).then(handleCli).catch((e) => notify(t('Startup error: {error}', { error: e })))
    return () => {
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

<div class="shell">
  <TitleBar />
  <div class="body">
  <nav aria-label={t('Sections')}>
    <div class="tabs" style:--i={activeIndex}>
      <span class="ind" aria-hidden="true"></span>
      {#each tabs as tb}
        <button class="tab" class:active={section === tb.id} aria-current={section === tb.id ? 'page' : undefined} onclick={() => (app.page = tb.id)}>
          <Icon name={tb.icon} />{t(tb.label)}
        </button>
      {/each}
    </div>
    <div class="foot">
      <div class="meta">
        <div class="mode">
          <Icon name={dark ? 'moon' : 'sun'} size={17} />
          <span class="grow">{t('Dark mode')}</span>
          <label class="switch">
            <input type="checkbox" checked={dark} onchange={toggleTheme} aria-label={t('Dark mode')} />
            <span class="track"></span>
          </label>
        </div>
        <LanguageMenu />
      </div>
    </div>
  </nav>

  <div class="content">
    <UpdateBanner />
    <main use:smoothScroll={app.settings.smoothScroll && !app.settings.reduceEffects}>
      {#key `${app.page}:${app.detailId ?? ''}`}
        <div class="page">
          {#if app.page === 'home'}
            <Home />
          {:else if app.page === 'library'}
            <Library />
          {:else if app.page === 'instance'}
            <InstanceDetail />
          {:else if app.page === 'quickplay'}
            <QuickPlay />
          {:else if app.page === 'accounts'}
            <Accounts />
          {:else if app.page === 'wardrobe'}
            <Wardrobe />
          {:else if app.page === 'themes'}
            <Themes />
          {:else if app.page === 'performance'}
            <Performance />
          {:else if app.page === 'settings'}
            <Settings />
          {:else}
            <Explore />
          {/if}
        </div>
      {/key}
    </main>
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
<Intro />
<Welcome />

<style>
  .shell { display: grid; grid-template-rows: auto 1fr; height: 100%; }
  .body { grid-template-rows: minmax(0, 1fr) auto; }
  .body { display: grid; grid-template-columns: 216px minmax(0, 1fr); min-height: 0; }
  nav { grid-row: 1; }
  .content { grid-row: 1; }
  .body > :global(.bar) { grid-column: 1 / -1; grid-row: 2; }
  nav { display: flex; flex-direction: column; gap: 4px; padding: 22px 12px 14px; border-right: 1px solid var(--line); }
  .tabs { position: relative; display: grid; grid-auto-rows: 42px; gap: 2px; flex: 1; align-content: start; }
  .ind { position: absolute; left: 0; right: 0; top: 0; height: 42px; border-radius: 12px; background: var(--surface-2); border: 1px solid var(--line); transform: translateY(calc(var(--i) * 44px)); transition: transform 420ms cubic-bezier(0.3, 0.8, 0.2, 1); }
  .tab { position: relative; display: flex; gap: 12px; align-items: center; text-align: left; border-color: transparent; color: var(--muted); padding: 0 16px; height: 42px; width: 100%; }
  .tab :global(svg) { transition: transform 360ms var(--spring); }
  .tab:hover :global(svg) { transform: translateX(3px) scale(1.08); }
  .tab.active :global(svg) { transform: scale(1.1); }
  .tab:hover:not(:disabled) { color: var(--text); background: transparent; border-color: transparent; }
  .tab.active { color: var(--text); background: transparent; }
  .content { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
  .foot { display: grid; gap: 14px; padding: 0 6px; }
  .meta { display: grid; gap: 8px; }
  .mode { display: flex; align-items: center; gap: 12px; height: 38px; padding: 0 10px; color: var(--muted); font-size: 13px; }
  main { flex: 1; padding: 34px 40px 48px; overflow-x: hidden; overflow-y: auto; display: flex; flex-direction: column; gap: 20px; min-height: 0; }
  .toastwrap { position: fixed; z-index: 40; left: 0; right: 0; bottom: 100px; display: flex; justify-content: center; pointer-events: none; }
  .toast { max-width: min(520px, calc(100vw - 32px)); background: var(--inv-bg); color: var(--inv-text); border-radius: 999px; padding: 11px 20px; box-shadow: 0 8px 30px rgba(0, 0, 0, 0.35); pointer-events: auto; }

  @media (max-width: 700px) {
    .body { grid-template-columns: 1fr; grid-template-rows: auto minmax(0, 1fr) auto; }
    nav, .content { grid-row: auto; }
    .body > :global(.bar) { grid-row: auto; }
    nav { flex-direction: row; align-items: center; overflow-x: auto; border-right: 0; border-bottom: 1px solid var(--line); padding: 10px 12px; }
    .tabs { display: flex; flex: 1; }
    .tab { width: auto; white-space: nowrap; }
      main { padding: 20px 16px; }
  }
</style>
