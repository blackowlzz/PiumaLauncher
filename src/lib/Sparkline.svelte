<script lang="ts">
  import { t } from './i18n.svelte'
  let { values, limit, height = 56 }: { values: number[]; limit: number; height?: number } = $props()

  const W = 240
  const top = $derived(Math.max(limit, ...values, 1))
  const points = $derived(
    values.map((v, i) => `${(i / Math.max(values.length - 1, 1)) * W},${height - (v / top) * (height - 4) - 2}`).join(' '),
  )
</script>

<svg viewBox="0 0 {W} {height}" preserveAspectRatio="none" role="img" aria-label={t('Game memory usage over time')} style:height="{height}px">
  <line x1="0" x2={W} y1="2" y2="2" stroke="var(--line-strong)" stroke-dasharray="3 4" vector-effect="non-scaling-stroke" />
  {#if values.length > 1}
    <polygon points="0,{height} {points} {W},{height}" fill="var(--text)" opacity="0.08" />
    <polyline {points} fill="none" stroke="var(--text)" stroke-width="1.5" stroke-linejoin="round" vector-effect="non-scaling-stroke" />
  {/if}
</svg>

<style>
  svg { width: 100%; display: block; animation: draw 900ms var(--spring) both; }
  @keyframes draw { from { clip-path: inset(0 100% 0 0); } to { clip-path: inset(0 0 0 0); } }
</style>
