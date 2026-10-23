<script lang="ts">
  import { onMount } from 'svelte'
  import { api, uiScales, type RuntimeEntry, type StorageInfo } from './api'
  import { LANGUAGES, i18n, setLanguage, stageText, t } from './i18n.svelte'
  import Select from './Select.svelte'
  import { formatBytes, formatDuration } from './meta'
  import { app, askConfirm, installRuntime, notify, reloadRuntimes, saveSettings } from './store.svelte'

  const minutes = (steps: number[], current: number) =>
    [0, ...new Set([...steps, current].filter((m) => m > 0))].sort((a, b) => a - b).map((m) => ({ value: m, label: m ? formatDuration(m * 60) : t('Off') }))

  let storage = $state<StorageInfo | null>(null)
  const loadStorage = async () => (storage = await api.storageInfo().catch(() => null))
  onMount(loadStorage)

  onMount(reloadRuntimes)

  const javaNote = (r: RuntimeEntry) =>
    r.flavor === 'graalvm'
      ? t('Fast peak performance, ideal for modern versions.')
      : r.major <= 8
        ? t('For Minecraft 1.16 and older.')
        : t('Lightweight and very stable.')

  async function installJava(r: RuntimeEntry) {
    if (await installRuntime(r)) await loadStorage()
  }

  async function removeJava(r: RuntimeEntry) {
    if (!(await askConfirm(t('Delete {name}?', { name: `${r.label} ${r.major}` })))) return
    try {
      await api.deleteRuntime(r.id)
      if (app.settings.preferredJava === r.id) {
        app.settings.preferredJava = null
        await saveSettings()
      }
      await Promise.all([reloadRuntimes(), loadStorage()])
    } catch (e) {
      notify(e)
    }
  }

  async function setPreferred(r: RuntimeEntry) {
    app.settings.preferredJava = app.settings.preferredJava === r.id ? null : r.id
    await saveSettings()
  }

  const total = $derived(storage ? storage.items.reduce((n, i) => n + i.bytes, 0) : 0)
  const label = (key: string) =>
    ({ instances: t('Instances'), backups: t('World backups'), versions: t('Game versions'), libraries: t('Libraries'), assets: t('Assets'), runtimes: t('Java runtimes'), skins: t('Skin cache'), other: t('Other') })[key] ?? key

  const themes = [
    { id: 'system', label: () => t('System') },
    { id: 'light', label: () => t('Light') },
    { id: 'dark', label: () => t('Dark') },
  ] as const

  const syncKinds = [
    { key: 'controls', label: () => t('Controls'), hint: () => t('Keybinds, sensitivity, auto-jump, toggle crouch and sprint') },
    { key: 'audio', label: () => t('Audio'), hint: () => t('Volumes, output device, subtitles') },
    { key: 'interface', label: () => t('Language and interface'), hint: () => t('Language, GUI scale, chat, accessibility, skin layers') },
    { key: 'video', label: () => t('Video'), hint: () => t('Render distance, FOV, brightness, graphics. Modpacks often tune these, so it starts off.') },
  ]

  function toggleSyncKind(key: string, on: boolean) {
    const rest = app.settings.syncCategories.filter((c) => c !== key)
    app.settings.syncCategories = on ? [...rest, key] : rest
    saveSettings()
  }

  let copied = $state(false)
  async function copyDiagnostics() {
    try {
      await navigator.clipboard.writeText(await api.diagnostics())
      copied = true
      setTimeout(() => (copied = false), 2500)
    } catch (e) {
      notify(e)
    }
  }

  async function changeLanguage(code: string) {
    await setLanguage(code)
    app.settings.language = code
  }

  async function deleteRuntime(name: string) {
    if (!(await askConfirm(t('Delete {name}?', { name })))) return
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
          ...uiScales.map((s) => ({ value: s, label: `${s}%` })),
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
        <div class="n">{t('Back up worlds automatically')}</div>
        <div class="d">{t('Before playing and before changing mods, only for worlds that changed. Keeps the last 5 automatic backups of each world; the ones you make yourself are never deleted.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={app.settings.autoBackup} onchange={saveSettings} aria-label={t('Back up worlds automatically')} />
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

  <section class="group" id="playtime">
    <h2>{t('Playtime')}</h2>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Break reminder')}</div>
        <div class="d">{t('A notification when you have been playing for this long without closing the game.')}</div>
      </div>
      <Select
        bind:value={app.settings.breakReminderMinutes}
        width="190px"
        label={t('Break reminder')}
        onchange={saveSettings}
        options={minutes([30, 45, 60, 90, 120], app.settings.breakReminderMinutes)}
      />
    </div>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Daily limit')}</div>
        <div class="d">{t('When you reach it you get a notification, and Play asks before starting again. The game is never closed for you.')}</div>
      </div>
      <Select
        bind:value={app.settings.dailyLimitMinutes}
        width="190px"
        label={t('Daily limit')}
        onchange={saveSettings}
        options={minutes([60, 90, 120, 180, 240, 360], app.settings.dailyLimitMinutes)}
      />
    </div>
  </section>

  <section class="group">
    <h2>{t('Synced game settings')}</h2>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Share settings between instances')}</div>
        <div class="d">{t('Change an option in one instance and the others get it the next time they start. New instances start with your settings instead of the defaults.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={app.settings.syncOptions} onchange={saveSettings} aria-label={t('Share settings between instances')} />
        <span class="track"></span>
      </label>
    </div>
    {#if app.settings.syncOptions}
      <div class="kinds">
        {#each syncKinds as k}
          <label class="opt kind">
            <input type="checkbox" checked={app.settings.syncCategories.includes(k.key)} onchange={(e) => toggleSyncKind(k.key, e.currentTarget.checked)} />
            <span class="txt"><span class="n">{k.label()}</span><span class="d">{k.hint()}</span></span>
          </label>
        {/each}
        <p class="muted small">{t('Resource packs and per-instance choices are never touched. Versions before 1.13 share settings only among themselves.')}</p>
      </div>
    {/if}
  </section>

  <section class="group">
    <h2>{t('Java')}</h2>
    <p class="muted small intro">{t('Install the Java you prefer. The default one is used whenever the game version allows it, otherwise the launcher picks a compatible one on its own.')}</p>

    {#each app.runtimes as r (r.id)}
      <div class="item">
        <div class="txt">
          <div class="n">
            {r.label} {r.major}
            {#if app.settings.preferredJava === r.id}<span class="tag def">{t('Default')}</span>{/if}
          </div>
          <div class="d">
            {#if app.javaBusy === r.id}
              {app.install ? `${stageText(app.install.stage)}${app.install.total > 0 ? ` · ${Math.round((app.install.done / app.install.total) * 100)}%` : ''}` : t('Installing…')}
            {:else if r.installed}
              {r.version} · {javaNote(r)}
            {:else}
              {javaNote(r)}
            {/if}
          </div>
        </div>
        <div class="acts">
          {#if r.installed}
            <button class:primary={app.settings.preferredJava !== r.id} onclick={() => setPreferred(r)}>
              {app.settings.preferredJava === r.id ? t('Remove default') : t('Set as default')}
            </button>
            <button class="ghost" onclick={() => removeJava(r)}>{t('Delete')}</button>
          {:else}
            <button onclick={() => installJava(r)} disabled={app.javaBusy !== null}>{app.javaBusy === r.id ? t('Installing…') : t('Install')}</button>
          {/if}
        </div>
      </div>
    {/each}
  </section>

  <section class="group">
    <h2>{t('Developer')}</h2>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Live console')}</div>
        <div class="d">{t('Opens a separate window with the live game log every time you play. Turned off, nothing runs and nothing is stored. Takes effect from the next launch.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={app.settings.liveConsole} onchange={saveSettings} aria-label={t('Live console')} />
        <span class="track"></span>
      </label>
    </div>
  </section>

  <section class="group">
    <h2>{t('Privacy')}</h2>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Discord Rich Presence')}</div>
        <div class="d">{t('Shows on your Discord profile that you are using PiumaLauncher, with your player name and whether you are in singleplayer or on a server.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={app.settings.discordPresence} onchange={saveSettings} aria-label={t('Discord Rich Presence')} />
        <span class="track"></span>
      </label>
    </div>
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
      <button class="danger-btn" onclick={() => (showReset = true)}>{t('Reset…')}</button>
    </div>
  </section>

  <section class="group">
    <h2>{t('Troubleshooting')}</h2>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Copy diagnostics')}</div>
        <div class="d">{t('Copies the launcher version, your system, your instances and the recent launcher log, ready to paste into a bug report. Account tokens are never included.')}</div>
      </div>
      <button onclick={copyDiagnostics}>{copied ? t('Copied') : t('Copy')}</button>
    </div>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Launcher log')}</div>
        <div class="d">{t('What the launcher did on each launch, with any errors.')}</div>
      </div>
      <button onclick={() => api.openLogs().catch(notify)}>{t('Open folder')}</button>
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
      <button class="danger-btn solid" onclick={doReset} disabled={resetting || !anyChosen}>{resetting ? t('Deleting…') : t('Delete and restart')}</button>
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
  .acts { display: flex; gap: 8px; align-items: center; flex: none; }
  .def { margin-left: 8px; color: var(--text); }
  .danger h2 { color: var(--text); }
  .danger-btn { color: #e5484d; border-color: rgba(229, 72, 77, 0.55); }
  .danger-btn:hover:not(:disabled) { background: rgba(229, 72, 77, 0.12); border-color: #e5484d; }
  .danger-btn.solid { background: #e5484d; border-color: #e5484d; color: #fff; font-weight: 600; }
  .danger-btn.solid:hover:not(:disabled) { background: #d23c41; border-color: #d23c41; }
  .danger-btn.solid:disabled { background: var(--surface-2); border-color: var(--line); color: var(--faint); }

  .backdrop { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.62); z-index: 20; animation: fade 200ms ease both; }
  @keyframes fade { from { opacity: 0; } }
  .modal { position: fixed; z-index: 21; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(520px, calc(100vw - 32px)); max-height: calc(100vh - 32px); overflow: auto; display: grid; gap: 16px; padding: 28px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 22px; box-shadow: 0 30px 90px rgba(0, 0, 0, 0.55); animation: pop 260ms var(--spring) both; }
  .modal h1 { font-size: 22px; }
  .opts { display: grid; gap: 10px; }
  .kinds { display: grid; gap: 12px; padding: 14px 0; border-bottom: 1px solid var(--line); }
  .kind { align-items: flex-start; }
  .kind .txt { display: grid; gap: 2px; }
  .kind input { margin-top: 3px; }
  .opt { display: flex; flex-direction: row; align-items: center; gap: 12px; color: var(--text); font-size: 14px; }
  .sel { justify-self: start; padding-left: 0; }
  .warn { color: var(--text); background: var(--surface-2); border-radius: 12px; padding: 10px 14px; }
  .end { justify-content: flex-end; }
  @keyframes pop { from { opacity: 0; transform: translate(-50%, -48%) scale(0.97); } }
</style>
