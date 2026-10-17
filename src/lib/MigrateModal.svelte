<script lang="ts">
  import { onMount } from 'svelte'
  import { fade } from 'svelte/transition'
  import { api, type FoundInstance, type LauncherSource, type Loader } from './api'
  import { app, notify, reloadInstances } from './store.svelte'
  import { stageText, t } from './i18n.svelte'
  import Icon from './Icon.svelte'
  import Select from './Select.svelte'
  import { loaderNames } from './meta'

  let { onClose }: { onClose: () => void } = $props()

  type Row = FoundInstance & { picked: boolean; version: string; kind: Loader; done: boolean }

  const sourceName = (s: LauncherSource) =>
    ({
      prism: 'Prism Launcher',
      polymc: 'PolyMC',
      multimc: 'MultiMC',
      curseforge: 'CurseForge',
      modrinth: 'Modrinth App',
      atlauncher: 'ATLauncher',
      official: 'Minecraft Launcher',
      folder: t('Folder'),
    })[s]

  let rows: Row[] = $state([])
  let scanning = $state(true)
  let folder = $state('')
  let searched: string | undefined = $state()
  let running = $state(false)
  let current: string | null = $state(null)

  const groups = $derived.by(() => {
    const out: { source: LauncherSource; rows: Row[] }[] = []
    for (const r of rows) {
      const g = out.find((x) => x.source === r.source)
      if (g) g.rows.push(r)
      else out.push({ source: r.source, rows: [r] })
    }
    return out
  })
  const picked = $derived(rows.filter((r) => r.picked && !r.done))
  const ready = $derived(picked.length > 0 && picked.every((r) => r.version) && !running)
  const stage = $derived(running && app.install ? stageText(app.install.stage) : '')
  const percent = $derived(running && app.install && app.install.total > 0 ? Math.round((app.install.done / app.install.total) * 100) : null)

  const versionOptions = $derived([
    { value: '', label: t('Choose…') },
    ...app.versions.filter((v) => v.kind === 'release' || v.kind === 'snapshot').map((v) => ({ value: v.id, label: v.id })),
  ])
  const loaderOptions = (Object.keys(loaderNames) as Loader[]).map((l) => ({ value: l, label: loaderNames[l] }))

  const count = (n: number, one: string, many: string) => (n === 1 ? t(one) : t(many, { n }))

  async function scan(dir?: string) {
    if (running) return
    scanning = true
    try {
      const found = await api.scanLaunchers(dir)
      rows = found.map((f) => ({ ...f, picked: !!f.mcVersion && !f.shared, version: f.mcVersion ?? '', kind: f.loader, done: false }))
      searched = dir
    } catch (e) {
      notify(e)
    } finally {
      scanning = false
    }
  }

  async function run() {
    if (!ready) return
    running = true
    app.installingId = 'import'
    let last: string | null = null
    let imported = 0
    const failed: string[] = []
    for (const r of picked) {
      current = r.key
      app.install = null
      try {
        const inst = await api.importFromLauncher(
          { key: r.key, mcVersion: r.version, loader: r.kind, loaderVersion: r.kind === r.loader ? r.loaderVersion : null },
          searched,
        )
        last = inst?.id ?? last
        r.done = true
        imported++
      } catch (e) {
        failed.push(`${r.name}: ${e}`)
      }
    }
    current = null
    running = false
    app.installingId = null
    app.install = null
    await reloadInstances().catch(() => {})
    if (last) app.selectedId = last
    if (failed.length) {
      notify(failed.length === 1 ? failed[0] : `${t('{n} imports failed', { n: failed.length })} · ${failed[0]}`)
      return
    }
    notify(imported === 1 ? t('1 instance imported') : t('{n} instances imported', { n: imported }))
    onClose()
  }

  const close = () => !running && onClose()
  onMount(() => scan())
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && close()} />

