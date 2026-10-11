<script lang="ts">
  let { seed, cubes = true }: { seed: string; cubes?: boolean } = $props()

  function rng(s: string) {
    let h = 2166136261
    for (const c of s) {
      h ^= c.charCodeAt(0)
      h = Math.imul(h, 16777619)
    }
    return () => {
      h ^= h << 13
      h ^= h >>> 17
      h ^= h << 5
      return ((h >>> 0) % 10000) / 10000
    }
  }

  const art = $derived.by(() => {
    const r = rng(seed || 'piuma')
    return Array.from({ length: 4 }, (_, i) => ({ x: 8 + r() * 84, y: 12 + r() * 70, s: 14 + r() * 26, dur: 7 + r() * 6, delay: -r() * 8, o: 0.16 + r() * 0.2, i }))
  })
</script>

<div class="art" aria-hidden="true">
  <div class="dots"></div>
  {#if cubes}
    {#each art as c}
      <svg class="cube" viewBox="-50 -50 100 100" style:left="{c.x}%" style:top="{c.y}%" style:width="{c.s}%" style:opacity={c.o} style:animation-duration="{c.dur}s" style:animation-delay="{c.delay}s">
        <path d="M0-40 34-20V20L0 40-34 20V-20ZM-34-20 0 0 34-20M0 0V40" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" />
      </svg>
    {/each}
  {/if}
  <div class="fade"></div>
</div>

<style>
  .art { position: absolute; inset: 0; overflow: hidden; color: var(--text); background: var(--surface); }
  .dots { position: absolute; inset: 0; background-image: radial-gradient(rgba(var(--glow), 0.16) 1px, transparent 1.2px); background-size: 18px 18px; -webkit-mask-image: radial-gradient(ellipse at 50% 40%, #000 20%, transparent 78%); mask-image: radial-gradient(ellipse at 50% 40%, #000 20%, transparent 78%); }
  .cube { position: absolute; aspect-ratio: 1; transform: translate(-50%, -50%); animation: float ease-in-out infinite alternate; }
  .fade { position: absolute; inset: 0; background: linear-gradient(to bottom, transparent 40%, var(--fade, var(--bg))); }
  @keyframes float { from { transform: translate(-50%, -50%) translateY(-8px) rotate(-3deg); } to { transform: translate(-50%, -50%) translateY(8px) rotate(3deg); } }
</style>
