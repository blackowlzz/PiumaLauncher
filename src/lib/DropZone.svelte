<script lang="ts">
  import { onMount } from 'svelte'
  import { fade } from 'svelte/transition'
  import { api, inTauri } from './api'
  import { app, importPack } from './store.svelte'
  import { t } from './i18n.svelte'
  import Icon from './Icon.svelte'

  let over = $state(false)
  const isPack = (p: string) => /\.mrpack$/i.test(p)

  onMount(() => {
    if (!inTauri) return
    let off: (() => void) | undefined
    let alive = true
    import('@tauri-apps/api/webview').then(async ({ getCurrentWebview }) => {
      const un = await getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload
        if (p.type === 'enter') over = p.paths.some(isPack)
        else if (p.type === 'leave') over = false
        else if (p.type === 'drop') {
          over = false
          const pack = p.paths.find(isPack)
          if (pack && !app.installingId) {
            app.page = 'library'
            importPack(() => api.importMrpackFile(pack))
          }
        }
      })
      if (alive) off = un
      else un()
    })
    return () => {
      alive = false
      off?.()
    }
  })
</script>

{#if over}
  <div class="drop" transition:fade={{ duration: 160 }} aria-hidden="true">
    <svg class="frame"><rect width="100%" height="100%" rx="22" /></svg>
    <div class="center">
      <span class="pk"><Icon name="package" size={40} /></span>
      <p class="big">{t('Drop to import the modpack')}</p>
      <p class="muted">{t('A new instance will be created')}</p>
    </div>
  </div>
{/if}

<style>
  .drop { position: fixed; inset: 0; z-index: 400; display: grid; place-items: center; padding: 18px; background: color-mix(in srgb, var(--bg) 78%, transparent); backdrop-filter: blur(10px); pointer-events: none; }
  .frame { position: absolute; inset: 18px; width: calc(100% - 36px); height: calc(100% - 36px); overflow: visible; }
  .frame rect { fill: none; stroke: var(--text); stroke-width: 2; stroke-dasharray: 10 8; animation: march 900ms linear infinite; }
  @keyframes march { to { stroke-dashoffset: -18; } }
  .center { display: grid; justify-items: center; gap: 8px; text-align: center; animation: gopop 360ms var(--spring) backwards; }
  .pk { display: grid; place-items: center; width: 92px; height: 92px; border-radius: 26px; background: var(--inv-bg); color: var(--inv-text); margin-bottom: 10px; animation: bob 1.6s ease-in-out infinite; }
  @keyframes bob { 50% { transform: translateY(-8px) rotate(-4deg); } }
  .big { font-size: 22px; font-weight: 650; letter-spacing: -0.03em; }
</style>