<div class="backdrop" role="presentation" transition:fade={{ duration: 160 }} onclick={close}></div>
<div class="modal" role="dialog" aria-modal="true" aria-labelledby="mg-title" out:fade={{ duration: 140 }}>
  {#if running}<div class="line top" class:indeterminate={percent === null}><div style:width={percent !== null ? `${percent}%` : undefined}></div></div>{/if}
  <div class="headrow">
    <h1 id="mg-title">{t('Import from another launcher')}</h1>
    <button class="ghost iconbtn" onclick={close} disabled={running} aria-label={t('Close')}><Icon name="x" size={16} /></button>
  </div>
  <p class="muted small">{t('Your worlds, mods, settings and resource packs are copied. The original instances stay as they are.')}</p>

  <div class="list" aria-busy={scanning}>
    {#if scanning}
      <p class="muted center">{t('Searching…')}</p>
    {:else if !rows.length}
      <p class="muted center">{searched ? t('No instances found in this folder.') : t('No other launchers found. If yours is installed somewhere else, enter its folder below.')}</p>
    {:else}
      {#each groups as g (g.source)}
        <div class="group">
          <div class="gname">{sourceName(g.source)}</div>
          {#each g.rows as r (r.key)}
            <div class="item" class:off={r.done}>
              <label class="pick">
                <input type="checkbox" bind:checked={r.picked} disabled={running || r.done} />
                <span class="txt">
                  <span class="nm">{r.name}{#if r.done}<span class="tag ok">{t('Imported')}</span>{/if}</span>
                  <span class="muted small">
                    {r.version || t('Unknown version')} · {loaderNames[r.kind]}{r.kind === r.loader && r.loaderVersion ? ` ${r.loaderVersion}` : ''}
                    {#if r.mods} · {count(r.mods, '1 mod', '{n} mods')}{/if}
                    {#if r.worlds} · {count(r.worlds, '1 world', '{n} worlds')}{/if}
                  </span>
                  {#if r.shared}<span class="note small">{t('Uses the shared .minecraft folder, so everything in it is copied.')}</span>{/if}
                  {#if running && current === r.key && stage}<span class="small st" aria-live="polite">{stage}{percent !== null ? ` · ${percent}%` : ''}</span>{/if}
                </span>
              </label>
              {#if !r.sure && !r.done}
                <div class="fix">
                  <span class="small">{t('Check the version, it could not be read for sure:')}</span>
                  <div class="sels">
                    <Select bind:value={r.version} options={versionOptions} width="150px" label={t('Minecraft version')} disabled={running} />
                    <Select bind:value={r.kind} options={loaderOptions} width="130px" label={t('Loader')} disabled={running} />
                  </div>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {/each}
    {/if}
  </div>

  <form class="other" onsubmit={(e) => { e.preventDefault(); scan(folder.trim() || undefined) }}>
    <input type="text" bind:value={folder} placeholder={t('Another folder, e.g. C:\\MultiMC')} spellcheck="false" disabled={running || scanning} aria-label={t('Launcher or instance folder')} />
    <button type="submit" disabled={running || scanning}>{folder.trim() ? t('Search') : t('Search again')}</button>
  </form>

  <div class="actions">
    <button class="ghost" onclick={close} disabled={running}>{t('Cancel')}</button>
    <button class="primary" onclick={run} disabled={!ready}>
      <Icon name="download" size={15} />{running ? t('Importing…') : picked.length === 1 ? t('Import 1 instance') : t('Import {n} instances', { n: picked.length })}
    </button>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.62); z-index: 50; }
  .modal { position: fixed; z-index: 51; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(620px, calc(100vw - 32px)); max-height: calc(100vh - 32px); overflow: hidden; display: grid; grid-template-rows: auto auto minmax(0, 1fr) auto auto; gap: 14px; padding: 26px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 20px; box-shadow: 0 30px 90px rgba(0, 0, 0, 0.55); animation: mg-pop 300ms var(--spring) both; }
  @keyframes mg-pop { from { opacity: 0; transform: translate(-50%, -47%) scale(0.97); } }
  .top { position: absolute; top: 0; left: 0; right: 0; border-radius: 0; }
  .headrow { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h1 { font-size: 20px; }
  .list { min-height: 120px; max-height: 52vh; overflow: auto; display: grid; align-content: start; gap: 18px; margin: 0 -6px; padding: 0 6px; }
  .center { text-align: center; padding: 30px 10px; }
  .group { display: grid; gap: 4px; }
  .gname { font-size: 12px; font-weight: 600; color: var(--muted); text-transform: uppercase; letter-spacing: 0.04em; padding-bottom: 4px; }
  .item { display: grid; gap: 8px; padding: 10px 12px; border: 1px solid var(--line); border-radius: 14px; transition: opacity 200ms; }
  .item.off { opacity: 0.6; }
  .pick { display: flex; flex-direction: row; align-items: flex-start; gap: 12px; color: var(--text); cursor: pointer; }
  .pick input { margin-top: 3px; }
  .txt { display: grid; gap: 2px; min-width: 0; }
  .nm { font-weight: 600; overflow-wrap: anywhere; }
  .ok { margin-left: 8px; color: var(--text); }
  .note { color: var(--muted); }
  .st { color: var(--text); animation: swapin 260ms var(--spring) backwards; }
  .fix { display: grid; gap: 6px; padding-left: 28px; }
  .sels { display: flex; flex-wrap: wrap; gap: 8px; }
  .other { display: flex; gap: 8px; }
  .other input { flex: 1; min-width: 0; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; }
  .actions .primary { display: inline-flex; align-items: center; gap: 8px; }
</style>
