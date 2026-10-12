<script lang="ts">
  import { onMount, tick } from 'svelte'
  import type { Window } from '@tauri-apps/api/window'
  import { api, inTauri } from './api'
  import { t } from './i18n.svelte'
  import { app, notify } from './store.svelte'
  import Icon from './Icon.svelte'
  import mark from '../assets/piuma.png'

  let win: Window | null = $state(null)
  let maximized = $state(false)

  onMount(() => {
    let off: (() => void) | undefined
    if (inTauri) {
      void (async () => {
        const { getCurrentWindow } = await import('@tauri-apps/api/window')
        win = getCurrentWindow()
        maximized = await win.isMaximized()
        off = await win.onResized(async () => (maximized = await win!.isMaximized()))
      })()
    }
    return () => off?.()
  })

  let term = $state('')
  async function search(e: SubmitEvent) {
    e.preventDefault()
    const q = term.trim()
    app.page = 'explore'
    await tick()
    const input = document.querySelector<HTMLInputElement>('main input[type="search"]')
    if (!input) return
    input.value = q
    input.dispatchEvent(new Event('input', { bubbles: true }))
    term = ''
    ;(document.activeElement as HTMLElement | null)?.blur()
  }

  const edges = [
    ['North', 'n'], ['South', 's'], ['East', 'e'], ['West', 'w'],
    ['NorthEast', 'ne'], ['NorthWest', 'nw'], ['SouthEast', 'se'], ['SouthWest', 'sw'],
  ] as const
</script>

<header class="bar" data-tauri-drag-region>
  <div class="id" data-tauri-drag-region>
    <span class="mark" style:--m="url({mark})" aria-hidden="true"></span>
    <span class="nm">PiumaLauncher</span>
    <span class="ver">v{__APP_VERSION__}</span>
  </div>

  <form class="find" onsubmit={search} role="search">
    <Icon name="search" size={15} />
    <input type="text" bind:value={term} placeholder={t('Search instances, modpacks, servers…')} aria-label={t('Search')} />
  </form>

  <div class="tools">
    <button class="iconbtn ghost" onclick={() => api.openUrl('https://discord.gg/abHWHuybYY').catch(notify)} aria-label="Discord" title="Discord"><Icon name="discord" size={17} /></button>
    <button class="iconbtn ghost" onclick={() => api.openUrl('https://ko-fi.com/blackowlzz').catch(notify)} aria-label="Ko-fi" title="Ko-fi"><Icon name="kofi" size={18} /></button>
    <button class="iconbtn ghost" onclick={() => (app.page = 'themes')} aria-label={t('Themes')} title={t('Themes')}><Icon name="palette" size={17} /></button>
    <button class="iconbtn ghost" onclick={() => (app.page = 'settings')} aria-label={t('Settings')} title={t('Settings')}><Icon name="settings" size={17} /></button>
  </div>

  {#if inTauri}
    <div class="ctl">
      <button onclick={() => win?.minimize()} aria-label={t('Minimize')} title={t('Minimize')}><Icon name="minimize" size={16} /></button>
      <button onclick={() => win?.toggleMaximize()} aria-label={maximized ? t('Restore') : t('Maximize')} title={maximized ? t('Restore') : t('Maximize')}>
        <Icon name={maximized ? 'restore' : 'maximize'} size={15} />
      </button>
      <button class="close" onclick={() => win?.close()} aria-label={t('Close')} title={t('Close')}><Icon name="x" size={17} /></button>
    </div>
  {/if}
</header>

{#if inTauri && !maximized}
  {#each edges as [dir, cls]}
    <div class="rz {cls}" role="presentation" onpointerdown={(e) => { e.preventDefault(); void win?.startResizeDragging(dir) }}></div>
  {/each}
{/if}

<style>
  .bar { position: relative; z-index: 150; display: flex; align-items: center; gap: 14px; height: 44px; padding-left: 16px; border-bottom: 1px solid var(--line); background: var(--bg); user-select: none; -webkit-user-select: none; }
  .id { display: flex; align-items: center; gap: 10px; flex: 1; height: 100%; }
  .id > * { pointer-events: none; }
  .mark { width: 19px; height: 19px; background: var(--text); -webkit-mask: var(--m) center / contain no-repeat; mask: var(--m) center / contain no-repeat; }
  .nm { font-size: 14px; font-weight: 600; letter-spacing: -0.03em; }
  .ver { font-size: 11px; color: var(--muted); }
  .find { display: flex; align-items: center; gap: 10px; width: min(380px, 34vw); height: 30px; padding: 0 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--surface); color: var(--faint); -webkit-user-select: text; user-select: text; }
  .find:focus-within { border-color: var(--muted); color: var(--muted); }
  .find input { border: 0; background: transparent; padding: 0; height: 100%; font-size: 13px; border-radius: 0; }
  .find input:focus { background: transparent; }
  .tools { display: flex; gap: 2px; }
  .tools button { width: 32px; height: 32px; }
  .ctl { display: flex; height: 100%; }
  .ctl button { display: grid; place-items: center; width: 48px; height: 100%; padding: 0; border: 0; border-radius: 0; color: var(--muted); background: transparent; }
  .ctl button:hover:not(:disabled) { background: var(--surface-2); color: var(--text); border: 0; }
  .ctl .close:hover:not(:disabled) { background: #e5484d; color: #fff; }
  .rz { position: fixed; z-index: 200; }
  .n { top: 0; left: 10px; right: 10px; height: 4px; cursor: ns-resize; }
  .s { bottom: 0; left: 10px; right: 10px; height: 4px; cursor: ns-resize; }
  .e { right: 0; top: 10px; bottom: 10px; width: 4px; cursor: ew-resize; }
  .w { left: 0; top: 10px; bottom: 10px; width: 4px; cursor: ew-resize; }
  .ne { top: 0; right: 0; width: 10px; height: 10px; cursor: nesw-resize; }
  .nw { top: 0; left: 0; width: 10px; height: 10px; cursor: nwse-resize; }
  .se { bottom: 0; right: 0; width: 10px; height: 10px; cursor: nwse-resize; }
  .sw { bottom: 0; left: 0; width: 10px; height: 10px; cursor: nesw-resize; }
</style>
