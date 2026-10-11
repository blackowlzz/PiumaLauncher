<script lang="ts">
  import { slide } from 'svelte/transition'
  import Icon from './Icon.svelte'
  import { api } from './api'
  import { t } from './i18n.svelte'
  import { app, notify } from './store.svelte'

  const update = $derived(app.update)

  function download() {
    if (update) api.openUrl(update.url).catch(notify)
  }
</script>

{#if update && !app.updateDismissed}
  <div class="banner" role="status" transition:slide={{ duration: 320 }}>
    <span class="ic"><Icon name="download" size={18} /></span>
    <div class="txt">
      <strong>{t('Update available')}</strong>
      <span class="muted">{t('PiumaLauncher {v} is ready to download.', { v: update.version })}</span>
    </div>
    <button class="primary" onclick={download}>{t('Download')}</button>
    <button class="ghost" onclick={() => (app.updateDismissed = true)}>{t('Later')}</button>
  </div>
{/if}

<style>
  .banner { display: flex; align-items: center; gap: 14px; padding: 10px 24px; border-bottom: 1px solid var(--line); background: var(--surface-2); flex: none; }
  .ic { display: grid; place-items: center; width: 34px; height: 34px; border-radius: 50%; background: var(--inv-bg); color: var(--inv-text); flex: none; animation: nudge 1.8s var(--spring) 600ms 2; }
  .txt { display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 10px; flex: 1; min-width: 0; }
  @keyframes nudge { 30% { transform: translateY(-4px); } 60% { transform: translateY(1px); } }
</style>
