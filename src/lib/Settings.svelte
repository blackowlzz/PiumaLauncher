<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type FlagProfile, type StorageInfo } from './api'
  import { LANGUAGES, i18n, setLanguage, t } from './i18n.svelte'
  import Select from './Select.svelte'
  import { formatBytes, profiles } from './meta'
  import { app, notify, saveSettings } from './store.svelte'

  let storage = $state<StorageInfo | null>(null)
  const loadStorage = async () => (storage = await api.storageInfo().catch(() => null))
  onMount(loadStorage)

  const total = $derived(storage ? storage.items.reduce((n, i) => n + i.bytes, 0) : 0)
  const label = (key: string) =>
    ({ instances: t('Instances'), versions: t('Game versions'), libraries: t('Libraries'), assets: t('Assets'), runtimes: t('Java runtimes'), skins: t('Skin cache'), other: t('Other') })[key] ?? key

  const themes = [
    { id: 'system', label: () => t('System') },
    { id: 'light', label: () => t('Light') },
    { id: 'dark', label: () => t('Dark') },
  ] as const

  async function changeLanguage(code: string) {
    await setLanguage(code)
    app.settings.language = code
  }

  async function deleteRuntime(name: string) {
    if (!confirm(t('Delete {name}?', { name }))) return
    try {
      await api.deleteRuntime(name)
      await loadStorage()
    } catch (e) {
      notify(e)
    }
  }

  const options = [
    { key: 'accounts', label: () => t('Accounts') },
    { key: 'settings', label: () => t('Settings and language') },
    { key: 'stats', label: () => t('Playtime history') },
    { key: 'instances', label: () => t('Instances, mods and worlds') },
    { key: 'gamefiles', label: () => t('Downloaded game files') },
    { key: 'runtimes', label: () => t('Downloaded Java') },
    { key: 'skins', label: () => t('Skin cache') },
  ]
  let showReset = $state(false)
  let resetting = $state(false)
  let chosen: Record<string, boolean> = $state(Object.fromEntries(options.map((o) => [o.key, true])))
  const allChosen = $derived(options.every((o) => chosen[o.key]))
  const anyChosen = $derived(options.some((o) => chosen[o.key]))

  async function doReset() {
    resetting = true
    try {
      await api.resetData(allChosen ? ['all'] : options.filter((o) => chosen[o.key]).map((o) => o.key))
      await api.restartApp()
    } catch (e) {
      notify(e)
      resetting = false
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && !resetting && (showReset = false)} />

<header class="head">
  <h1>{t('Settings')}</h1>
  <p class="muted">{t('Make the launcher yours.')}</p>
</header>

<div class="groups">
  <section class="group">
    <h2>{t('General')}</h2>

    <div class="item">
      <div class="txt"><div class="n">{t('Language')}</div></div>
      <Select value={i18n.lang} width="190px" label={t('Language')} onchange={(c) => changeLanguage(c)} options={LANGUAGES.map((l) => ({ value: l.code as string, label: l.name }))} />
    </div>

    <div class="item">
      <div class="txt"><div class="n">{t('Theme')}</div></div>
      <div class="seg" role="group" aria-label={t('Theme')}>
        {#each themes as th}
          <button aria-pressed={app.settings.theme === th.id} onclick={() => { app.settings.theme = th.id; saveSettings() }}>{th.label()}</button>
        {/each}
      </div>
    </div>

    <div class="item">
      <div class="txt"><div class="n">{t('Interface size')}</div></div>
      <Select
        bind:value={app.settings.uiScale}
        width="190px"
        label={t('Interface size')}
        onchange={saveSettings}
        options={[
          { value: 'auto', label: t('Automatic') },
          { value: '100', label: '100%' },
          { value: '125', label: '125%' },
          { value: '150', label: '150%' },
        ]}
      />
    </div>

    <div class="item">
      <div class="txt">
        <div class="n">{t('Minimize while playing')}</div>
        <div class="d">{t('The launcher reappears when you close the game.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={app.settings.minimizeOnLaunch} onchange={saveSettings} aria-label={t('Minimize while playing')} />
        <span class="track"></span>
      </label>
    </div>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Check for updates')}</div>
        <div class="d">{t('Looks on GitHub for a new version when the launcher starts.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={app.settings.checkUpdates} onchange={saveSettings} aria-label={t('Check for updates')} />
        <span class="track"></span>
      </label>
    </div>
  </section>

  <section class="group">
    <h2>{t('New instances')}</h2>
    <p class="muted small intro">{t('Used when you create a new instance.')}</p>

    <div class="item">
      <div class="txt"><div class="n">{t('Max RAM (MB)')}</div></div>
      <input class="num" type="number" min="512" step="256" bind:value={app.settings.defaultMemoryMaxMb} onchange={saveSettings} aria-label={t('Max RAM (MB)')} />
    </div>
    <div class="item">
      <div class="txt"><div class="n">{t('Initial RAM (MB)')}</div></div>
      <input class="num" type="number" min="128" step="128" bind:value={app.settings.defaultMemoryMinMb} onchange={saveSettings} aria-label={t('Initial RAM (MB)')} />
    </div>
    <div class="item">
      <div class="txt">
        <div class="n">{t('JVM profile')}</div>
        <div class="d">{profiles().find((p) => p.id === app.settings.defaultFlagProfile)?.hint}</div>
      </div>
      <Select bind:value={app.settings.defaultFlagProfile} width="190px" label={t('JVM profile')} onchange={saveSettings} options={profiles().map((p) => ({ value: p.id, label: p.label }))} />
    </div>
  </section>

  <section class="group">
    <h2>{t('Performance')}</h2>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Smooth scrolling')}</div>
        <div class="d">{t('Eases the mouse wheel for a softer scroll.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={app.settings.smoothScroll} onchange={saveSettings} aria-label={t('Smooth scrolling')} />
        <span class="track"></span>
      </label>
    </div>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Reduce visual effects')}</div>
        <div class="d">{t('Turns off animations and transitions. Helps on slower computers.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={app.settings.reduceEffects} onchange={saveSettings} aria-label={t('Reduce visual effects')} />
        <span class="track"></span>
      </label>
    </div>
  </section>

  <section class="group">
    <h2>{t('Privacy')}</h2>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Look up skins on Mojang')}</div>
        <div class="d">{t('Sends the player name you type to Mojang to show its skin. Turn off to use generated skins only.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={app.settings.lookupSkins} onchange={saveSettings} aria-label={t('Look up skins on Mojang')} />
        <span class="track"></span>
      </label>
    </div>
  </section>

  <section class="group">
    <h2>{t('Storage')}</h2>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Data folder')}</div>
        <div class="d path">{storage?.root ?? '…'}</div>
      </div>
      <button onclick={() => api.openDataDir().catch(notify)}>{t('Open folder')}</button>
    </div>

    {#if storage}
      <div class="usage">
        {#each storage.items.filter((i) => i.bytes > 0) as it}
          <div class="u"><span>{label(it.key)}</span><span class="muted">{formatBytes(it.bytes)}</span></div>
        {/each}
        <div class="u tot"><span>{t('Total')}</span><span>{formatBytes(total)}</span></div>
      </div>

      <div class="rt">
        <div class="n">{t('Java runtimes')}</div>
        {#each storage.runtimes as r (r.name)}
          <div class="item slim">
            <div class="txt"><div class="n">{r.name}</div><div class="d">{formatBytes(r.bytes)}</div></div>
            <button class="ghost" onclick={() => deleteRuntime(r.name)}>{t('Delete')}</button>
          </div>
        {:else}
          <p class="muted small">{t('No Java downloaded yet.')}</p>
        {/each}
      </div>
    {/if}
  </section>

  <section class="group danger">
    <h2>{t('Reset launcher')}</h2>
    <div class="item">
      <div class="txt"><div class="d">{t('Delete launcher data. After a restart, the launcher starts as if it were new.')}</div></div>
      <button onclick={() => (showReset = true)}>{t('Reset…')}</button>
    </div>
  </section>

  <section class="group">
    <h2>{t('About')}</h2>
    <div class="item">
      <div class="txt">
        <div class="n">PiumaLauncher</div>
        <div class="d">{t('Version {v}', { v: __APP_VERSION__ })} · {t('Open source · GPL-3.0')}</div>
      </div>
    </div>
  </section>
</div>

{#if showReset}
  <div class="backdrop" role="presentation" onclick={() => !resetting && (showReset = false)}></div>
  <div class="modal" role="dialog" aria-modal="true" aria-labelledby="reset-title">
    <h1 id="reset-title">{t('Reset the launcher')}</h1>
    <p class="muted">{t('Choose what to delete. The launcher restarts afterwards.')}</p>

    <div class="opts">
      {#each options as o}
        <label class="opt"><input type="checkbox" bind:checked={chosen[o.key]} disabled={resetting} />{o.label()}</label>
      {/each}
      <button class="ghost sel" onclick={() => options.forEach((o) => (chosen[o.key] = !allChosen))} disabled={resetting}>{t('Select all')}</button>
    </div>

    <p class="warn small">{t("This can't be undone. Worlds inside the instances are deleted too.")}</p>
    <div class="row end">
      <button class="ghost" onclick={() => (showReset = false)} disabled={resetting}>{t('Cancel')}</button>
      <button class="primary" onclick={doReset} disabled={resetting || !anyChosen}>{resetting ? t('Deleting…') : t('Delete and restart')}</button>
    </div>
  </div>
{/if}

<style>
  .head { display: grid; gap: 6px; }
  .groups { display: grid; gap: 34px; max-width: 820px; }
  .group h2 { margin: 0 0 4px; color: var(--text); font-size: 16px; font-weight: 600; letter-spacing: -0.02em; }
  .intro { margin-bottom: 4px; }
  .item { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 16px 0; border-bottom: 1px solid var(--line); }
  .item.slim { padding: 10px 0; }
  .txt { min-width: 0; }
  .n { font-weight: 500; }
  .d { margin-top: 2px; color: var(--muted); font-size: 13px; line-height: 1.45; max-width: 520px; }
  .path { font-family: inherit; font-size: 12px; word-break: break-all; }
  .num { width: 130px; }
  .usage { display: grid; gap: 2px; padding: 14px 0; border-bottom: 1px solid var(--line); }
  .u { display: flex; justify-content: space-between; font-size: 14px; padding: 3px 0; }
  .u.tot { margin-top: 6px; padding-top: 10px; border-top: 1px solid var(--line); font-weight: 600; }
  .rt { padding-top: 16px; }
  .danger h2 { color: var(--text); }

  .backdrop { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.62); z-index: 20; }
  .modal { position: fixed; z-index: 21; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(520px, calc(100vw - 32px)); max-height: calc(100vh - 32px); overflow: auto; display: grid; gap: 16px; padding: 28px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 22px; box-shadow: 0 30px 90px rgba(0, 0, 0, 0.55); animation: pop 260ms var(--spring) both; }
  .modal h1 { font-size: 22px; }
  .opts { display: grid; gap: 10px; }
  .opt { display: flex; flex-direction: row; align-items: center; gap: 12px; color: var(--text); font-size: 14px; }
  .sel { justify-self: start; padding-left: 0; }
  .warn { color: var(--text); background: var(--surface-2); border-radius: 12px; padding: 10px 14px; }
  .end { justify-content: flex-end; }
  @keyframes pop { from { opacity: 0; transform: translate(-50%, -48%) scale(0.97); } }
</style>
