<script lang="ts">
  import type { Snippet } from 'svelte'
  import Icon from './Icon.svelte'
  import { t } from './i18n.svelte'

  let { title, icon, action, onAction, cols = 'minmax(250px, 290px)', children }: {
    title: string
    icon?: 'flame' | 'box' | 'clock'
    action?: string
    onAction?: () => void
    cols?: string
    children: Snippet
  } = $props()

  let scroller: HTMLElement | undefined = $state()
  let canPrev = $state(false)
  let canNext = $state(false)

  function update() {
    if (!scroller) return
    canPrev = scroller.scrollLeft > 4
    canNext = scroller.scrollLeft + scroller.clientWidth < scroller.scrollWidth - 4
  }
  const page = (dir: 1 | -1) => scroller?.scrollBy({ left: dir * scroller.clientWidth * 0.85, behavior: 'smooth' })

  $effect(() => {
    if (!scroller) return
    update()
    const ro = new ResizeObserver(update)
    ro.observe(scroller)
    for (const c of scroller.children) ro.observe(c)
    return () => ro.disconnect()
  })
</script>

<section>
  <div class="head">
    <h2>{#if icon}<Icon name={icon} size={20} />{/if}{title}</h2>
    <div class="ctl">
      {#if action}<button class="ghost" onclick={onAction}>{action} →</button>{/if}
      <button class="arrow" onclick={() => page(-1)} disabled={!canPrev} aria-label={t('Previous')}><span class="flip"><Icon name="chevron" size={16} /></span></button>
      <button class="arrow" onclick={() => page(1)} disabled={!canNext} aria-label={t('Next')}><Icon name="chevron" size={16} /></button>
    </div>
  </div>
  <div class="frame" class:fade-l={canPrev} class:fade-r={canNext}>
    <div class="scroller" bind:this={scroller} onscroll={update} style:--cols={cols}>
      {@render children()}
    </div>
  </div>
</section>

<style>
  .head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 14px; }
  h2 { margin: 0; display: flex; align-items: center; gap: 10px; color: var(--text); font-size: 18px; font-weight: 600; letter-spacing: -0.03em; }
  .ctl { display: flex; align-items: center; gap: 8px; }
  .arrow { display: inline-grid; place-items: center; width: 34px; height: 34px; padding: 0; }
  .flip { display: inline-grid; transform: scaleX(-1); }
  .frame { position: relative; margin: 0 -4px; }
  .scroller { display: grid; grid-auto-flow: column; grid-auto-columns: var(--cols); gap: 14px; overflow-x: auto; padding: 4px 4px 14px; scroll-snap-type: x proximity; scrollbar-width: none; }
  .scroller::-webkit-scrollbar { display: none; }
  .scroller > :global(*) { scroll-snap-align: start; }
  .frame::before, .frame::after { content: ''; position: absolute; top: 0; bottom: 14px; width: 56px; pointer-events: none; z-index: 3; opacity: 0; transition: opacity 250ms; }
  .frame::before { left: 0; background: linear-gradient(to right, var(--bg), transparent); }
  .frame::after { right: 0; background: linear-gradient(to left, var(--bg), transparent); }
  .fade-l::before, .fade-r::after { opacity: 1; }
</style>
