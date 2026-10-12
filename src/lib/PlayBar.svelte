<script lang="ts">
  import Cover from './Cover.svelte'
  import AccountMenu from './AccountMenu.svelte'
  import InstancePicker from './InstancePicker.svelte'
  import { activeAccount, app, isBusy, openInstance, play, selectedInstance, stop } from './store.svelte'
  import { stageText, t } from './i18n.svelte'
  import { loaderNames } from './meta'

  const inst = $derived(selectedInstance())
  const account = $derived(activeAccount())
  const game = $derived(inst ? app.games[inst.id] : undefined)
  const busy = $derived(isBusy(inst?.id ?? null))
  const installing = $derived(app.installingId !== null)

  const percent = $derived.by(() => {
    const p = installing ? app.install : game?.state === 'preparing' ? game : null
    return p && p.total > 0 ? Math.round((p.done / p.total) * 100) : null
  })
  const status = $derived(
    installing
      ? `${app.install ? stageText(app.install.stage) : t('Installing…')}${percent !== null ? ` · ${percent}%` : ''}`
      : game?.state === 'preparing'
        ? `${game.stage ? stageText(game.stage) : t('Preparing…')}${percent !== null ? ` · ${percent}%` : ''}`
        : game?.state === 'running'
          ? t('Running')
          : '',
  )
  const working = $derived(installing || game?.state === 'preparing')
</script>

<footer class="bar">
  {#if working}
    <div class="line top" class:indeterminate={percent === null}><div style:width={percent !== null ? `${percent}%` : undefined}></div></div>
  {/if}

  <div class="side left">
    {#if inst}
      {#key inst.id}
        <button class="thumb" onclick={() => openInstance(inst.id)} title={t('Open details')} aria-label={t('Details of {name}', { name: inst.name })}>
          <Cover {inst} />
        </button>
      {/key}
      <div class="pick">
        <InstancePicker />
        <div class="sub muted small" aria-live="polite">
          {#key status}<span class="st">{#if game?.state === 'running'}<span class="dot"></span>{/if}{status || `${inst.mcVersion}${inst.loader !== 'vanilla' ? ` · ${loaderNames[inst.loader]}` : ''}`}</span>{/key}
        </div>
      </div>
    {:else}
      <span class="muted">{status || t('No instance')}</span>
    {/if}
  </div>

  {#if inst && game?.state === 'running'}
    <button class="stop go" onclick={() => stop(inst.id)}>{t('Stop')}</button>
  {:else}
    <button class="primary go" onclick={() => inst && play(inst.id)} disabled={!inst || busy || !account}>
      {game?.state === 'preparing' ? t('Preparing…') : t('Play')}
    </button>
  {/if}

  <div class="side right"><AccountMenu /></div>
</footer>

<style>
  .bar { position: relative; display: grid; grid-template-columns: 1fr auto 1fr; align-items: center; gap: 16px; padding: 0 24px; height: 84px; border-top: 1px solid var(--line); background: var(--bg); flex: none; }
  .top { position: absolute; top: -1px; left: 0; right: 0; border-radius: 0; }
  .side { display: flex; align-items: center; gap: 12px; min-width: 0; }
  .right { justify-content: flex-end; }
  .thumb { position: relative; width: 48px; height: 48px; padding: 0; border-radius: 14px; overflow: hidden; flex: none; display: grid; place-items: center; animation: gopop 320ms var(--spring) backwards; }
  .pick { display: grid; justify-items: start; min-width: 0; }
  .pick :global(.picker) { margin-left: -10px; }
  .sub { padding-left: 0; }
  .sub { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .st { display: inline-block; max-width: 100%; animation: swapin 300ms var(--spring) backwards; }
  .dot { display: inline-block; width: 6px; height: 6px; margin-right: 7px; vertical-align: 1px; border-radius: 50%; background: var(--text); animation: pulse 1.4s infinite; }
  @keyframes pulse { 50% { opacity: 0.25; } }
  .go { animation: gopop 380ms var(--spring) backwards; min-width: 230px; height: 52px; font-size: 16px; letter-spacing: -0.02em; }
  @media (max-width: 900px) { .bar { grid-template-columns: 1fr auto; } .right { display: none; } .go { min-width: 140px; } }
  @media (max-width: 560px) { .bar { padding: 0 12px; } .pick { display: none; } }
</style>
