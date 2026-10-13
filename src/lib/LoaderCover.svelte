<script lang="ts">
  let { loader, big = false }: { loader: string; big?: boolean } = $props()

  const themes: Record<string, string> = {
    vanilla: 'linear-gradient(150deg, #3a8a4a, #174a2b)',
    fabric: 'linear-gradient(150deg, #eadfc2, #b7a678)',
    quilt: 'linear-gradient(150deg, #8658ff, #3b1f9c)',
    forge: 'linear-gradient(150deg, #5a6b85, #1e2a3d)',
    neoforge: 'linear-gradient(150deg, #f6a04a, #b4531a)',
  }
</script>

<div class="lc" class:big style:background={themes[loader] ?? 'linear-gradient(150deg, #3a3a3a, #161616)'} aria-hidden="true">
  <svg class="ico" viewBox="0 0 64 64">
    {#if loader === 'fabric'}
      <path d="M32 8 56 32 32 56 8 32Z" fill="#6b5a3a" />
      <path d="M32 15 49 32 32 49 15 32Z" fill="none" stroke="#eadfc2" stroke-width="1.8" stroke-dasharray="3.2 3.2" stroke-linecap="round" />
      <path d="M32 22 42 32 32 42 22 32Z" fill="#eadfc2" />
    {:else if loader === 'quilt'}
      <rect x="9" y="9" width="21" height="21" rx="4" fill="#fff" />
      <rect x="34" y="9" width="21" height="21" rx="4" fill="#fff" opacity="0.55" />
      <rect x="9" y="34" width="21" height="21" rx="4" fill="#fff" opacity="0.55" />
      <rect x="34" y="34" width="21" height="21" rx="4" fill="#fff" />
      <path d="M13 19.5h13M44.5 13v13M13 44.5h13M44.5 38v13" stroke="#8658ff" stroke-width="1.6" stroke-dasharray="2.4 2.4" stroke-linecap="round" fill="none" />
    {:else if loader === 'forge'}
      <path d="M10 18h44v8H40c0 8 4 10 4 18H20c0-8 4-10 4-18H10Z" fill="#e6ebf5" />
      <path d="M18 48h28v6H18Z" fill="#e6ebf5" opacity="0.7" />
    {:else if loader === 'neoforge'}
      <path d="M32 8 52 20v24L32 56 12 44V20Z" fill="#fff" opacity="0.92" />
      <path d="M32 18 43 25v14L32 46 21 39V25Z" fill="#b4531a" />
    {:else}
      <path d="M32 7 56 20 32 33 8 20Z" fill="#8bd05f" />
      <path d="M8 20 32 33V57L8 44Z" fill="#8a5a2e" />
      <path d="M56 20 32 33V57L56 44Z" fill="#6a4321" />
      <path d="M8 20 32 33V40L8 27Z" fill="#6fb04e" />
      <path d="M56 20 32 33V40L56 27Z" fill="#5a9a40" />
      <path d="M18 37h4v4h-4zM42 42h4v4h-4zM36 49h4v4h-4zM14 31h3v3h-3z" fill="#000" opacity="0.18" />
    {/if}
  </svg>
</div>

<style>
  .lc { position: absolute; inset: 0; display: grid; place-items: center; container-type: size; overflow: hidden; }
  .ico { width: min(58cqw, 58cqh); height: min(58cqw, 58cqh); }
  .big { place-items: center end; }
  .big .ico { width: min(86cqh, 38cqw); height: min(86cqh, 38cqw); margin-right: 8cqw; opacity: 0.95; }
</style>
