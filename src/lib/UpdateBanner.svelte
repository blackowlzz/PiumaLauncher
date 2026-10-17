<script lang="ts">
  import { onMount } from 'svelte'
  import { slide } from 'svelte/transition'
  import { listen } from '@tauri-apps/api/event'
  import Icon from './Icon.svelte'
  import { api, inTauri } from './api'
  import { t } from './i18n.svelte'
  import { app, notify } from './store.svelte'

  const update = $derived(app.update)
  let phase: 'idle' | 'downloading' | 'installing' = $state('idle')
  let percent: number | null = $state(null)
  const playing = $derived(Object.values(app.games).some((g) => g.state === 'running' || g.state === 'preparing'))

  onMount(() => {
    if (!inTauri) return
    const offs = [
      listen<{ done: number; total: number | null }>('update-progress', (e) => {
        const { done, total } = e.payload
        percent = total ? Math.min(100, Math.round((done / total) * 100)) : null
      }),
      listen('update-installing', () => (phase = 'installing')),
    ]
    return () => offs.forEach((o) => void o.then((f) => f()))
  })

  function download() {
    if (update) api.openUrl(update.url).catch(notify)
  }

  async function install() {
    if (!update || phase !== 'idle') return
    phase = 'downloading'
    percent = 0
    try {
      await api.installUpdate()
    } catch (e) {
      notify(e)
      phase = 'idle'
      percent = null
    }
  }
</script>

{#if update && !app.updateDismissed}
  <div class="banner" role="status" transition:slide={{ duration: 320 }}>
    {#if phase === 'downloading'}<div class="line bar" class:indeterminate={percent === null}><div style:width={percent !== null ? `${percent}%` : undefined}></div></div>{/if}
    <span class="ic"><Icon name="download" size={18} /></span>
    <div class="txt" aria-live="polite">
      <strong>{t('Update available')}</strong>
      <span class="muted">
        {#if phase === 'installing'}
          {t('Installing, the launcher restarts by itself…')}
        {:else if phase === 'downloading'}
          {percent !== null ? t('Downloading… {n}%', { n: percent }) : t('Downloading…')}
        {:else}
          {t('PiumaLauncher {v} is ready to download.', { v: update.version })}
        {/if}
      </span>
    </div>
    {#if update.auto}
      <button class="primary" onclick={install} disabled={phase !== 'idle' || playing} title={playing ? t('Close the game before updating the launcher') : undefined}>{t('Update')}</button>
    {:else}
      <button class="primary" onclick={download}>{t('Download')}</button>
    {/if}
    <button class="ghost" onclick={() => (app.updateDismissed = true)} disabled={phase !== 'idle'}>{t('Later')}</button>
  </div>
{/if}

<style>
  .banner { position: relative; display: flex; align-items: center; gap: 14px; padding: 10px 24px; border-bottom: 1px solid var(--line); background: var(--surface-2); flex: none; }
  .bar { position: absolute; left: 0; right: 0; bottom: -1px; border-radius: 0; }
  .ic { display: grid; place-items: center; width: 34px; height: 34px; border-radius: 50%; background: var(--inv-bg); color: var(--inv-text); flex: none; animation: nudge 1.8s var(--spring) 600ms 2; }
  .txt { display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 10px; flex: 1; min-width: 0; }
  @keyframes nudge { 30% { transform: translateY(-4px); } 60% { transform: translateY(1px); } }
</style>
