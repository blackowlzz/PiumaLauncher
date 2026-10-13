<script lang="ts">
  import { fade } from 'svelte/transition'
  import { app } from './store.svelte'
  import { t } from './i18n.svelte'

  const ask = $derived(app.confirm)
  let cancelBtn: HTMLButtonElement | undefined = $state()

  $effect(() => {
    if (ask) cancelBtn?.focus()
  })

  function onKey(e: KeyboardEvent) {
    if (!ask) return
    if (e.key === 'Escape') {
      e.preventDefault()
      ask.resolve(false)
    }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if ask}
  <div class="backdrop" role="presentation" transition:fade={{ duration: 160 }} onclick={() => ask.resolve(false)}></div>
  <div class="dialog" role="alertdialog" aria-modal="true" aria-describedby="confirm-msg">
    <p id="confirm-msg">{ask.message}</p>
    <div class="actions">
      <button bind:this={cancelBtn} class="ghost" onclick={() => ask.resolve(false)}>{t('Cancel')}</button>
      <button class="primary danger" onclick={() => ask.resolve(true)}>{ask.confirmLabel}</button>
    </div>
  </div>
{/if}

<style>
  .backdrop { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.62); z-index: 60; }
  .dialog { position: fixed; z-index: 61; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(420px, calc(100vw - 32px)); display: grid; gap: 22px; padding: 26px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 18px; box-shadow: 0 30px 90px rgba(0, 0, 0, 0.55); animation: confirm-pop 260ms var(--spring) both; }
  p { margin: 0; font-size: 15px; line-height: 1.5; overflow-wrap: anywhere; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; }
  .danger { background: #e5484d; border-color: #e5484d; color: #fff; }
  .danger:hover:not(:disabled) { background: #f0575c; }
  @keyframes confirm-pop { from { opacity: 0; transform: translate(-50%, -46%) scale(0.97); } }
</style>
