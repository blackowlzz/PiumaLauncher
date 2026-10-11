<script lang="ts">
  import Icon from './Icon.svelte'
  import { t } from './i18n.svelte'
  import { app, isBusy, selectedInstance } from './store.svelte'

  let open = $state(false)
  let root: HTMLElement | undefined = $state()
  const inst = $derived(selectedInstance())

  function choose(id: string) {
    app.selectedId = id
    open = false
  }
  const onWindowClick = (e: MouseEvent) => {
    if (open && root && !root.contains(e.target as Node)) open = false
  }
</script>

<svelte:window onclick={onWindowClick} onkeydown={(e) => e.key === 'Escape' && (open = false)} />

<div class="picker" bind:this={root}>
  <button class="trigger ghost" onclick={() => (open = !open)} disabled={isBusy(inst?.id ?? null)} aria-haspopup="listbox" aria-expanded={open} aria-label={t('Change instance')}>
    <span class="name">{inst?.name}</span>
    <span class="chev" class:up={open}><Icon name="chevron" size={14} /></span>
  </button>
  {#if open}
    <ul class="menu" role="listbox" aria-label={t('Instances')}>
      {#each app.instances as i, n (i.id)}
        <li role="option" aria-selected={i.id === app.selectedId} style:--n={n}>
          <button class="opt" class:sel={i.id === app.selectedId} onclick={() => choose(i.id)}>
            <span class="on">{i.id === app.selectedId ? '●' : ''}</span>{i.name}<span class="muted small">{i.mcVersion}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .picker { position: relative; }
  .trigger { display: inline-flex; align-items: center; gap: 6px; padding: 4px 10px; font-weight: 600; font-size: 14px; color: var(--text); }
  .name { max-width: 200px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .chev { display: inline-grid; color: var(--muted); transform: rotate(90deg); transition: transform 250ms var(--spring); }
  .chev.up { transform: rotate(-90deg); }
  .menu { position: absolute; bottom: calc(100% + 10px); left: 0; z-index: 30; min-width: 240px; max-height: 320px; overflow: auto; margin: 0; padding: 6px; list-style: none; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 18px; box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5); animation: pop 220ms var(--spring) both; transform-origin: bottom left; }
  .menu li { animation: item 260ms var(--spring) both; animation-delay: calc(var(--n) * 30ms); }
  .opt { display: flex; align-items: center; gap: 8px; width: 100%; border-color: transparent; text-align: left; padding: 9px 12px; border-radius: 12px; }
  .opt .small { margin-left: auto; }
  .on { width: 10px; font-size: 8px; }
  .opt.sel { background: var(--surface-2); }
  @keyframes pop { from { opacity: 0; transform: translateY(8px) scale(0.96); } }
  @keyframes item { from { opacity: 0; transform: translateY(6px); } }
</style>
