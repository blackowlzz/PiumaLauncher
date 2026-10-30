<script lang="ts">
  import { onMount } from 'svelte'
  import { app } from './store.svelte'
  import mark from '../assets/piuma.png'

  const NAME = 'PiumaLauncher'
  const MIN_MS = 1100
  const MAX_MS = 4000
  const FADE_MS = 360
  let visible = $state(true)
  let leaving = $state(false)
  let settled = $state(false)
  let timedOut = $state(false)
  const done = $derived(timedOut || (app.loaded && (settled || app.settings.reduceEffects)))

  $effect(() => {
    if (!done || leaving) return
    leaving = true
    setTimeout(() => {
      visible = false
      document.documentElement.removeAttribute('data-intro')
    }, FADE_MS)
  })

  onMount(() => {
    const root = document.documentElement
    root.setAttribute('data-intro', '')
    const min = setTimeout(() => (settled = true), MIN_MS)
    const max = setTimeout(() => (timedOut = true), MAX_MS)
    return () => {
      clearTimeout(min)
      clearTimeout(max)
      root.removeAttribute('data-intro')
    }
  })
</script>

{#if visible}
  <div class="intro" class:leaving style:--fade={`${FADE_MS}ms`} style:--wait={`${MIN_MS + 50}ms`} data-tauri-drag-region aria-hidden="true">
    <div class="stack">
      <div class="logo">
        <span class="glow"></span>
        <span class="mark" style:--m={`url(${mark})`}><i></i></span>
      </div>
      <div class="word">
        {#each NAME as ch, i}
          <span style:--d={`${160 + i * 28}ms`}>{ch}</span>
        {/each}
      </div>
      <div class="bar"><i></i></div>
    </div>
  </div>
{/if}

<style>
  .intro { position: fixed; z-index: 160; inset: 0; display: grid; place-items: center; overflow: hidden; background: var(--bg); transition: opacity var(--fade) ease; }
  .intro.leaving { opacity: 0; pointer-events: none; }
  .stack { pointer-events: none; display: grid; justify-items: center; gap: 22px; }
  .logo { position: relative; width: 92px; height: 92px; animation: pop 520ms var(--spring) both; }
  .mark { position: absolute; inset: 0; overflow: hidden; background: var(--text); -webkit-mask: var(--m) center / contain no-repeat; mask: var(--m) center / contain no-repeat; }
  .mark i { position: absolute; inset: 0; background: linear-gradient(100deg, transparent 30%, #b79bff 50%, transparent 70%); transform: translateX(-100%); animation: sweep 760ms ease-in-out 380ms forwards; }
  .glow { position: absolute; inset: -30px; border-radius: 50%; background: radial-gradient(circle, rgba(124, 77, 255, 0.35), transparent 65%); animation: breathe 2.4s ease-in-out infinite; }
  .word { display: flex; font-size: 26px; font-weight: 600; letter-spacing: -0.02em; }
  .word span { display: inline-block; animation: rise 520ms var(--spring) var(--d) both; }
  .bar { width: 120px; height: 2px; border-radius: 2px; background: var(--line); overflow: hidden; animation: show 300ms ease var(--wait) both; }
  .bar i { display: block; height: 100%; width: 40%; background: #9b7bff; animation: slide 1.1s ease-in-out infinite; }
  @keyframes pop { from { opacity: 0; transform: scale(0.82) translateY(8px); } }
  @keyframes rise { from { opacity: 0; transform: translateY(12px); } }
  @keyframes sweep { to { transform: translateX(100%); } }
  @keyframes breathe { 50% { opacity: 0.5; transform: scale(1.12); } }
  @keyframes show { from { opacity: 0; } }
  @keyframes slide { from { transform: translateX(-100%); } to { transform: translateX(300%); } }
</style>
