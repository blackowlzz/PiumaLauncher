<script lang="ts">
  import { onMount } from 'svelte'
  import { app } from './store.svelte'
  import mark from '../assets/piuma.png'

  const NAME = 'PiumaLauncher'
  let visible = $state(true)
  let leaving = $state(false)

  onMount(() => {
    const root = document.documentElement
    root.setAttribute('data-intro', '')
    const started = performance.now()
    const poll = setInterval(() => {
      const waited = performance.now() - started
      if ((app.loaded && waited > 1600) || waited > 5000) {
        clearInterval(poll)
        leaving = true
        setTimeout(() => {
          visible = false
          root.removeAttribute('data-intro')
        }, 650)
      }
    }, 80)
    return () => {
      clearInterval(poll)
      root.removeAttribute('data-intro')
    }
  })
</script>

{#if visible}
  <div class="intro" class:leaving data-tauri-drag-region aria-hidden="true">
    <div class="stack">
      <div class="logo" style:--m={`url(${mark})`}>
        <span class="glow"></span>
        <span class="mark"></span>
        <span class="sheen"></span>
      </div>
      <div class="word">
        {#each NAME as ch, i}
          <span style:--d={`${450 + i * 45}ms`}>{ch}</span>
        {/each}
      </div>
      <div class="bar"><i></i></div>
    </div>
  </div>
{/if}

<style>
  .intro { position: fixed; z-index: 160; inset: 0; display: grid; place-items: center; overflow: hidden; background: var(--bg); transition: opacity 600ms ease, transform 600ms ease; }
  .intro.leaving { opacity: 0; transform: scale(1.04); pointer-events: none; }
  .stack { pointer-events: none; display: grid; justify-items: center; gap: 22px; }
  .logo { position: relative; width: 92px; height: 92px; animation: pop 900ms var(--spring) both; }
  .mark, .sheen { position: absolute; inset: 0; -webkit-mask: var(--m) center / contain no-repeat; mask: var(--m) center / contain no-repeat; }
  .mark { background: var(--text); }
  .sheen { background: linear-gradient(100deg, transparent 35%, #b79bff 50%, transparent 65%); background-size: 300% 100%; background-position: 100% 0; animation: sweep 1100ms ease-in-out 700ms both; }
  .glow { position: absolute; inset: -30px; border-radius: 50%; background: radial-gradient(circle, rgba(124, 77, 255, 0.35), transparent 65%); animation: breathe 2.4s ease-in-out infinite; }
  .word { display: flex; font-size: 26px; font-weight: 600; letter-spacing: -0.02em; }
  .word span { display: inline-block; opacity: 0; transform: translateY(14px); filter: blur(4px); animation: rise 700ms var(--spring) var(--d) both; }
  .bar { width: 120px; height: 2px; border-radius: 2px; background: var(--line); overflow: hidden; opacity: 0; animation: show 400ms ease 900ms forwards; }
  .bar i { display: block; height: 100%; width: 40%; background: #9b7bff; animation: slide 1.1s ease-in-out infinite; }
  @keyframes pop { from { opacity: 0; transform: scale(0.7) translateY(10px); } }
  @keyframes rise { to { opacity: 1; transform: none; filter: none; } }
  @keyframes sweep { to { background-position: 0% 0; } }
  @keyframes breathe { 50% { opacity: 0.5; transform: scale(1.12); } }
  @keyframes show { to { opacity: 1; } }
  @keyframes slide { from { transform: translateX(-100%); } to { transform: translateX(300%); } }
</style>
