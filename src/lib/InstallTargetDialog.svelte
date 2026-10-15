<script lang="ts">
  import { thumb } from './img'
  import { fade } from 'svelte/transition'
  import { modrinth, type InstalledProject, type PackVersion, type ProjectHit, type ProjectType } from './api'
  import { app } from './store.svelte'
  import { t } from './i18n.svelte'
  import { contentKindOf, isCompatible, loaderNames } from './meta'
  import Cover from './Cover.svelte'
  import Icon from './Icon.svelte'

  let { hit, type, version, onConfirm, onClose }: {
    hit: ProjectHit
    type: ProjectType
    version?: PackVersion
    onConfirm: (instanceIds: string[]) => void
    onClose: () => void
  } = $props()

  const fits = (id: string) => {
    const inst = app.instances.find((i) => i.id === id)
    return !!inst && isCompatible(hit, type, inst, version)
  }
  const eligible = $derived(
    (type === 'mod' ? app.instances.filter((i) => i.loader !== 'vanilla') : [...app.instances]).sort(
      (a, b) => Number(fits(b.id)) - Number(fits(a.id)) || Number(b.id === app.selectedId) - Number(a.id === app.selectedId),
    ),
  )
  let has: Set<string> = $state(new Set())
  let ready = $state(false)
  let chosen: string[] = $state([])
  let confirm: HTMLButtonElement | undefined = $state()

  $effect(() => {
    const kind = contentKindOf(type)
    const ids = eligible.map((i) => i.id)
    let stale = false
    void Promise.all(ids.map(async (id) => ((await modrinth.installedProjects(id, kind).catch((): InstalledProject[] => [])).some((p) => p.projectId === hit.projectId) ? id : null))).then((found) => {
      if (stale) return
      has = new Set(found.filter((id): id is string => id !== null))
      if (!ready) {
        ready = true
        const first = eligible.find((i) => fits(i.id) && !has.has(i.id))
        if (first) chosen = [first.id]
      }
    })
    return () => (stale = true)
  })

  $effect(() => {
    if (ready && chosen.length) confirm?.focus()
  })

  const toggle = (id: string) => (chosen = chosen.includes(id) ? chosen.filter((x) => x !== id) : [...chosen, id])

  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="backdrop" role="presentation" transition:fade={{ duration: 160 }} onclick={onClose}></div>
<div class="dialog" role="dialog" aria-modal="true" aria-labelledby="itd-title" out:fade={{ duration: 160 }}>
  <div class="head">
    <div class="icon">
      {#if hit.iconUrl}<img use:thumb={{ src: hit.iconUrl, w: 128 }} alt="" decoding="async" />{:else}<Icon name="package" size={22} />{/if}
    </div>
    <div class="grow">
      <h3 id="itd-title">{hit.title}{#if version}<span class="muted small"> · {version.number}</span>{/if}</h3>
      <p class="muted small">{t('Choose an instance to install "{name}" into.', { name: hit.title })}</p>
    </div>
    <button class="ghost iconbtn" onclick={onClose} aria-label={t('Close')}><Icon name="x" size={16} /></button>
  </div>

  {#if eligible.length}
    <ul class="list">
      {#each eligible as inst, n (inst.id)}
        {@const installed = has.has(inst.id)}
        {@const ok = fits(inst.id)}
        <li style:--n={n}>
          <button class="opt" class:on={chosen.includes(inst.id)} aria-pressed={chosen.includes(inst.id)} disabled={installed || !ok} onclick={() => toggle(inst.id)}>
            <span class="thumb"><Cover {inst} /></span>
            <span class="meta">
              <span class="nm">{inst.name}</span>
              <span class="muted small">{inst.mcVersion}{inst.loader !== 'vanilla' ? ` · ${loaderNames[inst.loader]}` : ''}</span>
            </span>
            {#if installed}<span class="tag good">{t('Installed')}</span>{:else if !ok}<span class="tag">{t('Not compatible')}</span>{:else}<span class="box" class:on={chosen.includes(inst.id)}>{#if chosen.includes(inst.id)}<Icon name="check" size={13} />{/if}</span>{/if}
          </button>
        </li>
      {/each}
    </ul>
    <button class="primary go" bind:this={confirm} disabled={!chosen.length} onclick={() => onConfirm(chosen)}>{t('Install')}{chosen.length > 1 ? ` · ${chosen.length}` : ''}</button>
  {:else}
    <p class="muted empty">{type === 'mod' ? t('To install mods you need a Fabric or Quilt instance: create one from the Library.') : t('Create an instance first.')}</p>
  {/if}
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.62); z-index: 50; }
  .dialog { position: fixed; z-index: 51; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(500px, calc(100vw - 32px)); max-height: calc(100vh - 48px); display: grid; grid-template-rows: auto minmax(0, 1fr) auto; gap: 16px; padding: 22px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 20px; box-shadow: 0 30px 90px rgba(0, 0, 0, 0.55); animation: itd-pop 300ms var(--spring) both; }
  .head { display: flex; align-items: center; gap: 14px; }
  .icon { width: 48px; height: 48px; flex: none; display: grid; place-items: center; border-radius: 12px; overflow: hidden; background: var(--surface-2); border: 1px solid var(--line); color: var(--muted); animation: gopop 420ms var(--spring) 80ms backwards; }
  .icon img { width: 100%; height: 100%; object-fit: cover; }
  h3 { margin: 0 0 3px; font-size: 18px; font-weight: 650; letter-spacing: -0.03em; overflow-wrap: anywhere; }
  .list { list-style: none; margin: -4px -6px; padding: 4px 6px; display: grid; gap: 6px; overflow: auto; }
  .list li { animation: swapin 360ms var(--spring) calc(var(--n) * 40ms + 60ms) backwards; }
  .opt { display: flex; align-items: center; gap: 12px; width: 100%; text-align: left; padding: 8px 12px 8px 8px; border-radius: 14px; border-color: var(--line); }
  .opt:hover:not(:disabled) { border-color: var(--line-strong); }
  .opt:disabled { color: inherit; opacity: 0.6; }
  .thumb { position: relative; width: 44px; height: 44px; border-radius: 10px; overflow: hidden; flex: none; }
  .meta { display: grid; min-width: 0; flex: 1; }
  .nm { font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tag { font-size: 10.5px; padding: 1px 8px; white-space: nowrap; }
  .good { color: var(--text); border-color: var(--text); }
  .opt.on { border-color: var(--text); }
  .box { display: inline-grid; place-items: center; width: 22px; height: 22px; border-radius: 7px; flex: none; border: 1px solid var(--line-strong); color: var(--inv-text); transition: background var(--ease), border-color var(--ease); }
  .box.on { background: var(--inv-bg); border-color: var(--inv-bg); }
  .go { justify-self: stretch; padding: 11px 18px; }
  .empty { margin: 0; padding: 8px 0; }
  @keyframes itd-pop { from { opacity: 0; transform: translate(-50%, -47%) scale(0.97); } }
</style>
