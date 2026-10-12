<script lang="ts">
  import Icon from './Icon.svelte'
  import { LANGUAGES, i18n, setLanguage, t } from './i18n.svelte'

  let open = $state(false)
  let root: HTMLElement | undefined = $state()
  const onWindowClick = (e: MouseEvent) => {
    if (open && root && !root.contains(e.target as Node)) open = false
  }
  const pick = (code: string) => {
    open = false
    setLanguage(code)
  }
</script>

<svelte:window onclick={onWindowClick} onkeydown={(e) => e.key === 'Escape' && (open = false)} />

<div class="wrap" bind:this={root}>
  <button class="trigger" onclick={() => (open = !open)} aria-haspopup="menu" aria-expanded={open} aria-label={t('Language')} title={t('Language')}>
    <Icon name="globe" size={17} />
    <span class="grow cur">{LANGUAGES.find((l) => l.code === i18n.lang)?.name}</span>
    <span class="chev" class:up={open}><Icon name="chevron" size={14} /></span>
  </button>
  {#if open}
    <div class="menu" role="menu">
      {#each LANGUAGES as l, n}
        <button class="opt" role="menuitemradio" aria-checked={l.code === i18n.lang} onclick={() => pick(l.code)} style:--n={n}>
          {l.name}{#if l.code === i18n.lang}<span class="tick"><Icon name="check" size={15} /></span>{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .wrap { position: relative; }
  .trigger { display: flex; align-items: center; gap: 12px; width: 100%; height: 38px; padding: 0 10px; border-radius: 12px; border-color: var(--line); background: var(--surface); color: var(--muted); font-size: 13px; text-align: left; }
  .trigger:hover:not(:disabled) { color: var(--text); }
  .cur { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .chev { display: inline-grid; transform: rotate(90deg); transition: transform 200ms var(--spring); }
  .chev.up { transform: rotate(-90deg); }
  .menu { position: absolute; left: 0; bottom: calc(100% + 10px); z-index: 30; min-width: 100%; max-height: min(70vh, 460px); overflow-y: auto; padding: 6px; display: grid; gap: 2px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 18px; box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5); animation: pop 220ms var(--spring) both; transform-origin: bottom left; }
  .opt { display: flex; align-items: center; justify-content: space-between; width: 100%; border-color: transparent; border-radius: 12px; padding: 9px 12px; text-align: left; animation: item 260ms var(--spring) both; animation-delay: calc(var(--n, 0) * 18ms); }
  .opt[aria-checked='true'] { background: var(--surface-2); }
  .tick { display: inline-grid; }
  @keyframes pop { from { opacity: 0; transform: translateY(8px) scale(0.96); } }
  @keyframes item { from { opacity: 0; transform: translateY(6px); } }
</style>
