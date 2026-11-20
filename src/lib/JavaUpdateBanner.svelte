<script lang="ts">
  import { slide } from 'svelte/transition'
  import Icon from './Icon.svelte'
  import { stageText, t } from './i18n.svelte'
  import { app, updateRuntime } from './store.svelte'

  const list = $derived(app.javaUpdates)
  const playing = $derived(Object.values(app.games).some((g) => g.state === 'running' || g.state === 'preparing'))
  const percent = $derived(app.install && app.install.total > 0 ? Math.round((app.install.done / app.install.total) * 100) : null)
  let busy = $state(false)

  async function updateAll() {
    if (busy || app.javaBusy) return
    busy = true
    for (const u of [...list]) if (!(await updateRuntime(u))) break
    busy = false
  }
</script>

{#if list.length && !app.javaUpdatesDismissed}
  <div class="banner" role="status" transition:slide={{ duration: 320 }}>
    {#if busy}<div class="line bar" class:indeterminate={percent === null}><div style:width={percent !== null ? `${percent}%` : undefined}></div></div>{/if}
    <span class="ic"><Icon name="cpu" size={18} /></span>
    <div class="txt" aria-live="polite">
      <strong>{t('New Java version available')}</strong>
      <span class="muted">
        {#if busy}
          {app.install ? stageText(app.install.stage) : t('Installing…')}{percent !== null ? ` · ${percent}%` : ''}
        {:else if list.length === 1}
          {t('{name} is out, with better performance and security fixes.', { name: `${list[0].label} ${list[0].latest ?? list[0].major}` })}
        {:else}
          {t('{n} Java versions can be updated, with better performance and security fixes.', { n: list.length })}
        {/if}
      </span>
    </div>
    <button class="primary" onclick={updateAll} disabled={busy || playing || app.javaBusy !== null} title={playing ? t('Close the game before updating Java') : undefined}>{t('Update')}</button>
    <button class="ghost" onclick={() => (app.javaUpdatesDismissed = true)} disabled={busy}>{t('Later')}</button>
  </div>
{/if}

<style>
  .banner { position: relative; display: flex; align-items: center; gap: 14px; padding: 10px 24px; border-bottom: 1px solid var(--line); background: var(--surface-2); flex: none; }
  .bar { position: absolute; left: 0; right: 0; bottom: -1px; border-radius: 0; }
  .ic { display: grid; place-items: center; width: 34px; height: 34px; border-radius: 50%; background: var(--inv-bg); color: var(--inv-text); flex: none; animation: spin-in 700ms var(--spring) 400ms both; }
  .txt { display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 10px; flex: 1; min-width: 0; }
  @keyframes spin-in { from { transform: rotate(-90deg) scale(0.6); opacity: 0; } }
</style>
