<script lang="ts">
  import { portal } from './actions'
  import { api, type Instance } from './api'
  import Icon from './Icon.svelte'
  import { t } from './i18n.svelte'
  import { app, askConfirm, isBusy, notify, openInstance, reloadInstances } from './store.svelte'

  let { inst }: { inst: Instance } = $props()

  let open = $state(false)
  let x = $state(0)
  let y = $state(0)
  let trigger: HTMLButtonElement | undefined = $state()
  let menu: HTMLElement | undefined = $state()

  const locked = $derived(isBusy(inst.id) || app.games[inst.id]?.state === 'running')

  function toggle(e: MouseEvent) {
    e.stopPropagation()
    if (open) return (open = false)
    const r = trigger!.getBoundingClientRect()
    const w = 210
    const h = 190
    x = Math.max(8, Math.min(innerWidth - w - 8, r.right - w))
    y = innerHeight - r.bottom < h + 8 ? Math.max(8, r.top - h - 6) : r.bottom + 6
    open = true
  }

  const close = () => (open = false)
  const onWindowClick = (e: MouseEvent) => {
    if (!open) return
    const n = e.target as Node
    if (!menu?.contains(n) && !trigger?.contains(n)) open = false
  }

  async function duplicate() {
    close()
    try {
      const copy = await api.duplicateInstance(inst.id)
      await reloadInstances()
      notify(t('Created "{name}"', { name: copy.name }))
    } catch (e) {
      notify(e)
    }
  }

  async function remove() {
    close()
    if (!(await askConfirm(t('Delete "{name}" with all its worlds and mods?', { name: inst.name })))) return
    try {
      await api.deleteInstance(inst.id)
      if (app.selectedId === inst.id) app.selectedId = null
      await reloadInstances()
    } catch (e) {
      notify(e)
    }
  }

  function folder() {
    close()
    api.openFolder(inst.id, 'game').catch(notify)
  }
</script>

<svelte:window onclick={onWindowClick} onkeydown={(e) => e.key === 'Escape' && close()} onresize={close} />

<button class="more" bind:this={trigger} onclick={toggle} aria-haspopup="menu" aria-expanded={open} aria-label={t('More options for {name}', { name: inst.name })} title={t('More')}>
  <Icon name="dots" size={16} />
</button>

{#if open}
  <div class="menu" use:portal bind:this={menu} role="menu" style:left="{x}px" style:top="{y}px">
    <button role="menuitem" onclick={() => { close(); openInstance(inst.id) }}><Icon name="settings" size={15} />{t('Details')}</button>
    <button role="menuitem" onclick={folder}><Icon name="folder" size={15} />{t('Open instance folder')}</button>
    <button role="menuitem" onclick={duplicate} disabled={locked}><Icon name="file" size={15} />{t('Duplicate')}</button>
    <div class="sep"></div>
    <button role="menuitem" class="danger" onclick={remove} disabled={locked}><Icon name="trash" size={15} />{t('Delete instance')}</button>
  </div>
{/if}

<style>
  .more { width: 30px; height: 30px; padding: 0; display: inline-grid; place-items: center; border-radius: 50%; border-color: transparent; color: var(--muted); flex: none; }
  .menu { position: fixed; z-index: 300; width: 210px; padding: 6px; display: grid; gap: 2px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 14px; box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5); animation: pop 160ms var(--spring) both; }
  .menu button { display: flex; align-items: center; gap: 10px; width: 100%; border-color: transparent; border-radius: 10px; padding: 9px 12px; text-align: left; font-size: 13.5px; }
  .menu .danger { color: #e5484d; }
  .sep { height: 1px; background: var(--line); margin: 2px 4px; }
  @keyframes pop { from { opacity: 0; transform: translateY(-4px) scale(0.97); } }
</style>
