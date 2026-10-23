<script lang="ts">
  import { inview } from './actions'

  type Series = { label: string; color: string; values: number[] }

  let {
    series,
    labels,
    format,
    height = 200,
    area = false,
    label = '',
    bare = false,
  }: { series: Series[]; labels: string[]; format: (v: number) => string; height?: number; area?: boolean; label?: string; bare?: boolean } = $props()

  const uid = Math.random().toString(36).slice(2, 8)
  const PAD_X = 10
  const PAD_TOP = 14
  const PAD_BOTTOM = 8
  let width = $state(600)
  let hover: number | null = $state(null)

  const count = $derived(Math.max(1, ...series.map((s) => s.values.length)))
  const peak = $derived(Math.max(1e-9, ...series.flatMap((s) => s.values)) * 1.12)
  const x = (i: number) => (count <= 1 ? width / 2 : PAD_X + (i / (count - 1)) * (width - PAD_X * 2))
  const y = (v: number) => height - PAD_BOTTOM - (v / peak) * (height - PAD_TOP - PAD_BOTTOM)

  function line(values: number[]): string {
    const pts = values.map((v, i) => [x(i), y(v)] as const)
    if (pts.length < 2) return pts.length ? `M${pts[0][0]},${pts[0][1]}` : ''
    const floor = y(0)
    let d = `M${pts[0][0]},${pts[0][1]}`
    for (let i = 0; i < pts.length - 1; i++) {
      const p0 = pts[Math.max(0, i - 1)]
      const p1 = pts[i]
      const p2 = pts[i + 1]
      const p3 = pts[Math.min(pts.length - 1, i + 2)]
      const c1y = Math.min(floor, p1[1] + (p2[1] - p0[1]) / 6)
      const c2y = Math.min(floor, p2[1] - (p3[1] - p1[1]) / 6)
      d += ` C${p1[0] + (p2[0] - p0[0]) / 6},${c1y} ${p2[0] - (p3[0] - p1[0]) / 6},${c2y} ${p2[0]},${p2[1]}`
    }
    return d
  }

  function fill(values: number[]): string {
    if (values.length < 2) return ''
    return `${line(values)} L${x(values.length - 1)},${y(0)} L${x(0)},${y(0)} Z`
  }

  function move(e: PointerEvent) {
    const rect = (e.currentTarget as SVGElement).getBoundingClientRect()
    const rel = e.clientX - rect.left
    hover = count <= 1 ? 0 : Math.max(0, Math.min(count - 1, Math.round(((rel - PAD_X) / (width - PAD_X * 2)) * (count - 1))))
  }

  let seen = $state(false)
  const key = $derived(JSON.stringify(series.map((s) => s.values)) + width)
</script>

<div class="lc" bind:clientWidth={width} style:height="{height}px" role="img" aria-label={label} use:inview={() => (seen = true)}>
  <svg width={width} {height} viewBox="0 0 {width} {height}" role="presentation" onpointermove={move} onpointerleave={() => (hover = null)}>
    <defs>
      {#each series as s, i}
        <linearGradient id="lc-{uid}-{i}" x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stop-color={s.color} stop-opacity="0.28" />
          <stop offset="100%" stop-color={s.color} stop-opacity="0" />
        </linearGradient>
      {/each}
    </defs>
    {#if !bare}
      {#each [0.25, 0.5, 0.75] as g}
        <line class="grid" x1={PAD_X} x2={width - PAD_X} y1={y(peak * g / 1.12)} y2={y(peak * g / 1.12)} />
      {/each}
      <line class="base" x1={PAD_X} x2={width - PAD_X} y1={y(0)} y2={y(0)} />
    {/if}
    {#if seen}{#key key}
      {#each series as s, i}
        {#if area && s.values.length > 1}<path class="area" d={fill(s.values)} fill="url(#lc-{uid}-{i})" style:--delay="{i * 160 + 500}ms" />{/if}
        <path class="glow" d={line(s.values)} stroke={s.color} pathLength="1" style:--delay="{i * 160}ms" />
        <path class="line" d={line(s.values)} stroke={s.color} pathLength="1" style:--delay="{i * 160}ms" />
      {/each}
    {/key}{/if}
    {#if hover !== null}
      <line class="cursor" x1={x(hover)} x2={x(hover)} y1={PAD_TOP / 2} y2={y(0)} />
      {#each series as s}
        {#if s.values[hover] !== undefined}<circle cx={x(hover)} cy={y(s.values[hover])} r="4.5" fill={s.color} class="dot" />{/if}
      {/each}
    {/if}
  </svg>
  {#if hover !== null}
    <div class="tip" class:right={x(hover) > width * 0.66} style:left="{x(hover)}px">
      <div class="tl">{labels[hover] ?? ''}</div>
      {#each series as s}
        {#if s.values[hover] !== undefined}
          <div class="tv"><span class="sw" style:background={s.color}></span>{#if series.length > 1}<span class="sn">{s.label}</span>{/if}<b>{format(s.values[hover])}</b></div>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .lc { position: relative; width: 100%; }
  svg { display: block; overflow: visible; cursor: crosshair; }
  .grid { stroke: var(--line); stroke-dasharray: 3 5; }
  .base { stroke: var(--line-strong); }
  .line { fill: none; stroke-width: 2.25; stroke-linecap: round; stroke-linejoin: round; stroke-dasharray: 1; stroke-dashoffset: 1; animation: draw 1300ms cubic-bezier(0.3, 0.7, 0.2, 1) var(--delay) forwards; }
  .glow { fill: none; stroke-width: 9; stroke-opacity: 0.12; stroke-linecap: round; stroke-linejoin: round; stroke-dasharray: 1; stroke-dashoffset: 1; animation: draw 1300ms cubic-bezier(0.3, 0.7, 0.2, 1) var(--delay) forwards; }
  .area { opacity: 0; animation: show 700ms ease var(--delay) forwards; }
  @keyframes draw { to { stroke-dashoffset: 0; } }
  @keyframes show { to { opacity: 1; } }
  .cursor { stroke: var(--faint); stroke-dasharray: 3 3; }
  .dot { stroke: var(--surface); stroke-width: 2; }
  .tip { position: absolute; top: 0; transform: translate(-50%, -100%); margin-top: -6px; min-width: 120px; padding: 8px 11px; border-radius: 10px; background: var(--inv-bg); color: var(--inv-text); font-size: 12px; box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35); pointer-events: none; z-index: 3; white-space: nowrap; }
  .tip.right { transform: translate(-90%, -100%); }
  .tl { opacity: 0.7; margin-bottom: 3px; }
  .tl::first-letter { text-transform: uppercase; }
  .tv { display: flex; align-items: center; gap: 7px; }
  .sw { width: 8px; height: 8px; border-radius: 50%; }
  .sn { opacity: 0.75; }
  .tv b { margin-left: auto; font-weight: 650; }
  :global(:root[data-reduce]) .line, :global(:root[data-reduce]) .glow { stroke-dashoffset: 0; }
  :global(:root[data-reduce]) .area { opacity: 1; }
  @media (prefers-reduced-motion: reduce) { .line, .glow { stroke-dashoffset: 0; } .area { opacity: 1; } }
</style>
