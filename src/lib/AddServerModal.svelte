<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import { fade, slide } from 'svelte/transition'
  import { api } from './api'
  import { app, notify } from './store.svelte'
  import { locale, t } from './i18n.svelte'
  import { ping, pingOf } from './quickplay.svelte'
  import Cover from './Cover.svelte'
  import Icon from './Icon.svelte'
  import Motd from './Motd.svelte'

  let { instanceId, onClose, onAdded }: { instanceId?: string; onClose: () => void; onAdded: () => void } = $props()

  let name = $state('')
  let address = $state('')
  let picked = $state<string[]>(untrack(() => (instanceId ? [instanceId] : app.instances.length === 1 ? [app.instances[0].id] : [])))
  let saving = $state(false)
  let probe = $state('')
  let input: HTMLInputElement | undefined = $state()

  const valid = $derived(/^[^\s/]+$/.test(address.trim()))
  const preview = $derived(probe ? pingOf(probe) : undefined)

  $effect(() => {
    const a = address.trim()
    if (!/^[^\s/]+\.[^\s/]+$|^\[.+\]|:\d+$/.test(a)) {
      probe = ''
      return
    }
    const id = setTimeout(() => {
      probe = a
      ping(a)
    }, 650)
    return () => clearTimeout(id)
  })

  const toggle = (id: string) => (picked = picked.includes(id) ? picked.filter((x) => x !== id) : [...picked, id])

  async function save() {
    if (!valid || !picked.length || saving) return
    saving = true
    try {
      await api.addServer(picked, name, address)
      onAdded()
      onClose()
    } catch (e) {
      notify(e)
    } finally {
      saving = false
    }
  }

  onMount(() => input?.focus())
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onClose()} />

<div class="backdrop" role="presentation" transition:fade={{ duration: 160 }} onclick={onClose}></div>
<div class="modal" role="dialog" aria-modal="true" aria-labelledby="as-title" out:fade={{ duration: 140 }}>
  <form class="mform" onsubmit={(e) => { e.preventDefault(); save() }}>
    <h1 id="as-title">{t('Add a server')}</h1>

    <label>{t('Address')}<input bind:this={input} bind:value={address} placeholder="play.example.net" autocomplete="off" spellcheck="false" /></label>
    <label>{t('Name')}<input bind:value={name} placeholder={address.trim() || t('Minecraft Server')} maxlength="64" /></label>

    {#if preview}
      <div class="preview" transition:slide={{ duration: 220 }}>
        {#if preview.state === 'pending'}
          <div class="scan muted small"><span class="radar"></span>{t('Looking for the server…')}</div>
        {:else if preview.state === 'online'}
          <div class="found">
            <div class="fav">{#if preview.status.favicon}<img src={preview.status.favicon} alt="" />{:else}<Icon name="server" size={18} />{/if}</div>
            <div class="grow">
              <div class="small"><span class="dot"></span>{t('Online')} · {preview.status.online.toLocaleString(locale())}/{preview.status.max.toLocaleString(locale())} · {preview.status.version}</div>
              <Motd text={preview.status.motd} />
            </div>
          </div>
        {:else}
          <div class="muted small">{t("The server didn't answer. You can still add it.")}</div>
        {/if}
      </div>
    {/if}

    {#if !instanceId}
      <div class="field">
        <span class="lbl">{t('Save it in')}</span>
        {#if app.instances.length}
          <div class="pick">
            {#each app.instances as i, n (i.id)}
              <button type="button" class="ichip" aria-pressed={picked.includes(i.id)} style:--n={n} onclick={() => toggle(i.id)}>
                <span class="th"><Cover inst={i} /></span>
                <span class="nm">{i.name}</span>
                <span class="muted small">{i.mcVersion}</span>
                <span class="tick"><Icon name="check" size={13} /></span>
              </button>
            {/each}
          </div>
        {:else}
          <p class="muted small">{t('Create an instance first.')}</p>
        {/if}
      </div>
    {/if}

    <div class="foot">
      <button type="button" class="ghost" onclick={onClose}>{t('Cancel')}</button>
      <button class="primary" type="submit" disabled={!valid || !picked.length || saving}>{picked.length > 1 ? t('Add to {n} instances', { n: picked.length }) : t('Add')}</button>
    </div>
  </form>
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.62); z-index: 50; }
  .modal { position: fixed; z-index: 51; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(520px, calc(100vw - 32px)); max-height: calc(100vh - 32px); overflow: auto; padding: 26px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 18px; box-shadow: 0 30px 90px rgba(0, 0, 0, 0.55); animation: as-pop 280ms var(--spring) both; }
  @keyframes as-pop { from { opacity: 0; transform: translate(-50%, -47%) scale(0.97); } }
  .mform { display: grid; gap: 16px; }
  h1 { font-size: 20px; }
  .field { display: grid; gap: 8px; }
  .lbl { font-size: 12px; color: var(--muted); }
  .preview { border: 1px solid var(--line); border-radius: 14px; padding: 12px; background: var(--surface-2); }
  .scan { display: flex; align-items: center; gap: 10px; }
  .radar { position: relative; width: 10px; height: 10px; border-radius: 50%; background: var(--text); }
  .radar::after { content: ''; position: absolute; inset: 0; border-radius: 50%; border: 1px solid var(--text); animation: ring 1.1s ease-out infinite; }
  @keyframes ring { to { transform: scale(2.8); opacity: 0; } }
  .found { display: flex; gap: 12px; align-items: flex-start; animation: swapin 360ms var(--spring) backwards; }
  .found .grow { display: grid; gap: 6px; }
  .fav { width: 40px; height: 40px; flex: none; display: grid; place-items: center; border-radius: 10px; overflow: hidden; background: var(--surface); color: var(--muted); animation: gopop 400ms var(--spring) backwards; }
  .fav img { width: 100%; height: 100%; image-rendering: pixelated; }
  .dot { display: inline-block; width: 7px; height: 7px; margin-right: 6px; border-radius: 50%; background: #3ecf6e; box-shadow: 0 0 0 0 rgba(62, 207, 110, 0.6); animation: beat 1.6s ease-out infinite; }
  @keyframes beat { 70% { box-shadow: 0 0 0 6px rgba(62, 207, 110, 0); } }
  .pick { display: grid; gap: 6px; max-height: 220px; overflow: auto; padding: 2px; }
  .ichip { display: flex; align-items: center; gap: 10px; text-align: left; padding: 6px 12px 6px 6px; border-radius: 12px; border-color: var(--line); animation: swapin 320ms var(--spring) calc(var(--n) * 30ms) backwards; }
  .ichip[aria-pressed='true'] { border-color: var(--text); background: var(--surface-2); }
  .th { position: relative; width: 32px; height: 32px; border-radius: 8px; overflow: hidden; flex: none; display: grid; place-items: center; }
  .nm { flex: 1; min-width: 0; font-weight: 550; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tick { display: grid; place-items: center; width: 20px; height: 20px; border-radius: 50%; border: 1px solid var(--line-strong); color: transparent; transition: background var(--ease), color var(--ease), border-color var(--ease), transform 280ms var(--spring); }
  .ichip[aria-pressed='true'] .tick { background: var(--inv-bg); border-color: var(--inv-bg); color: var(--inv-text); transform: scale(1.1); }
  .foot { display: flex; justify-content: flex-end; gap: 8px; }
</style>
