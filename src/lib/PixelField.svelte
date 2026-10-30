<script lang="ts">
  import { onMount } from 'svelte'

  let { level }: { level: number } = $props()

  const FRAME_MS = 1000 / 30 - 2
  const CELL = 10
  const SPEED = 1.6
  let canvas: HTMLCanvasElement
  let width = 0
  let height = 0
  let live = level
  let prev = 0
  let sized = ''

  const hash = (n: number) => {
    const s = Math.sin(n * 127.1 + 311.7) * 43758.5453
    return s - Math.floor(s)
  }

  function smooth(x: number) {
    const i = Math.floor(x)
    const f = x - i
    const u = f * f * (3 - 2 * f)
    return hash(i) * (1 - u) + hash(i + 1) * u
  }

  const ridge = (x: number) => smooth(x * 0.09) * 0.65 + smooth(x * 0.23 + 40) * 0.25 + smooth(x * 0.6 + 90) * 0.1

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
    const t = time / 1000
    const tint = getComputedStyle(canvas).getPropertyValue('--inv-bg').trim()
    const rgb = /^#[0-9a-f]{6}$/i.test(tint) ? [1, 3, 5].map((k) => parseInt(tint.slice(k, k + 2), 16)).join(', ') : '250, 250, 250'
    const cols = Math.ceil(width / CELL) + 1
    const rows = Math.ceil(height / CELL)
    const cell = (x: number, y: number, a: number) => {
      ctx.fillStyle = `rgba(${rgb}, ${a})`
      ctx.fillRect(x, y, CELL - 1, CELL - 1)
    }

    const cloudScroll = t * SPEED * 0.35
    const cloudShift = (cloudScroll % 1) * CELL
    const scroll = t * SPEED
    const shift = (scroll % 1) * CELL
    for (let i = 0; i < cols; i++) {
      const side = Math.min(1, Math.max(0, ((i * CELL) / width - 0.42) / 0.3))
      if (side <= 0) continue

      const cc = i + Math.floor(cloudScroll)
      const puff = smooth(cc * 0.18 + 7)
      if (puff > 0.62) {
        const top = 1 + Math.floor(hash(Math.floor(cc / 6)) * 2)
        const thick = puff > 0.75 ? 2 : 1
        for (let j = top; j < top + thick && j < rows; j++) cell(i * CELL - cloudShift, j * CELL, 0.1 * side)
      }

      const c = i + Math.floor(scroll)
      const ground = Math.round(rows * (0.86 - (0.14 + 0.3 * live) * ridge(c)))
      const x = i * CELL - shift
      for (let j = Math.max(0, ground); j < rows; j++) {
        const depth = j - ground
        let a = depth === 0 ? 0.62 : depth < 3 ? 0.34 : 0.18
        if (depth >= 3 && hash(c * 31.7 + j * 7.3) > 0.94) a = 0.3 + 0.35 * (0.5 + 0.5 * Math.sin(t * 2.4 + c * 1.7 + j))
        cell(x, j * CELL, a * side * (0.6 + 0.4 * live))
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
