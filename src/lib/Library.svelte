<script lang="ts">
  import Art from './Art.svelte'
  import Cover from './Cover.svelte'
  import Icon from './Icon.svelte'
  import { reveal } from './actions'
  import { t } from './i18n.svelte'
  import InstanceMenu from './InstanceMenu.svelte'
  import { ctx } from './contextmenu'
  import { instanceItems } from './instanceMenu'
  import NewInstanceModal from './NewInstanceModal.svelte'
  import ImportModal from './ImportModal.svelte'
  import MigrateModal from './MigrateModal.svelte'
  import Select from './Select.svelte'
  import { app, isBusy, isInstalled, openInstance, play, playInfo } from './store.svelte'
  import { formatDuration, loaderNames, relativeTime } from './meta'
  import type { Instance } from './api'

  let query = $state('')
  let sort: 'recent' | 'name' | 'time' = $state('recent')
  let view: 'grid' | 'list' = $state('grid')
  let creating = $state(false)
  let importing = $state(false)
  let migrating = $state(false)

  const rows = $derived.by(() => {
    const q = query.trim().toLowerCase()
    return app.instances
      .filter((i) => !q || `${i.name} ${i.mcVersion} ${i.loader}`.toLowerCase().includes(q))
      .map((i) => ({ inst: i, ...playInfo(i.id) }))
      .sort((a, b) =>
        sort === 'name' ? a.inst.name.localeCompare(b.inst.name) : sort === 'time' ? b.secs - a.secs : b.last - a.last || a.inst.name.localeCompare(b.inst.name),
      )
  })

  const status = (i: Instance, last: number) =>
    !isInstalled(i.mcVersion) ? t('To install') : last ? relativeTime(last) : t('Never played')
</script>

<header class="head">
  <div class="grow">
    <h1>{t('Library')}</h1>
    <p class="muted">{t('Manage your Minecraft instances.')}</p>
  </div>
  <div class="hacts">
    <button onclick={() => (importing = true)}><Icon name="download" size={15} />{t('Import')}</button>
    <button class="primary" onclick={() => (creating = true)}>{t('New instance')}</button>
  </div>
