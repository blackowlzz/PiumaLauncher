<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { fly } from 'svelte/transition'
  import { app, init, installModpack, notify, openInstance, playInfo, saveSettings, type Page } from './lib/store.svelte'
  import { inTauri } from './lib/api'
  import { t } from './lib/i18n.svelte'
  import LanguageMenu from './lib/LanguageMenu.svelte'
  import Welcome from './lib/Welcome.svelte'
  import ProjectDetail from './lib/ProjectDetail.svelte'
  import Cover from './lib/Cover.svelte'
  import Icon from './lib/Icon.svelte'
  import Home from './lib/Home.svelte'
  import Library from './lib/Library.svelte'
  import InstanceDetail from './lib/InstanceDetail.svelte'
  import PlayBar from './lib/PlayBar.svelte'
  import Accounts from './lib/Accounts.svelte'
  import Explore from './lib/Explore.svelte'
  import Settings from './lib/Settings.svelte'
  import TitleBar from './lib/TitleBar.svelte'
  import { maybeBench } from './lib/bench'
  import { smoothScroll } from './lib/smoothscroll'

  const tabs: { id: Page; label: string; icon: 'home' | 'grid' | 'compass' | 'user' | 'settings' }[] = [
    { id: 'home', label: 'Home', icon: 'home' },
    { id: 'library', label: 'Library', icon: 'grid' },
    { id: 'explore', label: 'Explore', icon: 'compass' },
    { id: 'accounts', label: 'Account', icon: 'user' },
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

  $effect(() => {
    document.documentElement.toggleAttribute('data-reduce', app.settings.reduceEffects)
  })

  $effect(() => {
    if (theme === 'system') delete document.documentElement.dataset.theme
    else document.documentElement.dataset.theme = theme
  })

  function toggleTheme() {
    app.settings.theme = dark ? 'light' : 'dark'
    saveSettings()
  }

  const recents = $derived(
    app.instances
      .map((i) => ({ inst: i, last: playInfo(i.id).last }))
      .filter((r) => r.last > 0)
      .sort((a, b) => b.last - a.last)
      .slice(0, 3),
  )

  let zoom = 1
  async function fitScale() {
    if (!inTauri) return
    const s = app.settings.uiScale
    const physical = innerWidth * zoom
    const z = s === 'auto' ? (physical < 1700 ? 1 : physical < 2200 ? 1.15 : physical < 2900 ? 1.3 : 1.5) : Number(s) / 100
    if (z === zoom) return
    zoom = z
    const { getCurrentWebview } = await import('@tauri-apps/api/webview')
    await getCurrentWebview().setZoom(z)
  }
  $effect(() => {
    void app.settings.uiScale
    void fitScale()
  })

  const order: Page[] = ['home', 'library', 'explore', 'accounts', 'settings']
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
    } else if (mod && /^[1-5]$/.test(e.key)) {
      e.preventDefault()
      app.page = order[Number(e.key) - 1]
    }
  }

  async function installFromDetail(versionId?: string) {
    const hit = app.detail
    if (hit && (await installModpack(hit.projectId, versionId, hit.latestMc))) app.detail = null
  }

  onMount(() => {
    const onScheme = () => (systemDark = !scheme.matches)
    scheme.addEventListener('change', onScheme)
    addEventListener('resize', fitScale)
    init().then(maybeBench).catch((e) => notify(t('Startup error: {error}', { error: e })))
    return () => {
      scheme.removeEventListener('change', onScheme)
      removeEventListener('resize', fitScale)
    }
  })
</script>

