<script lang="ts" generics="T extends string | number | null">
  import Icon from './Icon.svelte'

  type Opt = { value: T; label: string; hint?: string }
  let { value = $bindable(), options, label = '', disabled = false, onchange, width = '100%' }: {
    value: T
    options: Opt[]
    label?: string
    disabled?: boolean
    onchange?: (v: T) => void
    width?: string
  } = $props()

  let open = $state(false)
  let up = $state(false)
  let active = $state(0)
  let root: HTMLElement | undefined = $state()
  let list: HTMLElement | undefined = $state()
  const current = $derived(options.find((o) => o.value === value))

  const reveal = () => queueMicrotask(() => list?.querySelector<HTMLElement>('[data-active="true"]')?.scrollIntoView({ block: 'nearest' }))

  function show() {
    if (disabled || !root) return
    const r = root.getBoundingClientRect()
    const need = Math.min(300, options.length * 42 + 16)
    up = innerHeight - r.bottom < need && r.top > innerHeight - r.bottom
    active = Math.max(0, options.findIndex((o) => o.value === value))
    open = true
    reveal()
  }

  function choose(i: number) {
    const o = options[i]
    if (!o) return
    value = o.value
    open = false
    onchange?.(o.value)
    root?.querySelector<HTMLElement>('.trigger')?.focus()
  }

  function onKey(e: KeyboardEvent) {
    if (!open) {
      if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) {
        e.preventDefault()
        show()
      }
      return
    }
    const n = options.length
    if (e.key === 'Escape') {
      e.stopPropagation()
      open = false
    } else if (e.key === 'ArrowDown') {
      e.preventDefault()
      active = (active + 1) % n
      reveal()
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      active = (active - 1 + n) % n
      reveal()
    } else if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault()
      active = e.key === 'Home' ? 0 : n - 1
      reveal()
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      choose(active)
    } else if (e.key === 'Tab') {
      open = false
    }
  }
</script>

<svelte:window onpointerdown={(e) => open && root && !root.contains(e.target as Node) && (open = false)} />

<div class="sel" bind:this={root} style:width>
  <button type="button" class="trigger" {disabled} aria-haspopup="listbox" aria-expanded={open} aria-label={label || undefined} onclick={() => (open ? (open = false) : show())} onkeydown={onKey}>
    <span class="cur">{current?.label ?? ''}</span>
    <span class="chev" class:open><Icon name="chevron" size={14} /></span>
  </button>

  {#if open}
    <ul class="menu" class:up role="listbox" bind:this={list} aria-label={label || undefined}>
      {#each options as o, i}
        <li role="option" aria-selected={o.value === value} data-active={i === active}>
          <button type="button" tabindex="-1" class="opt" class:on={i === active} onpointerenter={() => (active = i)} onclick={() => choose(i)}>
            <span class="ol">{o.label}{#if o.hint}<span class="oh">{o.hint}</span>{/if}</span>
            {#if o.value === value}<span class="tick"><Icon name="check" size={15} /></span>{/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .sel { position: relative; display: inline-block; max-width: 100%; }
  .trigger { display: flex; align-items: center; justify-content: space-between; gap: 12px; width: 100%; padding: 10px 14px 10px 18px; text-align: left; background: var(--surface); border-color: var(--line); color: var(--text); }
  .trigger:hover:not(:disabled) { background: var(--surface-2); border-color: var(--line-strong); }
  .cur { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .chev { display: inline-grid; flex: none; color: var(--muted); transform: rotate(90deg); transition: transform 220ms var(--spring); }
  .chev.open { transform: rotate(-90deg); }
  .menu { position: absolute; left: 0; top: calc(100% + 8px); z-index: 60; min-width: 100%; width: max-content; max-width: min(420px, 90vw); max-height: 300px; overflow-y: auto; margin: 0; padding: 6px; list-style: none; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 18px; box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5); animation: pop 180ms var(--spring) both; transform-origin: top left; }
  .menu.up { top: auto; bottom: calc(100% + 8px); transform-origin: bottom left; }
  .opt { display: flex; align-items: center; justify-content: space-between; gap: 14px; width: 100%; border-color: transparent; border-radius: 12px; padding: 9px 12px; text-align: left; color: var(--text); }
  .opt.on { background: var(--surface-2); border-color: transparent; }
  .opt:hover:not(:disabled) { background: var(--surface-2); border-color: transparent; }
  .ol { display: grid; gap: 1px; min-width: 0; }
  .oh { font-size: 12px; color: var(--muted); }
  .tick { display: inline-grid; flex: none; }
  @keyframes pop { from { opacity: 0; transform: translateY(-6px) scale(0.97); } }
  .menu.up { animation-name: popUp; }
  @keyframes popUp { from { opacity: 0; transform: translateY(6px) scale(0.97); } }
</style>
