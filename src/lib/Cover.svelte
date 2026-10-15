<script lang="ts">
  import { thumb } from './img'
  import Placeholder from './Placeholder.svelte'
  import type { Instance } from './api'
  import { photoFor } from './store.svelte'

  let { inst, big = false }: { inst: Pick<Instance, 'loader' | 'bannerUrl' | 'mcVersion'> & { name?: string }; big?: boolean; cubes?: boolean } = $props()

  const photo = $derived(inst.bannerUrl ?? photoFor(inst.mcVersion, inst.name ?? inst.mcVersion))
</script>

<Placeholder {big} />
{#if photo}
  <img class="photo" data-bare use:thumb={{ src: photo, w: big ? 1280 : 480 }} alt="" decoding="async" />
{/if}

<style>
  .photo { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
</style>
