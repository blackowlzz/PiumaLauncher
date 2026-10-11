<script lang="ts">
  import { onMount } from 'svelte'
  import type { Window } from '@tauri-apps/api/window'
  import { inTauri } from './api'
  import { t } from './i18n.svelte'
  import Icon from './Icon.svelte'

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

  const edges = [
    ['North', 'n'], ['South', 's'], ['East', 'e'], ['West', 'w'],
    ['NorthEast', 'ne'], ['NorthWest', 'nw'], ['SouthEast', 'se'], ['SouthWest', 'sw'],
  ] as const
</script>

<header class="bar" data-tauri-drag-region>
  <div class="id" data-tauri-drag-region>
    <span class="mark"><Icon name="feather" size={17} /></span>
    <span class="nm">PiumaLauncher</span>
    <span class="ver">v{__APP_VERSION__}</span>
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
  .bar { position: relative; z-index: 150; display: flex; align-items: center; justify-content: space-between; height: 40px; padding-left: 16px; border-bottom: 1px solid var(--line); background: var(--bg); user-select: none; -webkit-user-select: none; }
  .id { display: flex; align-items: center; gap: 10px; flex: 1; height: 100%; }
  .id > * { pointer-events: none; }
  .mark { display: grid; place-items: center; }
  .nm { font-size: 14px; font-weight: 600; letter-spacing: -0.03em; }
  .ver { font-size: 11px; color: var(--muted); }
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
