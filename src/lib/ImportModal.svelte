<script lang="ts">
  import { onMount } from 'svelte'
  import { fade } from 'svelte/transition'
  import { api } from './api'
  import { app, importPack } from './store.svelte'
  import { stageText, t } from './i18n.svelte'
  import Icon from './Icon.svelte'

  let { onClose }: { onClose: () => void } = $props()

  let code = $state('')
  let link = $state('')
  let picker: HTMLInputElement | undefined = $state()
  let area: HTMLTextAreaElement | undefined = $state()
  const busy = $derived(app.installingId === 'import')
  const looksValid = $derived(/PIUMA1-[A-Za-z0-9_-]{8,}/.test(code.replace(/\s+/g, '')))
  const linkValid = $derived(/modrinth\.com\/(modpack|project)\/[A-Za-z0-9_-]+/i.test(link))
  const stage = $derived(busy && app.install ? stageText(app.install.stage) : '')
  const percent = $derived(busy && app.install && app.install.total > 0 ? Math.round((app.install.done / app.install.total) * 100) : null)

  async function fromCode() {
    if (!looksValid || busy) return
    if (await importPack(() => api.importShareCode(code))) onClose()
  }

  async function fromLink() {
    if (!linkValid || busy) return
    if (await importPack(() => api.importModrinthLink(link))) onClose()
  }

  async function fromFile(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0]
    if (!file || busy) return
    const bytes = new Uint8Array(await file.arrayBuffer())
    if (await importPack(() => api.importMrpackBytes(bytes))) onClose()
  }

  const close = () => !busy && onClose()
  onMount(() => area?.focus())
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && close()} />

<div class="backdrop" role="presentation" transition:fade={{ duration: 160 }} onclick={close}></div>
<div class="modal" role="dialog" aria-modal="true" aria-labelledby="im-title" out:fade={{ duration: 140 }}>
  {#if busy}<div class="line top" class:indeterminate={percent === null}><div style:width={percent !== null ? `${percent}%` : undefined}></div></div>{/if}
  <h1 id="im-title">{t('Import a modpack')}</h1>

  <label>
    {t('Share code')}
    <textarea bind:this={area} bind:value={code} rows="3" placeholder="PIUMA1-…" spellcheck="false" disabled={busy}></textarea>
  </label>
  <button class="primary wide" class:ready={looksValid && !busy} onclick={fromCode} disabled={!looksValid || busy}>
    <Icon name="download" size={15} />{t('Import from code')}
  </button>

  <div class="or"><span>{t('or')}</span></div>

  <label>
    {t('Modrinth link')}
    <input type="text" bind:value={link} placeholder="modrinth.com/modpack/…" spellcheck="false" disabled={busy} onkeydown={(e) => e.key === 'Enter' && fromLink()} />
  </label>
  <button class="primary wide" class:ready={linkValid && !busy} onclick={fromLink} disabled={!linkValid || busy}>
    <Icon name="download" size={15} />{t('Import from link')}
  </button>

  <div class="or"><span>{t('or')}</span></div>

  <button class="file" onclick={() => picker?.click()} disabled={busy}>
    <span class="pk"><Icon name="package" size={22} /></span>
    <span class="grow">
      <span class="fl">{t('Choose a .mrpack file')}</span>
      <span class="muted small">{t('You can also drag it onto the window.')}</span>
    </span>
    <Icon name="chevron" size={16} />
  </button>
  <input bind:this={picker} type="file" accept=".mrpack,.zip" hidden onchange={fromFile} />

  {#if stage}<p class="muted small st" aria-live="polite">{stage}{percent !== null ? ` · ${percent}%` : ''}</p>{/if}
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.62); z-index: 50; }
  .modal { position: fixed; z-index: 51; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(480px, calc(100vw - 32px)); max-height: calc(100vh - 32px); overflow: hidden auto; display: grid; gap: 14px; padding: 26px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 20px; box-shadow: 0 30px 90px rgba(0, 0, 0, 0.55); animation: im-pop 300ms var(--spring) both; }
  @keyframes im-pop { from { opacity: 0; transform: translate(-50%, -47%) scale(0.97); } }
  .top { position: absolute; top: 0; left: 0; right: 0; border-radius: 0; }
  h1 { font-size: 20px; margin-bottom: 4px; }
  textarea { resize: none; font: 12px/1.5 ui-monospace, 'JetBrains Mono', monospace; letter-spacing: 0; word-break: break-all; }
  .wide { display: inline-flex; align-items: center; justify-content: center; gap: 8px; }
  .ready { animation: gopop 360ms var(--spring); }
  .or { display: flex; align-items: center; gap: 12px; color: var(--faint); font-size: 12px; }
  .or::before, .or::after { content: ''; flex: 1; height: 1px; background: var(--line); }
  .file { display: flex; align-items: center; gap: 14px; text-align: left; padding: 14px 16px; border-radius: 16px; border-style: dashed; }
  .file .grow { display: grid; gap: 2px; }
  .fl { font-weight: 600; }
  .pk { display: grid; place-items: center; width: 44px; height: 44px; border-radius: 12px; background: var(--surface-2); transition: transform 400ms var(--spring); }
  .file:hover:not(:disabled) .pk { transform: translateY(-3px) rotate(-6deg); }
  .st { animation: swapin 260ms var(--spring) backwards; }
</style>
