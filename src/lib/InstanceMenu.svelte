<script lang="ts">
  import { portal } from './actions'
  import type { Instance } from './api'
  import type { MenuEntry } from './contextmenu'
  import Icon from './Icon.svelte'
  import { instanceItems } from './instanceMenu'
  import { t } from './i18n.svelte'

  let { inst }: { inst: Instance } = $props()

  let open = $state(false)
  let x = $state(0)
  let y = $state(0)
  let trigger: HTMLButtonElement | undefined = $state()
  let menu: HTMLElement | undefined = $state()

  const items = $derived(open ? instanceItems(inst, false) : [])

  function toggle(e: MouseEvent) {
    e.stopPropagation()
    if (open) return (open = false)
    const r = trigger!.getBoundingClientRect()
    const w = 210
    const h = 270
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

  function pick(it: MenuEntry) {
    close()
    it.onclick?.()
  }
</script>

<svelte:window onclick={onWindowClick} onkeydown={(e) => e.key === 'Escape' && close()} onresize={close} />

<button class="more" bind:this={trigger} onclick={toggle} aria-haspopup="menu" aria-expanded={open} aria-label={t('More options for {name}', { name: inst.name })} title={t('More')}>
  <Icon name="dots" size={16} />
</button>

{#if open}
  <div class="menu" use:portal bind:this={menu} role="menu" style:left="{x}px" style:top="{y}px">
    {#each items as it}
      {#if it === 'sep'}
        <div class="sep"></div>
      {:else}
        <button role="menuitem" class:danger={it.danger} disabled={it.disabled} onclick={() => pick(it)}>{#if it.icon}<Icon name={it.icon} size={15} />{/if}{it.label}</button>
      {/if}
    {/each}
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
