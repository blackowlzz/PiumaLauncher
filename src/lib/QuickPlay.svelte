<script lang="ts">
  import type { WorldEntry } from './api'
  import Art from './Art.svelte'
  import Icon from './Icon.svelte'
  import Select from './Select.svelte'
  import Servers from './Servers.svelte'
  import Worlds from './Worlds.svelte'
  import { t } from './i18n.svelte'
  import { thumb } from './img'
  import { relativeTime } from './meta'
  import { opensWorlds } from './serverstatus.svelte'
  import { app, isBusy, photoFor, play } from './store.svelte'

  let query = $state('')
  let instance = $state('all')
  let worlds: WorldEntry[] = $state([])
  let serversRef: Servers | undefined = $state()
  let worldsRef: Worlds | undefined = $state()
  let order: 'recent' | 'name' | 'size' = $state('recent')
  let spin = $state(0)
  type View = 'servers' | 'worlds'
  const VIEW_KEY = 'piuma-quick-view'
  let view: View = $state(readView())
  const views: { id: View; label: string; icon: 'map' | 'server' }[] = [
    { id: 'servers', label: 'Servers', icon: 'server' },
    { id: 'worlds', label: 'Worlds', icon: 'map' },
  ]
  function readView(): View {
    try {
      return localStorage.getItem(VIEW_KEY) === 'worlds' ? 'worlds' : 'servers'
    } catch {
      return 'servers'
    }
  }
  let dock: HTMLElement | undefined = $state()
  let pill = $state<{ x: number; w: number } | null>(null)
  let glide = $state(false)
  function place() {
    const el = dock?.querySelector<HTMLElement>('[aria-selected="true"]')
    pill = el ? { x: el.offsetLeft, w: el.offsetWidth } : null
  }
  $effect(() => {
    void view
    place()
  })
  $effect(() => {
    if (!dock) return
    const ro = new ResizeObserver(place)
    ro.observe(dock)
    requestAnimationFrame(() => (glide = true))
    return () => ro.disconnect()
  })
  function fill(node: HTMLElement) {
    const page = node.parentElement
    if (page) page.style.flex = '1 0 auto'
    return { destroy: () => page?.style.removeProperty('flex') }
  }

  function pick(v: View) {
    view = v
    try {
      localStorage.setItem(VIEW_KEY, v)
    } catch {}
    document.querySelector('main')?.scrollTo({ top: 0, behavior: 'smooth' })
  }

  const instOf = (id: string) => app.instances.find((i) => i.id === id)
  const last = $derived(worlds.reduce<WorldEntry | null>((a, w) => (!a || w.modifiedMs > a.modifiedMs ? w : a), null))
  const lastInst = $derived(last ? instOf(last.instanceId) : undefined)
  const lastPhoto = $derived(last ? photoFor(last.version ?? undefined, last.name) : null)

  function resume() {
    if (!last || !lastInst) return
    if (opensWorlds(lastInst.mcVersion)) play(lastInst.id, { kind: 'world', folder: last.folder })
    else play(lastInst.id)
  }
</script>