</header>
{#if importing}<ImportModal onClose={() => (importing = false)} onMigrate={() => ((importing = false), (migrating = true))} />{/if}
{#if migrating}<MigrateModal onClose={() => (migrating = false)} />{/if}

<div class="toolbar">
  <label class="search grow">
    <span class="sr">{t('Search instances')}</span>
    <span class="field"><Icon name="search" size={16} /><input type="search" bind:value={query} placeholder={t('Search instances…')} /></span>
  </label>
  <Select
    bind:value={sort}
    width="auto"
    label={t('Sort by')}
    options={[
      { value: 'recent', label: t('Last played') },
      { value: 'name', label: t('Name') },
      { value: 'time', label: t('Playtime') },
    ]}
  />
  <div class="seg" role="group" aria-label={t('View')}>
    <button class="iconbtn" aria-pressed={view === 'grid'} aria-label={t('Grid')} onclick={() => (view = 'grid')}><Icon name="layout" size={16} /></button>
    <button class="iconbtn" aria-pressed={view === 'list'} aria-label={t('List')} onclick={() => (view = 'list')}><Icon name="list" size={16} /></button>
  </div>
</div>

{#if !app.instances.length}
  <div class="empty">
    <p class="muted">{t('No instances yet.')}</p>
    <button class="primary" onclick={() => (creating = true)}>{t('Create the first one')}</button>
  </div>
{:else if !rows.length}
  <p class="muted">{t('No instance matches "{query}".', { query })}</p>
{:else if view === 'grid'}
  <div class="grid">
    {#each rows as { inst, secs, last }, n (inst.id)}
      {@const state = app.games[inst.id]?.state}
      <article class="card" class:sel={app.selectedId === inst.id} use:reveal={n} use:ctx={() => instanceItems(inst)}>
        <button class="open" onclick={() => openInstance(inst.id)} aria-label={t('Open {name}', { name: inst.name })}>
          <div class="cover">
            <Cover {inst} />
            <span class="when"><Icon name="clock" size={12} />{status(inst, last)}</span>
            {#if state === 'running' || state === 'preparing'}
              <span class="live"><span class="dot"></span>{state === 'running' ? t('Running') : t('Preparing')}</span>
            {/if}
          </div>
          <div class="body">
            <div class="title">{inst.name}</div>
            <div class="meta">
              <span class="tag">{loaderNames[inst.loader]}</span>
              <span class="tag">{inst.mcVersion}</span>
              {#if secs}<span class="muted small">{formatDuration(secs)}</span>{/if}
            </div>
          </div>
        </button>
        <span class="dots"><InstanceMenu {inst} /></span>
        <button class="quick" onclick={() => play(inst.id)} disabled={isBusy(inst.id)} aria-label={t('Play {name}', { name: inst.name })} title={t('Play')}><Icon name="play" size={16} /></button>
      </article>
    {/each}
  </div>
{:else}
  <div class="list">
    {#each rows as { inst, secs, last }, n (inst.id)}
      <div class="item row" use:reveal={n} use:ctx={() => instanceItems(inst)}>
        <button class="ghost rowopen grow" onclick={() => openInstance(inst.id)}>
          <span class="thumb"><Cover {inst} /></span>
          <span class="rt"><span class="n">{inst.name}</span><span class="muted small">{status(inst, last)}{secs ? ` · ${formatDuration(secs)}` : ''}</span></span>
          <span class="tag">{loaderNames[inst.loader]}</span>
          <span class="tag">{inst.mcVersion}</span>
        </button>
        <button onclick={() => play(inst.id)} disabled={isBusy(inst.id)}>{t('Play')}</button>
        <InstanceMenu {inst} />
      </div>
    {/each}
  </div>
{/if}

{#if creating}<NewInstanceModal onClose={() => (creating = false)} />{/if}

<style>
  .hacts { display: flex; gap: 8px; }
  .hacts button { display: inline-flex; align-items: center; gap: 8px; }
  .head { display: flex; align-items: flex-start; gap: 16px; }
  .head p { margin-top: 6px; }
  .toolbar { display: flex; flex-wrap: wrap; gap: 12px; align-items: center; }
  .search { min-width: 220px; }
  .field { position: relative; display: block; }
  .field :global(svg) { position: absolute; left: 11px; top: 50%; transform: translateY(-50%); color: var(--faint); pointer-events: none; }
  .field input { padding-left: 34px; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
  .empty { display: grid; gap: 12px; justify-items: start; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 16px; }
  .card { position: relative; }
  .open { transition: transform 380ms var(--spring), border-color var(--ease); }
  .card:hover .open { transform: translateY(-4px); }
  .cover :global(.photo) { transition: transform 900ms var(--spring); }
  .card:hover .cover :global(.photo) { transform: scale(1.06); }
  .open { display: block; width: 100%; padding: 0; text-align: left; border-radius: 22px; border: 1px solid var(--line); background: var(--surface); overflow: hidden; }
  .open:hover:not(:disabled) { border-color: var(--line-strong); background: var(--surface); }
  .card.sel .open { border-color: var(--text); }
  .cover { --fade: var(--surface); position: relative; overflow: hidden; aspect-ratio: 16 / 9; display: grid; place-items: center; }
  .when { position: absolute; left: 12px; bottom: 10px; display: inline-flex; align-items: center; gap: 5px; font-size: 11px; color: #fff; background: rgba(0, 0, 0, 0.55); border-radius: 999px; padding: 3px 10px; }
  .dots { position: absolute; right: 8px; bottom: 8px; }
  .body { padding: 12px 46px 14px 14px; display: grid; gap: 8px; }
  .title { font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .meta { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  .live { position: absolute; left: 10px; top: 10px; display: inline-flex; align-items: center; gap: 6px; font-size: 11px; background: var(--bg); border-radius: 999px; padding: 3px 9px; }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--text); animation: pulse 1.4s infinite; }
  @keyframes pulse { 50% { opacity: 0.25; } }
  .quick { position: absolute; right: 10px; top: 10px; width: 34px; height: 34px; padding: 0; display: grid; place-items: center; border-radius: 50%; background: var(--inv-bg); color: var(--inv-text); border-color: var(--inv-bg); opacity: 0; transform: translateY(-4px); transition: opacity var(--ease), transform var(--ease); }
  .quick:hover:not(:disabled) { background: var(--inv-bg); border-color: var(--inv-bg); opacity: 1; }
  .card:hover .quick, .quick:focus-visible { opacity: 1; transform: none; }
  .quick:disabled { display: none; }
  .rowopen { display: flex; align-items: center; gap: 12px; text-align: left; padding: 4px 8px; }
  .thumb { position: relative; width: 40px; height: 40px; border-radius: 10px; overflow: hidden; flex: none; display: grid; place-items: center; border: 1px solid var(--line); }
  .rt { display: grid; flex: 1; min-width: 0; }
  .n { font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
