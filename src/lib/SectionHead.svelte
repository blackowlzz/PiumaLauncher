<script lang="ts">
  import Icon from './Icon.svelte'

  let { title, back, onback }: { title: string; back: string; onback: () => void } = $props()
</script>

<header class="head">
  <button class="ghost back" onclick={onback}><span class="flip"><Icon name="chevron" size={15} /></span>{back}</button>
  <h1>{title}</h1>
</header>

<style>
  .head { display: grid; gap: 6px; }
  .back { justify-self: start; display: inline-flex; align-items: center; gap: 6px; padding: 5px 12px 5px 8px; margin-left: -8px; font-size: 13px; color: var(--muted); }
  .back:hover:not(:disabled) { color: var(--text); }
  .flip { display: inline-grid; transform: scaleX(-1); }
</style>