<div class="qp" use:fill>
  <div class="top">
  <header class="qhead">
    <h1>{t('Quick Play')}</h1>
    <p class="muted">{t('Jump straight into a world or a server.')}</p>
  </header>

  <div class="toolbar">
    <label class="search">
      <span class="field"><Icon name="search" size={16} /><input type="search" bind:value={query} placeholder={view === 'worlds' ? t('Search worlds…') : t('Search servers…')} aria-label={view === 'worlds' ? t('Search worlds…') : t('Search servers…')} /></span>
    </label>
    {#if app.instances.length > 1}
      <Select bind:value={instance} width="auto" label={t('Instance')} options={[{ value: 'all', label: t('All instances') }, ...app.instances.map((i) => ({ value: i.id, label: i.name }))]} />
    {/if}
    {#if view === 'servers'}
      <button class="ghost iconbtn" onclick={() => (spin++, serversRef?.refresh())} aria-label={t('Refresh')} title={t('Refresh')}><span class="spin" style:--r="{spin * 360}deg"><Icon name="refresh" size={16} /></span></button>
      <button class="act" onclick={() => serversRef?.add()}><Icon name="plus" size={15} />{t('Add a server')}</button>
    {:else}
      <Select bind:value={order} width="auto" label={t('Sort by')} options={[{ value: 'recent', label: t('Last played') }, { value: 'name', label: t('Name') }, { value: 'size', label: t('Size') }]} />
      <button class="act" onclick={() => worldsRef?.importWorld()}><Icon name="download" size={15} />{t('Import world')}</button>
    {/if}
  </div>
  </div>

  {#if view === 'servers'}
    <section class="sec">
      <Servers bind:this={serversRef} filter={query} {instance} />
    </section>
  {/if}

  {#if last && lastInst && view === 'worlds' && !query}
    <section class="resume">
      <div class="rcover">
        {#if last.icon}
          <img class="rbg" src={last.icon} alt="" />
          <img class="ricon" src={last.icon} alt="" />
        {:else if lastPhoto}
          <img class="rphoto" data-bare use:thumb={{ src: lastPhoto, w: 640 }} alt="" decoding="async" />
        {:else}
          <Art seed={last.name} />
        {/if}
      </div>
      <div class="rtxt">
        <span class="muted small">{t('Continue where you left off')}</span>
        <span class="rname" title={last.name}>{last.name}</span>
        <span class="muted small">{lastInst.name} · {relativeTime(last.modifiedMs)}</span>
      </div>
      <button class="primary rplay" onclick={resume} disabled={isBusy(lastInst.id) || !app.accounts.active}><Icon name="play" size={14} />{t('Play')}</button>
    </section>
  {/if}

  <section class="sec" hidden={view !== 'worlds'}>
    <Worlds bind:this={worldsRef} filter={query} {instance} {order} onworlds={(l) => (worlds = l)} />
  </section>

  <nav class="dock" aria-label={t('Quick Play')}>
    <div class="dtabs" role="tablist" bind:this={dock}>
      {#if pill}<span class="pill" class:glide aria-hidden="true" style:width="{pill.w}px" style:transform="translateX({pill.x}px)"></span>{/if}
      {#each views as v}
        <button role="tab" aria-selected={view === v.id} onclick={() => pick(v.id)}><Icon name={v.icon} size={15} />{t(v.label)}</button>
      {/each}
    </div>
  </nav>
</div>

<style>
  .qp { flex: 1; display: flex; flex-direction: column; gap: 22px; margin-bottom: -34px; }
  .qhead { display: grid; gap: 6px; }
  .resume { display: flex; align-items: center; gap: 18px; padding: 12px 18px 12px 12px; border: 1px solid var(--line); border-radius: 20px; background: var(--surface); }
  .rcover { position: relative; flex: none; width: 150px; height: 84px; border-radius: 14px; overflow: hidden; display: grid; place-items: center; background: #000; }
  .rcover :global(.art) { position: absolute; inset: 0; }
  .rbg { position: absolute; inset: -10%; width: 120%; height: 120%; object-fit: cover; opacity: 0.55; }
  .ricon { position: relative; width: 52px; height: 52px; border-radius: 10px; image-rendering: pixelated; }
  .rphoto { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
  .rtxt { flex: 1; min-width: 0; display: grid; gap: 2px; }
  .rname { font-size: 20px; font-weight: 650; letter-spacing: -0.03em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .rplay { display: inline-flex; align-items: center; gap: 8px; padding: 11px 26px; flex: none; }
  .top { display: grid; gap: 18px; }
  .toolbar { display: flex; gap: 10px; align-items: center; }
  .act { flex: none; display: inline-flex; align-items: center; gap: 8px; white-space: nowrap; }
  .spin { display: grid; place-items: center; line-height: 0; transform: rotate(var(--r)); transition: transform 700ms var(--spring); }
  .search { flex: 1; min-width: 220px; }
  .field { position: relative; display: block; }
  .field :global(svg) { position: absolute; left: 11px; top: 50%; transform: translateY(-50%); color: var(--faint); pointer-events: none; }
  .field input { padding-left: 34px; }
  .sec { display: grid; gap: 14px; }
  .sec[hidden] { display: none; }
  .dock { position: sticky; bottom: -34px; z-index: 6; align-self: center; margin-top: auto; padding: 5px; border-radius: 999px; border: 1px solid color-mix(in srgb, var(--line-strong) 70%, transparent); background: color-mix(in srgb, var(--surface-2) 78%, transparent); -webkit-backdrop-filter: blur(20px) saturate(160%); backdrop-filter: blur(20px) saturate(160%); box-shadow: 0 12px 32px rgba(0, 0, 0, 0.45), inset 0 1px 0 rgba(255, 255, 255, 0.05); }
  .dtabs { position: relative; display: grid; grid-auto-flow: column; grid-auto-columns: 1fr; gap: 2px; }
  .pill { position: absolute; left: 0; top: 0; bottom: 0; border-radius: 999px; background: var(--inv-bg); }
  .pill.glide { transition: transform 420ms cubic-bezier(0.3, 0.8, 0.2, 1), width 420ms cubic-bezier(0.3, 0.8, 0.2, 1); }
  .dtabs button { position: relative; display: inline-flex; align-items: center; justify-content: center; gap: 8px; min-width: 124px; height: 38px; padding: 0 20px; border: 0; border-radius: 999px; background: transparent; color: var(--muted); font-size: 13.5px; font-weight: 600; transition: color 260ms ease; }
  .dtabs button:hover:not(:disabled) { color: var(--text); background: transparent; }
  .dtabs button[aria-selected='true'], .dtabs button[aria-selected='true']:hover:not(:disabled) { color: var(--inv-text); }
  @media (max-width: 700px) { .rcover { display: none; } .qp { margin-bottom: 0; } .dock { bottom: -6px; } }
</style>
