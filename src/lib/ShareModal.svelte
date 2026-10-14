<script lang="ts">
  import { fade, slide } from 'svelte/transition'
  import { api, type Exported, type ExportOptions, type Instance, type ShareCode } from './api'
  import { app, notify } from './store.svelte'
  import { stageText, t } from './i18n.svelte'
  import Cover from './Cover.svelte'
  import Icon from './Icon.svelte'

  let { inst, onClose }: { inst: Instance; onClose: () => void } = $props()

  type Tab = 'file' | 'code'
  let tab = $state<Tab>('file')
  let opts = $state<ExportOptions>({ resourcepacks: true, shaderpacks: true, config: true, options: false, servers: false })
  let working = $state(false)
  let exported = $state<Exported | null>(null)
  let shared = $state<ShareCode | null>(null)
  let copied = $state(false)
  let copyTimer: ReturnType<typeof setTimeout> | undefined

  const stage = $derived(working && app.install ? stageText(app.install.stage) : '')
  const fileName = $derived(exported ? exported.path.split(/[\\/]/).pop() : '')
  const toggles: { key: keyof ExportOptions; label: string; hint: string }[] = [
    { key: 'resourcepacks', label: 'Resource packs', hint: 'Textures and sounds' },
    { key: 'shaderpacks', label: 'Shader packs', hint: 'Lights and shadows' },
    { key: 'config', label: 'Mod settings', hint: 'The config folder' },
    { key: 'options', label: 'Game options', hint: 'Controls, video and audio' },
    { key: 'servers', label: 'Server list', hint: 'Shares the addresses you saved' },
  ]

  async function run<T>(fn: () => Promise<T>): Promise<T | null> {
    working = true
    app.install = null
    try {
      return await fn()
    } catch (e) {
      notify(e)
      return null
    } finally {
      working = false
      app.install = null
    }
  }

  async function exportFile() {
    exported = await run(() => api.exportMrpack(inst.id, $state.snapshot(opts)))
  }

  async function makeCode() {
    shared = await run(() => api.shareCode(inst.id))
  }

  async function copy() {
    if (!shared) return
    try {
      await navigator.clipboard.writeText(shared.code)
    } catch {
      const area = document.querySelector<HTMLTextAreaElement>('.codebox')
      area?.select()
      document.execCommand('copy')
    }
    copied = true
    clearTimeout(copyTimer)
    copyTimer = setTimeout(() => (copied = false), 2200)
  }

  const close = () => !working && onClose()
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && close()} />

