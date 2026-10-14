<script lang="ts">
  import { onMount } from 'svelte'

  let { level }: { level: number } = $props()

  const FRAME_MS = 1000 / 30 - 2
  const CELL = 10
  let canvas: HTMLCanvasElement
  let width = 0
  let height = 0
  let live = level
  let prev = 0
  let sized = ''

  function noise(x: number, y: number, t: number) {
    return (
      Math.sin(x * 0.45 + t * 1.3) * 0.5 +
      Math.sin(x * 0.13 - t * 0.7 + y * 0.8) * 0.35 +
      Math.sin(y * 1.3 + t * 2.1 + x * 0.05) * 0.15
    ) * 0.5 + 0.5
  }

  function draw(time: number) {
    const ctx = canvas.getContext('2d')
    if (!ctx || width === 0 || height === 0) return
    const dpr = devicePixelRatio || 1
    const key = `${width}x${height}x${dpr}`
    if (key !== sized) {
      sized = key
      canvas.width = width * dpr
      canvas.height = height * dpr
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, width, height)
    const dt = Math.min(0.1, prev ? (time - prev) / 1000 : 0.016)
    prev = time
    live += (level - live) * (1 - Math.exp(-dt * 5))
    const t = (time / 1000) * 0.8
    const cols = Math.ceil(width / CELL)
    const rows = Math.ceil(height / CELL)
    for (let i = 0; i < cols; i++) {
      const x = i / cols
      const side = Math.max(0, (x - 0.25) / 0.75)
      for (let j = 0; j < rows; j++) {
        const n = noise(i, j, t)
        const vert = 1 - Math.abs(j / rows - 0.5) * 0.7
        const strength = side * vert * (0.3 + 0.7 * live)
        const fade = Math.min(1, Math.max(0, (strength * 1.5 * (0.55 + n * 0.6) - 0.12) / 0.4))
        if (fade <= 0) continue
        const light = 16 + (20 + 46 * live) * n * vert
        const alpha = (0.2 + 0.6 * n) * fade
        ctx.fillStyle = `hsla(${262 + n * 16}, ${68 + 25 * live}%, ${light}%, ${alpha})`
        ctx.fillRect(i * CELL, j * CELL, CELL - 1, CELL - 1)
      }
    }
  }

  onMount(() => {
    const ro = new ResizeObserver(([e]) => {
      width = Math.floor(e.contentRect.width)
      height = Math.floor(e.contentRect.height)
    })
    ro.observe(canvas.parentElement!)
    let raf = 0
    let drawn = 0
    const loop = (time: number) => {
      raf = requestAnimationFrame(loop)
      const still = document.documentElement.hasAttribute('data-reduce') || document.documentElement.hasAttribute('data-idle')
      if (still && Math.abs(live - level) < 0.001) {
        prev = 0
        return
      }
      if (time - drawn < FRAME_MS && Math.abs(live - level) < 0.001) return
      drawn = time
      draw(still ? 0 : time)
    }
    raf = requestAnimationFrame(loop)
    return () => {
      cancelAnimationFrame(raf)
      ro.disconnect()
    }
  })
</script>

<canvas bind:this={canvas} aria-hidden="true"></canvas>

<style>
  canvas { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; }
</style>
