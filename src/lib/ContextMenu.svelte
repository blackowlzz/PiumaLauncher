<script lang="ts">
  import { tick } from 'svelte'
  import { portal } from './actions'
  import Icon from './Icon.svelte'
  import { t } from './i18n.svelte'
  import { app } from './store.svelte'
  import type { MenuEntry, MenuItem } from './contextmenu'

  let menu: HTMLElement | undefined = $state()
  let stack = $state<MenuItem[][]>([])
  let x = $state(0)
  let y = $state(0)

  const open = $derived(app.ctx !== null)
  const items = $derived(stack[stack.length - 1] ?? [])

  $effect(() => {
    const c = app.ctx
    stack = c ? [c.items] : []
    if (!c) return
    x = c.x
    y = c.y
    tick().then(() => {
      if (!menu) return
      const r = menu.getBoundingClientRect()
      x = Math.max(8, Math.min(c.x, innerWidth - r.width - 8))
      y = Math.max(8, Math.min(c.y, innerHeight - r.height - 8))
      menu.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus()
    })
  })

  const close = () => (app.ctx = null)

  function pick(e: MenuEntry) {
    if (e.disabled) return
    if (e.children) {
      stack = [...stack, e.children]
      return
    }
    close()
    e.onclick?.()
  }

  function keys(e: KeyboardEvent) {
    if (!open) return
    if (e.key === 'Escape') {
      e.preventDefault()
      if (stack.length > 1) stack = stack.slice(0, -1)
      else close()
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      const btns = [...(menu?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? [])]
      if (!btns.length) return
      const at = btns.indexOf(document.activeElement as HTMLButtonElement)
      const next = e.key === 'ArrowDown' ? (at + 1) % btns.length : (at - 1 + btns.length) % btns.length
      btns[next].focus()
    }
  }

  const away = (e: Event) => {
    if (open && !menu?.contains(e.target as Node)) close()
  }
</script>

<svelte:window onkeydown={keys} onmousedown={away} oncontextmenu={away} onresize={close} onblur={close} onwheel={away} />

{#if open}
  <div class="menu" use:portal bind:this={menu} role="menu" style:left="{x}px" style:top="{y}px">
    {#if stack.length > 1}
      <button role="menuitem" class="back" onclick={() => (stack = stack.slice(0, -1))}><Icon name="chevron" size={15} />{t('Back')}</button>
      <div class="sep"></div>
    {/if}
    {#each items as it}
      {#if it === 'sep'}
        <div class="sep"></div>
      {:else}
        <button role="menuitem" class:danger={it.danger} disabled={it.disabled} onclick={() => pick(it)}>
          {#if it.icon}<Icon name={it.icon} size={15} />{:else}<span class="gap"></span>{/if}
          <span class="grow">{it.label}</span>
          {#if it.children}<span class="arrow"><Icon name="chevron" size={14} /></span>{/if}
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .menu { position: fixed; z-index: 400; min-width: 220px; max-width: 320px; max-height: calc(100vh - 16px); overflow: auto; padding: 6px; display: grid; gap: 2px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 14px; box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5); animation: pop 140ms var(--spring) both; }
  .menu button { display: flex; align-items: center; gap: 10px; width: 100%; border-color: transparent; border-radius: 10px; padding: 9px 12px; text-align: left; font-size: 13.5px; }
  .menu button:focus-visible { background: var(--surface-2); outline: none; }
  .menu .danger { color: #e5484d; }
  .menu .back { color: var(--muted); }
  .menu .back :global(svg) { transform: rotate(180deg); }
  .gap { width: 15px; flex: none; }
  .grow { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .arrow { color: var(--muted); display: inline-flex; }
  .sep { height: 1px; background: var(--line); margin: 2px 4px; }
  @keyframes pop { from { opacity: 0; transform: scale(0.97); } }
</style>