<div class="backdrop" role="presentation" transition:fade={{ duration: 160 }} onclick={close}></div>
<div class="modal" role="dialog" aria-modal="true" aria-labelledby="sh-title" out:fade={{ duration: 140 }}>
  {#if working}<div class="line indeterminate top"><div></div></div>{/if}
  <div class="head">
    <span class="thumb"><Cover {inst} /></span>
    <div class="grow">
      <h1 id="sh-title">{t('Share {name}', { name: inst.name })}</h1>
      <p class="muted small">Minecraft {inst.mcVersion}</p>
    </div>
    <button class="ghost iconbtn" onclick={close} disabled={working} aria-label={t('Close')}><Icon name="x" size={16} /></button>
  </div>

  <div class="seg tabs" role="tablist" style:--i={tab === 'file' ? 0 : 1}>
    <span class="pill" aria-hidden="true"></span>
    <button role="tab" aria-selected={tab === 'file'} onclick={() => (tab = 'file')} disabled={working}><Icon name="package" size={15} />{t('.mrpack file')}</button>
    <button role="tab" aria-selected={tab === 'code'} onclick={() => (tab = 'code')} disabled={working}><Icon name="share" size={15} />{t('Share code')}</button>
  </div>

  {#key tab}
    <div class="pane">
      {#if tab === 'file'}
        <p class="muted lead">{t('One file to send to your friends. It opens in PiumaLauncher, the Modrinth App, Prism and other launchers. Mods from Modrinth are linked, the rest goes inside the file.')}</p>
        {#if exported}
          <div class="done" in:slide={{ duration: 260 }}>
            <div class="box" aria-hidden="true"><Icon name="package" size={30} /></div>
            <div class="grow">
              <div class="fn">{fileName}</div>
              <div class="muted small">{t('Saved in Downloads')} · {t('{a} from Modrinth, {b} inside the file', { a: exported.linked, b: exported.bundled })}</div>
            </div>
            <button onclick={() => api.revealFile(exported!.path).catch(notify)}><Icon name="folder" size={15} />{t('Show in folder')}</button>
          </div>
        {:else}
          <div class="opts">
            {#each toggles as o, n}
              <label class="opt" style:--n={n}>
                <span class="grow"><span class="ol">{t(o.label)}</span><span class="muted small">{t(o.hint)}</span></span>
                <span class="switch"><input type="checkbox" bind:checked={opts[o.key]} disabled={working} /><span class="track"></span></span>
              </label>
            {/each}
          </div>
        {/if}
      {:else}
        <p class="muted lead">{t('A short text to paste in a chat. Your friend pastes it in PiumaLauncher and gets the same mods. Only content from Modrinth is included.')}</p>
        {#if shared}
          <div class="codewrap" in:slide={{ duration: 260 }}>
            <textarea class="codebox" readonly rows="4" value={shared.code} onfocus={(e) => e.currentTarget.select()}></textarea>
            <span class="sweep" aria-hidden="true"></span>
          </div>
          <div class="row meta">
            <span class="muted small grow">{t('{n} files · {c} characters', { n: shared.included, c: shared.code.length })}</span>
            <button class="primary copyb" class:ok={copied} onclick={copy}>
              {#key copied}<span class="ci">{#if copied}<Icon name="check" size={15} />{t('Copied')}{:else}<Icon name="copy" size={15} />{t('Copy')}{/if}</span>{/key}
            </button>
          </div>
          {#if shared.excluded.length}
            <div class="warn" in:slide={{ duration: 240 }}>
              <p class="small">{t('Not on Modrinth, so not in the code. Use the .mrpack file to include them:')}</p>
              <div class="chips">{#each shared.excluded as f, n}<span class="tag" style:--n={n}>{f}</span>{/each}</div>
            </div>
          {/if}
        {/if}
      {/if}
    </div>
  {/key}

  <div class="foot">
    {#if stage}<span class="muted small grow st">{stage}</span>{:else}<span class="grow"></span>{/if}
    {#if tab === 'file' && !exported}
      <button class="primary" onclick={exportFile} disabled={working}><Icon name="download" size={15} />{working ? t('Exporting…') : t('Export .mrpack')}</button>
    {:else if tab === 'code' && !shared}
      <button class="primary" onclick={makeCode} disabled={working}><Icon name="share" size={15} />{working ? t('Creating…') : t('Create code')}</button>
    {:else}
      <button class="ghost" onclick={close}>{t('Close')}</button>
    {/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.62); z-index: 50; }
  .modal { position: fixed; z-index: 51; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(560px, calc(100vw - 32px)); max-height: calc(100vh - 32px); overflow: hidden auto; display: grid; gap: 18px; padding: 24px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 20px; box-shadow: 0 30px 90px rgba(0, 0, 0, 0.55); animation: sh-pop 300ms var(--spring) both; }
  @keyframes sh-pop { from { opacity: 0; transform: translate(-50%, -47%) scale(0.97); } }
  .top { position: absolute; top: 0; left: 0; right: 0; border-radius: 0; }
  .head { display: flex; align-items: center; gap: 14px; }
  .thumb { position: relative; width: 46px; height: 46px; border-radius: 12px; overflow: hidden; flex: none; display: grid; place-items: center; animation: gopop 400ms var(--spring) 60ms backwards; }
  h1 { font-size: 19px; overflow-wrap: anywhere; }
  .tabs { position: relative; display: grid; grid-template-columns: 1fr 1fr; }
  .tabs button { position: relative; z-index: 1; display: inline-flex; align-items: center; justify-content: center; gap: 8px; background: transparent !important; }
  .tabs button[aria-selected='true'] { color: var(--text); }
  .pill { position: absolute; top: 3px; bottom: 3px; left: 3px; width: calc(50% - 4px); border-radius: 999px; background: var(--surface-2); border: 1px solid var(--line-strong); transform: translateX(calc(var(--i) * 100%)); transition: transform 380ms var(--spring); }
  .pane { display: grid; gap: 14px; animation: swapin 320ms var(--spring) backwards; }
  .lead { line-height: 1.55; font-size: 13.5px; }
  .opts { display: grid; gap: 4px; }
  .opt { display: flex; flex-direction: row; align-items: center; gap: 14px; padding: 10px 12px; border-radius: 12px; color: var(--text); font-size: 14px; cursor: pointer; transition: background var(--ease); animation: swapin 320ms var(--spring) calc(var(--n) * 35ms) backwards; }
  .opt:hover { background: var(--surface-2); }
  .opt .grow { display: grid; gap: 1px; }
  .ol { font-weight: 550; }
  .done { display: flex; align-items: center; gap: 14px; padding: 14px; border-radius: 16px; border: 1px solid var(--line-strong); background: var(--surface-2); }
  .done button { display: inline-flex; align-items: center; gap: 8px; }
  .box { width: 52px; height: 52px; display: grid; place-items: center; border-radius: 14px; background: var(--inv-bg); color: var(--inv-text); flex: none; animation: drop 700ms var(--spring) backwards; }
  @keyframes drop { 0% { transform: translateY(-26px) rotate(-12deg) scale(0.6); opacity: 0; } 55% { transform: translateY(3px) rotate(3deg) scale(1.04); opacity: 1; } 75% { transform: translateY(-2px) rotate(-1deg); } }
  .fn { font-weight: 600; overflow-wrap: anywhere; }
  .codewrap { position: relative; border-radius: 14px; overflow: hidden; }
  .codebox { display: block; resize: none; font: 12px/1.55 ui-monospace, 'JetBrains Mono', monospace; letter-spacing: 0; word-break: break-all; background: var(--bg); border-color: var(--line-strong); border-radius: 14px; padding: 12px 14px; }
  .sweep { position: absolute; inset: 0; pointer-events: none; background: linear-gradient(90deg, transparent, var(--surface) 12%, var(--surface)); animation: reveal 900ms cubic-bezier(0.6, 0, 0.2, 1) forwards; }
  @keyframes reveal { from { transform: translateX(0); } to { transform: translateX(105%); } }
  .meta { gap: 10px; }
  .copyb { display: inline-flex; align-items: center; min-width: 120px; justify-content: center; transition: background var(--ease), transform 120ms var(--spring); }
  .copyb.ok { animation: bump 420ms var(--spring); }
  @keyframes bump { 40% { transform: scale(1.06); } }
  .ci { display: inline-flex; align-items: center; gap: 8px; animation: swapin 260ms var(--spring) backwards; }
  .warn { display: grid; gap: 8px; padding: 12px 14px; border-radius: 14px; border: 1px dashed var(--line-strong); }
  .warn .chips { gap: 6px; }
  .warn .tag { font-size: 11px; animation: gopop 320ms var(--spring) calc(var(--n) * 40ms) backwards; }
  .foot { display: flex; align-items: center; gap: 10px; }
  .foot button { display: inline-flex; align-items: center; gap: 8px; }
  .st { animation: swapin 260ms var(--spring) backwards; }
  :global(:root[data-reduce]) .sweep { display: none; }
  @media (prefers-reduced-motion: reduce) { .sweep { display: none; } }
</style>
