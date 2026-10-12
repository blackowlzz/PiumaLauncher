<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type Loader } from './api'
  import { app, notify, play, reloadInstances } from './store.svelte'
  import { t } from './i18n.svelte'
  import Select from './Select.svelte'
  import { loaderNames } from './meta'

  let { onClose }: { onClose: () => void } = $props()

  let name = $state('')
  let version = $state('')
  let loader: Loader = $state('vanilla')
  let snapshots = $state(false)
  let launchAfter = $state(false)
  let creating = $state(false)
  let nameInput: HTMLInputElement | undefined = $state()

  const choices = $derived(app.versions.filter((v) => v.kind === 'release' || (snapshots && v.kind === 'snapshot')))
  const latest = $derived(app.versions.find((v) => v.kind === 'release')?.id ?? '')
  const chosen = $derived(version || latest)
  const versionOptions = $derived(
    app.versions.length
      ? choices.map((v) => ({ value: v.id, label: v.id + (v.id === latest ? ` (${t('latest')})` : '') }))
      : [{ value: '', label: t('Loading…') }],
  )
  $effect(() => {
    if (!version && latest) version = latest
  })
  const autoName = $derived(`${loader === 'vanilla' ? 'Minecraft' : loaderNames[loader]} ${chosen}`.trim())

  const loaders: { id: Loader | 'forge' | 'neoforge'; label: string; ready: boolean }[] = [
    { id: 'vanilla', label: 'Vanilla', ready: true },
    { id: 'fabric', label: 'Fabric', ready: true },
    { id: 'quilt', label: 'Quilt', ready: true },
    { id: 'forge', label: 'Forge', ready: false },
    { id: 'neoforge', label: 'NeoForge', ready: false },
  ]

  async function create() {
    creating = true
    try {
      const inst = await api.createInstance(name.trim() || autoName, chosen, loader)
      await reloadInstances()
      app.selectedId = inst.id
      onClose()
      if (launchAfter) await play(inst.id)
    } catch (e) {
      notify(e)
    } finally {
      creating = false
    }
  }

  onMount(() => nameInput?.focus())
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onClose()} />

<div class="backdrop" role="presentation" onclick={onClose}></div>
<div class="modal" role="dialog" aria-modal="true" aria-labelledby="ni-title" tabindex="-1">
<form class="mform" onsubmit={(e) => { e.preventDefault(); create() }}>
  <h1 id="ni-title">{t('New instance')}</h1>

  <label>
    {t('Name')}
    <input bind:this={nameInput} bind:value={name} placeholder={autoName} />
  </label>

  <div class="field">
    <div class="row between">
      <span class="lbl" id="ver-label">{t('Minecraft version')}</span>
      <label class="inline"><input type="checkbox" bind:checked={snapshots} />{t('Show snapshots')}</label>
    </div>
    <Select bind:value={version} options={versionOptions} disabled={!app.versions.length} label={t('Minecraft version')} />
  </div>

  <div class="field">
    <span class="lbl">{t('Loader')}</span>
    <div class="chips">
      {#each loaders as l}
        <button type="button" class="chip" aria-pressed={loader === l.id} disabled={!l.ready} title={l.ready ? '' : t('Coming soon')} onclick={() => l.ready && (loader = l.id as Loader)}>
          {l.label}{l.ready ? '' : ` · ${t('soon')}`}
        </button>
      {/each}
    </div>
  </div>

  <div class="foot">
    <label class="inline"><input type="checkbox" bind:checked={launchAfter} />{t('Launch after install')}</label>
    <div class="row">
      <button type="button" class="ghost" onclick={onClose}>{t('Cancel')}</button>
      <button class="primary" type="submit" disabled={creating || !app.versions.length}>{creating ? t('Creating…') : t('Create instance')}</button>
    </div>
  </div>
</form>
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.62); z-index: 20; animation: fade 200ms ease both; }
  .mform { display: grid; gap: 20px; }
  .modal { position: fixed; z-index: 21; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(520px, calc(100vw - 32px)); max-height: calc(100vh - 32px); overflow: auto; padding: 26px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 18px; box-shadow: 0 30px 90px rgba(0, 0, 0, 0.55); animation: pop 280ms var(--spring) both; }
  @keyframes fade { from { opacity: 0; } }
  @keyframes pop { from { opacity: 0; transform: translate(-50%, -47%) scale(0.97); } }
  h1 { font-size: 20px; }
  .field { display: grid; gap: 8px; }
  .lbl { font-size: 12px; color: var(--muted); }
  .between { justify-content: space-between; }
  .inline { display: inline-flex; flex-direction: row; align-items: center; gap: 8px; color: var(--muted); font-size: 13px; }
  .foot { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; }
</style>
