<script lang="ts">
  import mark from '../assets/piuma-ph.png'

  let { big = false }: { big?: boolean } = $props()
</script>

<div class="ph" class:big style:--m="url({mark})" aria-hidden="true"></div>

<style>
  .ph { position: absolute; inset: 0; background: var(--surface-2) var(--m) center / auto clamp(20px, 45%, 96px) no-repeat; }
  .big { background-position: 92% center; background-size: auto 62%; }
</style>
