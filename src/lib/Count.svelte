<script lang="ts">
  import { tweened } from 'svelte/motion'
  import { cubicOut } from 'svelte/easing'

  let { value, format = (n: number) => String(Math.round(n)) }: { value: number; format?: (n: number) => string } = $props()
  const t = tweened(0, { duration: 1000, easing: cubicOut })
  $effect(() => {
    t.set(value)
  })
</script>

<span>{format($t)}</span>
