<script lang="ts">
  import { onMount } from 'svelte'
  import { ensureSkin, skinOf } from './skins.svelte'
  import { headCells } from './skin'

  let { name, size = 36 }: { name: string; size?: number } = $props()

  const entry = $derived(skinOf(name))
  const cells = $derived(headCells(name).flatMap((row, y) => row.map((fill, x) => ({ x, y, fill }))))
  let canvas: HTMLCanvasElement | undefined = $state()

  onMount(() => void ensureSkin(name))

  $effect(() => {
    const e = entry
    if (!canvas || e?.state !== 'ready') return
    const img = new Image()
    img.onload = () => {
      const ctx = canvas!.getContext('2d')!
      ctx.imageSmoothingEnabled = false
      ctx.clearRect(0, 0, 8, 8)
      ctx.drawImage(img, 8, 8, 8, 8, 0, 0, 8, 8)
      ctx.drawImage(img, 40, 8, 8, 8, 0, 0, 8, 8)
    }
    img.src = e.png
  })
</script>

{#if entry?.state === 'ready'}
  <canvas bind:this={canvas} width="8" height="8" style:width="{size}px" style:height="{size}px" style:border-radius="{size * 0.24}px" aria-hidden="true"></canvas>
{:else}
  <svg width={size} height={size} viewBox="0 0 8 8" shape-rendering="crispEdges" aria-hidden="true" style:border-radius="{size * 0.24}px">
    {#each cells as c}<rect x={c.x} y={c.y} width="1" height="1" fill={c.fill} />{/each}
  </svg>
{/if}

<style>
  svg, canvas { display: block; flex: none; outline: 1px solid rgba(var(--glow), 0.18); outline-offset: -1px; }
  canvas { image-rendering: pixelated; }
</style>
