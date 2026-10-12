<script lang="ts">
  import { onMount } from 'svelte'

  let { value, min, max, step, label, oninput, onchange }: {
    value: number
    min: number
    max: number
    step: number
    label: string
    oninput: (v: number) => void
    onchange: (v: number) => void
  } = $props()

  const ratio = $derived(max > min ? Math.min(1, Math.max(0, (value - min) / (max - min))) : 0)

  const CELL = 9
  const HEIGHT = 54
  let host: HTMLDivElement
  let canvas: HTMLCanvasElement
  let width = 0
  let live = ratio
  let prev = 0
  let sized = ''

  function noise(x: number, y: number, t: number) {
    return (
      Math.sin(x * 0.55 + t * 1.3) * 0.5 +
      Math.sin(x * 0.17 - t * 0.7 + y * 0.9) * 0.35 +
      Math.sin(y * 1.7 + t * 2.1 + x * 0.05) * 0.15
    ) * 0.5 + 0.5
  }

  function draw(time: number) {
    const ctx = canvas.getContext('2d')
    if (!ctx || width === 0) return
    const dpr = devicePixelRatio || 1
    const key = `${width}x${dpr}`
    if (key !== sized) {
      sized = key
      canvas.width = width * dpr
      canvas.height = HEIGHT * dpr
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, width, HEIGHT)
    const dt = Math.min(0.1, prev ? (time - prev) / 1000 : 0.016)
    prev = time
    live += (ratio - live) * (1 - Math.exp(-dt * 9))
    const r = live
    const t = time / 1000 * 0.8
    const fillW = r * width
    const cols = Math.ceil(fillW / CELL)
    const rows = Math.floor(HEIGHT / CELL)
    const mid = (rows - 1) / 2
    for (let i = 0; i < cols; i++) {
      const edge = 1 - Math.max(0, (i * CELL - (fillW - CELL * 5)) / (CELL * 5)) * 0.5
      for (let j = 0; j < rows; j++) {
        const reach = 0.25 + 0.75 * noise(i, 0, t) * (0.4 + r * 0.6)
        const d = Math.abs(j - mid) / (mid + 0.5)
        const fade = Math.min(1, Math.max(0, (reach - d) / 0.22 + 0.5))
        if (fade <= 0) continue
        const n = noise(i, j, t)
        const light = 18 + (22 + 50 * r) * n * (1 - d * 0.55) * edge
        const alpha = (0.35 + 0.65 * Math.min(1, n * 1.2 + r * 0.35)) * fade
        ctx.fillStyle = `hsla(${262 + n * 14}, ${70 + 25 * r}%, ${light}%, ${alpha})`
        ctx.fillRect(i * CELL, j * CELL + (HEIGHT - rows * CELL) / 2, CELL - 1, CELL - 1)
      }
    }
    if (r > 0.9) {
      const glow = ctx.createLinearGradient(fillW - 60, 0, fillW, 0)
      glow.addColorStop(0, 'rgba(180,140,255,0)')
      glow.addColorStop(1, `rgba(200,170,255,${(r - 0.9) * 4})`)
      ctx.fillStyle = glow
      ctx.fillRect(Math.max(0, fillW - 60), 0, Math.min(60, fillW), HEIGHT)
    }
  }

  onMount(() => {
    const ro = new ResizeObserver(([e]) => (width = Math.floor(e.contentRect.width)))
    ro.observe(host)
    let raf = 0
    const loop = (time: number) => {
      raf = requestAnimationFrame(loop)
      const still = document.documentElement.hasAttribute('data-reduce') || document.documentElement.hasAttribute('data-idle')
      if (still && Math.abs(live - ratio) < 0.001) {
        prev = 0
        return
      }
      draw(still ? 0 : time)
    }
    raf = requestAnimationFrame(loop)
    return () => {
      cancelAnimationFrame(raf)
      ro.disconnect()
    }
  })
</script>

<div class="slider" bind:this={host} style:--p={`${ratio * 100}%`}>
  <canvas bind:this={canvas} style:height={`${HEIGHT}px`} aria-hidden="true"></canvas>
  <div class="rail"></div>
  <div class="mark" aria-hidden="true"></div>
  <input
    type="range"
    {min}
    {max}
    {step}
    {value}
    aria-label={label}
    oninput={(e) => oninput(Number(e.currentTarget.value))}
    onchange={(e) => onchange(Number(e.currentTarget.value))}
  />
</div>

<style>
  .slider { position: relative; width: 100%; height: 54px; }
  canvas { position: absolute; inset: 0; width: 100%; }
  .rail { position: absolute; left: 0; right: 0; top: 50%; height: 1px; background: var(--line-strong); }
  .mark { position: absolute; top: 50%; left: var(--p); transition: left 120ms ease-out; width: 3px; height: 30px; margin: -15px 0 0 -3px; border-radius: 2px; background: #c9b2ff; box-shadow: 0 0 12px 2px rgba(155, 107, 255, 0.7); pointer-events: none; }
  input { position: absolute; inset: 0; width: 100%; height: 100%; margin: 0; padding: 0; border: 0; background: transparent; appearance: none; -webkit-appearance: none; cursor: pointer; opacity: 0; }
  input:hover, input:focus { background: transparent; border: 0; }
  input::-webkit-slider-runnable-track { height: 54px; background: transparent; }
  input::-webkit-slider-thumb { -webkit-appearance: none; appearance: none; width: 18px; height: 54px; }
  .slider:has(input:focus-visible) .mark { outline: 2px solid var(--text); outline-offset: 3px; }
</style>