<svelte:window onkeydown={shortcuts} />

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
      {#if recents.length}
        <div class="rtiles" aria-label={t('Recent instances')}>
          {#each recents as r, n (r.inst.id)}
            <button class="rt" style:--n={n} onclick={() => openInstance(r.inst.id)} title={r.inst.name} aria-label={t('Open {name}', { name: r.inst.name })}>
              <Cover inst={r.inst} />
            </button>
          {/each}
        </div>
      {/if}
      <div class="meta">
        <button class="iconbtn ghost" onclick={toggleTheme} aria-label={dark ? t('Switch to light theme') : t('Switch to dark theme')} title={dark ? t('Light theme') : t('Dark theme')}>
          <Icon name={dark ? 'sun' : 'moon'} size={17} />
        </button>
        <LanguageMenu />
      </div>
    </div>
  </nav>

  <div class="content">
    <main use:smoothScroll={app.settings.smoothScroll && !app.settings.reduceEffects}>
      {#key `${app.page}:${app.detailId ?? ''}`}
        <div class="page">
          {#if app.page === 'home'}
            <Home />
          {:else if app.page === 'library'}
            <Library />
          {:else if app.page === 'instance'}
            <InstanceDetail />
          {:else if app.page === 'accounts'}
            <Accounts />
          {:else if app.page === 'settings'}
            <Settings />
          {:else}
            <Explore />
          {/if}
        </div>
      {/key}
    </main>
    <PlayBar />
  </div>

  </div>

  {#if app.toast}
    <div class="toastwrap"><div class="toast" role="alert" transition:fly={{ y: 16, duration: 260 }}>{app.toast}</div></div>
  {/if}
</div>

{#if app.detail}
  {#key app.detail.projectId}
    <ProjectDetail hit={app.detail} busy={app.installingId === app.detail.projectId} disabled={app.installingId !== null} onInstall={installFromDetail} onClose={() => (app.detail = null)} />
  {/key}
{/if}

<Welcome />

<style>
  .shell { display: grid; grid-template-rows: auto 1fr; height: 100%; }
  .body { display: grid; grid-template-columns: 208px 1fr; min-height: 0; }
  nav { display: flex; flex-direction: column; gap: 4px; padding: 22px 12px 14px; border-right: 1px solid var(--line); }
  .tabs { position: relative; display: grid; grid-auto-rows: 42px; gap: 2px; flex: 1; align-content: start; }
  .ind { position: absolute; left: 0; right: 0; top: 0; height: 42px; border-radius: 999px; background: var(--surface-2); border: 1px solid var(--line); transform: translateY(calc(var(--i) * 44px)); transition: transform 420ms cubic-bezier(0.3, 0.8, 0.2, 1); }
  .tab { position: relative; display: flex; gap: 12px; align-items: center; text-align: left; border-color: transparent; color: var(--muted); padding: 0 16px; height: 42px; width: 100%; }
  .tab :global(svg) { transition: transform 360ms var(--spring); }
  .tab:hover :global(svg) { transform: translateX(3px) scale(1.08); }
  .tab.active :global(svg) { transform: scale(1.1); }
  .tab:hover:not(:disabled) { color: var(--text); background: transparent; border-color: transparent; }
  .tab.active { color: var(--text); background: transparent; }
  .content { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
  .foot { display: grid; gap: 14px; padding: 0 6px; }
  .rtiles { display: flex; gap: 10px; }
  .rt { animation: swapin 500ms var(--spring) backwards calc(var(--n, 0) * 70ms + 250ms); position: relative; width: 46px; height: 46px; padding: 0; border-radius: 15px; overflow: hidden; display: grid; place-items: center; flex: none; transition: transform 300ms var(--spring), border-color var(--ease); }
  .rt:hover { transform: translateY(-3px) scale(1.06); border-color: var(--text); }
  .meta { display: flex; align-items: center; gap: 8px; border-top: 1px solid var(--line); padding-top: 12px; }
  main { flex: 1; padding: 34px 40px; overflow-x: hidden; overflow-y: auto; display: flex; flex-direction: column; gap: 20px; min-height: 0; }
  .toastwrap { position: fixed; z-index: 40; left: 0; right: 0; bottom: 100px; display: flex; justify-content: center; pointer-events: none; }
  .toast { max-width: min(520px, calc(100vw - 32px)); background: var(--inv-bg); color: var(--inv-text); border-radius: 999px; padding: 11px 20px; box-shadow: 0 8px 30px rgba(0, 0, 0, 0.35); pointer-events: auto; }

  @media (max-width: 700px) {
    .body { grid-template-columns: 1fr; grid-template-rows: auto 1fr; }
    nav { flex-direction: row; align-items: center; overflow-x: auto; border-right: 0; border-bottom: 1px solid var(--line); padding: 10px 12px; }
    .tabs { display: flex; flex: 1; }
    .tab { width: auto; white-space: nowrap; }
      main { padding: 20px 16px; }
  }
</style>
