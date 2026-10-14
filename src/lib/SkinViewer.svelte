<script lang="ts">
  import { onMount } from 'svelte'

  let { skin, size = 400, arrive = true, slim = false, cape = null, turn = false }: { skin: string; size?: number; arrive?: boolean; slim?: boolean; cape?: string | null; turn?: boolean } = $props()

  type V3 = [number, number, number]
  type Uv = Record<'f' | 'b' | 'r' | 'l' | 't' | 'o', [number, number]>
  interface Part { w: number; h: number; d: number; c: [number, number]; pivot: [number, number]; kind: 'head' | 'body' | 'armR' | 'armL' | 'legR' | 'legL' | 'cape'; uv: Uv; e?: number; z?: number }

  function buildParts(slimArms: boolean): Part[] {
    const aw = slimArms ? 3 : 4
    const ac = slimArms ? 5.5 : 6
    const base: Part[] = [
      { kind: 'legR', w: 4, h: 12, d: 4, c: [-2, 12], pivot: [-2, 6], uv: { f: [4, 20], b: [12, 20], r: [0, 20], l: [8, 20], t: [4, 16], o: [8, 16] } },
      { kind: 'legL', w: 4, h: 12, d: 4, c: [2, 12], pivot: [2, 6], uv: { f: [20, 52], b: [28, 52], r: [16, 52], l: [24, 52], t: [20, 48], o: [24, 48] } },
      { kind: 'body', w: 8, h: 12, d: 4, c: [0, 0], pivot: [0, 0], uv: { f: [20, 20], b: [32, 20], r: [16, 20], l: [28, 20], t: [20, 16], o: [28, 16] } },
      { kind: 'armR', w: aw, h: 12, d: 4, c: [-ac, 0], pivot: [-ac, -6], uv: slimArms ? { f: [44, 20], b: [51, 20], r: [40, 20], l: [47, 20], t: [44, 16], o: [47, 16] } : { f: [44, 20], b: [52, 20], r: [40, 20], l: [48, 20], t: [44, 16], o: [48, 16] } },
      { kind: 'armL', w: aw, h: 12, d: 4, c: [ac, 0], pivot: [ac, -6], uv: slimArms ? { f: [36, 52], b: [43, 52], r: [32, 52], l: [39, 52], t: [36, 48], o: [39, 48] } : { f: [36, 52], b: [44, 52], r: [32, 52], l: [40, 52], t: [36, 48], o: [40, 48] } },
      { kind: 'head', w: 8, h: 8, d: 8, c: [0, -10], pivot: [0, -6], uv: { f: [8, 8], b: [24, 8], r: [0, 8], l: [16, 8], t: [8, 0], o: [16, 0] } },
    ]
    const shift: Record<Exclude<Part['kind'], 'cape'>, [number, number]> = { head: [32, 0], body: [0, 16], armR: [0, 16], armL: [16, 0], legR: [0, 16], legL: [-16, 0] }
    const overlay = base.map((p): Part => {
      const [dx, dy] = shift[p.kind as keyof typeof shift]
      const uv = Object.fromEntries(Object.entries(p.uv).map(([k, [u, v]]) => [k, [u + dx, v + dy]])) as Uv
      return { ...p, uv, e: p.kind === 'head' ? 0.5 : 0.25 }
    })
    return [...base, ...overlay]
  }
  const CAPE: Part = { kind: 'cape', w: 10, h: 16, d: 1, c: [0, 2], z: -2.5, pivot: [0, -6], uv: { f: [12, 1], b: [1, 1], r: [0, 1], l: [11, 1], t: [1, 0], o: [11, 0] } }
  const PARTS_CLASSIC = buildParts(false)
  const PARTS_SLIM = buildParts(true)

  function faces(w: number, h: number, d: number, e = 0) {
    const W = w + 2 * e, Hh = h + 2 * e, D = d + 2 * e
    return {
      f: { o: [-W / 2, -Hh / 2, D / 2], u: [W, 0, 0], v: [0, Hh, 0], n: [0, 0, 1], fw: w, fh: h },
      b: { o: [W / 2, -Hh / 2, -D / 2], u: [-W, 0, 0], v: [0, Hh, 0], n: [0, 0, -1], fw: w, fh: h },
      r: { o: [-W / 2, -Hh / 2, -D / 2], u: [0, 0, D], v: [0, Hh, 0], n: [-1, 0, 0], fw: d, fh: h },
      l: { o: [W / 2, -Hh / 2, D / 2], u: [0, 0, -D], v: [0, Hh, 0], n: [1, 0, 0], fw: d, fh: h },
      t: { o: [-W / 2, -Hh / 2, -D / 2], u: [W, 0, 0], v: [0, 0, D], n: [0, -1, 0], fw: w, fh: d },
      o: { o: [-W / 2, Hh / 2, D / 2], u: [W, 0, 0], v: [0, 0, -D], n: [0, 1, 0], fw: w, fh: d },
    } as Record<keyof Uv, { o: V3; u: V3; v: V3; n: V3; fw: number; fh: number }>
  }

  const rotX = ([x, y, z]: V3, a: number): V3 => [x, y * Math.cos(a) - z * Math.sin(a), y * Math.sin(a) + z * Math.cos(a)]
  const rotY = ([x, y, z]: V3, a: number): V3 => [x * Math.cos(a) + z * Math.sin(a), y, -x * Math.sin(a) + z * Math.cos(a)]
  const LIGHT: V3 = [-0.3, -0.8, 0.52]

  let canvas: HTMLCanvasElement | undefined = $state()
  let atlas: HTMLCanvasElement | null = null
  let capeImg: HTMLImageElement | null = null
  let popAt = 0
  const W = $derived(Math.round(size * 0.82))
  const H = $derived(size)
  const pointer = { x: 0, y: 0 }

  function normalize(img: HTMLImageElement): HTMLCanvasElement {
    const cv = document.createElement('canvas')
    cv.width = cv.height = 64
    const c = cv.getContext('2d')!
    c.imageSmoothingEnabled = false
    c.drawImage(img, 0, 0)
    if (img.height <= 32) {
      const hat = c.getImageData(32, 0, 32, 16).data
      let opaque = true
      for (let i = 3; i < hat.length; i += 4) {
        if (hat[i] < 128) {
          opaque = false
          break
        }
      }
      if (opaque) c.clearRect(32, 0, 32, 16)
      const flip = (sx: number, sy: number, w: number, h: number, dx: number, dy: number) => {
        c.save()
        c.translate(dx + w, dy)
        c.scale(-1, 1)
        c.drawImage(img, sx, sy, w, h, 0, 0, w, h)
        c.restore()
      }
      for (const [sx, sy, dx, dy] of [[0, 16, 16, 48], [40, 16, 32, 48]]) {
        flip(sx + 4, sy, 4, 4, dx + 4, dy)
        flip(sx + 8, sy, 4, 4, dx + 8, dy)
        flip(sx + 8, sy + 4, 4, 12, dx, dy + 4)
        flip(sx + 4, sy + 4, 4, 12, dx + 4, dy + 4)
        flip(sx, sy + 4, 4, 12, dx + 8, dy + 4)
        flip(sx + 12, sy + 4, 4, 12, dx + 12, dy + 4)
      }
    }
    return cv
  }

  let firstSkin = true
  $effect(() => {
    const src = skin
    const img = new Image()
    img.onload = () => {
      atlas = normalize(img)
      if (firstSkin) firstSkin = false
      else popAt = performance.now()
    }
    img.src = src
  })

  $effect(() => {
    const src = cape
    if (!src) {
      capeImg = null
      return
    }
    const img = new Image()
    img.onload = () => {
      capeImg = img
    }
    img.src = src
  })

  onMount(() => {
    const ctx = canvas!.getContext('2d')!
    const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches
    const start = performance.now()
    let raf = 0
    const look = { x: 0, y: 0 }
    let turned = 0

    const move = (e: PointerEvent) => {
      const r = canvas!.getBoundingClientRect()
      pointer.x = Math.max(-1, Math.min(1, (e.clientX - (r.left + r.width / 2)) / (innerWidth / 2)))
      pointer.y = Math.max(-1, Math.min(1, (e.clientY - (r.top + r.height / 2)) / (innerHeight / 2)))
    }
    addEventListener('pointermove', move)

    const sparks = Array.from({ length: 18 }, (_, i) => {
      const a = (i / 18) * Math.PI * 2
      return { vx: Math.cos(a) * (14 + (i % 4) * 5), vy: -Math.abs(Math.sin(a)) * (14 + (i % 3) * 7) - 5, s: 0.8 + (i % 3) * 0.5 }
    })

    function frame(now: number) {
      raf = requestAnimationFrame(frame)
      if (!atlas) return
      const dpr = Math.min(2, devicePixelRatio || 1)
      if (canvas!.width !== W * dpr) {
        canvas!.width = W * dpr
        canvas!.height = H * dpr
      }
      const s = (H / 40) * dpr
      const t = (now - start) / 1000
      ctx.setTransform(1, 0, 0, 1, 0, 0)
      ctx.clearRect(0, 0, canvas!.width, canvas!.height)
      ctx.imageSmoothingEnabled = false

      const T = arrive && !reduce ? Math.min(1, t / 1.5) : 1
      const ease = 1 - Math.pow(1 - T, 3)
      let dropY = 0
      let squash = 1
      if (T < 1) {
        if (T < 0.58) dropY = -52 * Math.pow(1 - T / 0.58, 2)
        else if (T < 0.74) dropY = -2.6 * Math.sin(((T - 0.58) / 0.16) * Math.PI)
        else if (T < 0.88) dropY = -0.8 * Math.sin(((T - 0.74) / 0.14) * Math.PI)
        squash = T >= 0.5 && T < 0.64 ? 1 - 0.07 * Math.sin(((T - 0.5) / 0.14) * Math.PI) : 1
      }
      const spin = T < 1 ? -540 * (1 - ease) * (Math.PI / 180) : 0

      look.x += (pointer.x - look.x) * 0.08
      look.y += (pointer.y - look.y) * 0.08
      const idle = T >= 1 ? 1 : 0
      const bob = Math.sin(t * 1.5) * 0.3 * idle
      const swing = Math.sin(t * 1.9) * 0.22 * idle
      const pop = Math.max(0, 1 - (now - popAt) / 260)
      const popS = 1 + 0.05 * Math.sin(pop * Math.PI)
      turned += ((turn ? Math.PI : 0) - turned) * 0.12
      const yaw = (0.34 + look.x * 0.5) * (T >= 1 ? 1 : 0) + spin + turned
      const pitch = look.y * -0.16 * idle
      const ox = (W * dpr) / 2
      const oy = (H * dpr) / 2 - 2 * s

      const lift = Math.max(0, -dropY)
      const sh = Math.max(0.15, 1 - lift / 40)
      const g = ctx.createRadialGradient(ox, oy + 18.6 * s, 0, ox, oy + 18.6 * s, 10 * s * sh)
      g.addColorStop(0, `rgba(0,0,0,${0.55 * sh})`)
      g.addColorStop(1, 'rgba(0,0,0,0)')
      ctx.save()
      ctx.translate(0, 0)
      ctx.scale(1, 0.28)
      ctx.fillStyle = g
      ctx.beginPath()
      ctx.arc(ox, (oy + 18.6 * s) / 0.28, 10 * s * sh, 0, Math.PI * 2)
      ctx.fill()
      ctx.restore()

      type Item = { src: CanvasImageSource; k: number; depth: number; sx: number; sy: number; m: [number, number, number, number, number, number]; fw: number; fh: number; shade: number }
      const items: Item[] = []
      const headLook = [look.x * 0.5 * idle, look.y * -0.3 * idle]
      const place = (p0: V3, part: Part): V3 => {
        let p: V3 = [p0[0] + part.c[0], p0[1] + part.c[1], p0[2] + (part.z ?? 0)]
        const [px, py] = part.pivot
        const rotAround = (fn: (v: V3) => V3) => {
          const q = fn([p[0] - px, p[1] - py, p[2]])
          p = [q[0] + px, q[1] + py, q[2]]
        }
        if (part.kind === 'armR' || part.kind === 'legL') rotAround((v) => rotX(v, swing))
        if (part.kind === 'armL' || part.kind === 'legR') rotAround((v) => rotX(v, -swing))
        if (part.kind === 'head') rotAround((v) => rotX(rotY(v, headLook[0]), headLook[1]))
        if (part.kind === 'cape') rotAround((v) => rotX(v, -0.1 - Math.abs(swing) * 0.4))
        p = rotX(rotY(p, yaw), pitch)
        return [p[0] * popS, (p[1] + dropY + bob) * squash * popS, p[2]]
      }
      const dir = (v: V3, part: Part): V3 => {
        let n: V3 = v
        if (part.kind === 'armR' || part.kind === 'legL') n = rotX(n, swing)
        if (part.kind === 'armL' || part.kind === 'legR') n = rotX(n, -swing)
        if (part.kind === 'head') n = rotX(rotY(n, headLook[0]), headLook[1])
        if (part.kind === 'cape') n = rotX(n, -0.1 - Math.abs(swing) * 0.4)
        return rotX(rotY(n, yaw), pitch)
      }
      const parts = slim ? PARTS_SLIM : PARTS_CLASSIC
      for (const part of capeImg ? [...parts, CAPE] : parts) {
        const isCape = part.kind === 'cape'
        const fs = faces(part.w, part.h, part.d, part.e ?? 0)
        for (const k of Object.keys(fs) as (keyof Uv)[]) {
          const f = fs[k]
          const n = dir(f.n, part)
          if (n[2] <= 0.02) continue
          const O = place(f.o, part)
          const Uv = place([f.o[0] + f.u[0], f.o[1] + f.u[1], f.o[2] + f.u[2]], part)
          const Vv = place([f.o[0] + f.v[0], f.o[1] + f.v[1], f.o[2] + f.v[2]], part)
          const ux = (Uv[0] - O[0]) * s, uy = (Uv[1] - O[1]) * s
          const vx = (Vv[0] - O[0]) * s, vy = (Vv[1] - O[1]) * s
          const light = Math.max(0, n[0] * LIGHT[0] + n[1] * LIGHT[1] + n[2] * LIGHT[2])
          items.push({
            src: isCape ? capeImg! : atlas!,
            k: isCape ? capeImg!.width / 64 : 1,
            depth: O[2] + (Uv[2] - O[2] + Vv[2] - O[2]) / 2,
            sx: part.uv[k][0], sy: part.uv[k][1], fw: f.fw, fh: f.fh,
            m: [ux / f.fw, uy / f.fw, vx / f.fh, vy / f.fh, ox + O[0] * s, oy + O[1] * s],
            shade: 1 - (0.6 + 0.4 * light),
          })
        }
      }
      items.sort((a, b) => a.depth - b.depth)
      const pad = 0.05
      for (const it of items) {
        ctx.setTransform(...it.m)
        ctx.drawImage(it.src, it.sx * it.k, it.sy * it.k, it.fw * it.k, it.fh * it.k, -pad, -pad, it.fw + pad * 2, it.fh + pad * 2)
        if (it.shade > 0.01) {
          ctx.fillStyle = `rgba(0,0,0,${it.shade})`
          ctx.fillRect(-pad, -pad, it.fw + pad * 2, it.fh + pad * 2)
        }
      }

      ctx.setTransform(1, 0, 0, 1, 0, 0)
      if (arrive && !reduce) {
        const k = (t - 1.0) / 0.9
        if (k > 0 && k < 1) {
          for (const sp of sparks) {
            ctx.globalAlpha = 1 - k
            ctx.fillStyle = '#fff'
            const x = ox + sp.vx * k * s * 1.6
            const y = oy + 18 * s + (sp.vy * k + 22 * k * k) * s * 0.9
            ctx.fillRect(x, y, sp.s * s * (1 - k * 0.6), sp.s * s * (1 - k * 0.6))
          }
          ctx.globalAlpha = 1
        }
      }
    }
    raf = requestAnimationFrame(frame)
    return () => {
      cancelAnimationFrame(raf)
      removeEventListener('pointermove', move)
    }
  })
</script>

<canvas bind:this={canvas} style:width="{W}px" style:height="{H}px" aria-hidden="true"></canvas>

<style>
  canvas { display: block; }
</style>
