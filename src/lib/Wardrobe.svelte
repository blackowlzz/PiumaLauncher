<script lang="ts">
  import SkinViewer from './SkinViewer.svelte'
  import Icon from './Icon.svelte'
  import { api, type Wardrobe } from './api'
  import { t } from './i18n.svelte'
  import Head from './Head.svelte'
  import { app, notify } from './store.svelte'
  import { refreshSkin } from './skins.svelte'

  const premium = $derived(app.accounts.accounts.filter((a) => a.kind === 'microsoft'))
  const account = $derived(premium.find((a) => a.id === app.wardrobeId) ?? premium.find((a) => a.id === app.accounts.active) ?? premium[0] ?? null)
  const accountId = $derived(account?.id ?? '')
  const name = $derived(account?.name ?? '')

  let data = $state<Wardrobe | null>(null)
  let loading = $state(true)
  let busy = $state(false)
  let picked = $state<{ png: string; slim: boolean } | null>(null)
  let back = $state(false)
  let input: HTMLInputElement | undefined = $state()

  const current = $derived(data?.skin ?? null)
  const shown = $derived(picked ?? current)
  const activeCape = $derived(data?.capes.find((c) => c.active) ?? null)

  async function load() {
    if (!account) {
      loading = false
      return
    }
    try {
      data = await api.wardrobeLoad(accountId)
    } catch (e) {
      notify(e)
    } finally {
      loading = false
    }
  }

  let loadedFor = ''

  $effect(() => {
    const id = accountId
    if (id === loadedFor) return
    loadedFor = id
    data = null
    picked = null
    loading = true
    load()
  })

  function readFile(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const r = new FileReader()
      r.onload = () => resolve(String(r.result))
      r.onerror = () => reject(new Error(t('This is not a valid image')))
      r.readAsDataURL(file)
    })
  }

  function dimensions(src: string): Promise<[number, number]> {
    return new Promise((resolve, reject) => {
      const img = new Image()
      img.onload = () => resolve([img.naturalWidth, img.naturalHeight])
      img.onerror = () => reject(new Error(t('This is not a valid image')))
      img.src = src
    })
  }

  async function choose(e: Event) {
    const el = e.currentTarget as HTMLInputElement
    const file = el.files?.[0]
    el.value = ''
    if (!file) return
    try {
      const png = await readFile(file)
      if (!png.startsWith('data:image/png')) throw new Error(t('A skin must be a 64x64 or 64x32 PNG'))
      const [w, h] = await dimensions(png)
      if (w !== 64 || (h !== 64 && h !== 32)) throw new Error(t('A skin must be a 64x64 or 64x32 PNG'))
      picked = { png, slim: current?.slim ?? false }
    } catch (err) {
      notify(err instanceof Error ? err.message : err)
    }
  }

  async function applySkin() {
    if (!picked || busy) return
    busy = true
    try {
      await api.wardrobeSetSkin(accountId, picked.png.slice(picked.png.indexOf(',') + 1), picked.slim)
      picked = null
      await load()
      await refreshSkin(name)
      notify(t('Skin updated'))
    } catch (e) {
      notify(e)
    } finally {
      busy = false
    }
  }

  async function chooseCape(id: string | null) {
    if (busy || (activeCape?.id ?? null) === id) return
    busy = true
    try {
      await api.wardrobeSetCape(accountId, id)
      await load()
      notify(t('Cape updated'))
    } catch (e) {
      notify(e)
    } finally {
      busy = false
    }
  }

  function paint(node: HTMLCanvasElement, src: string | null) {
    const draw = (s: string | null) => {
      const ctx = node.getContext('2d')!
      ctx.clearRect(0, 0, 10, 16)
      if (!s) return
      const img = new Image()
      img.onload = () => {
        const k = img.width / 64
        ctx.imageSmoothingEnabled = false
        ctx.drawImage(img, k, k, 10 * k, 16 * k, 0, 0, 10, 16)
      }
      img.src = s
    }
    draw(src)
    return { update: draw }
  }
</script>

<header class="head">
  <h1>{t('Wardrobe')}</h1>
</header>

{#if premium.length === 0}
  <p class="muted">{t('Add a Premium account to change your skin and cape.')}</p>
  <div class="row"><button class="primary" onclick={() => (app.page = 'accounts')}>{t('Go to accounts')}</button></div>
{:else}
  <div class="accs" role="group" aria-label={t('Account')}>
    {#each premium as a (a.id)}
      <button class="acc" aria-pressed={a.id === account?.id} onclick={() => (app.wardrobeId = a.id)}>
        <Head name={a.name} size={28} />{a.name}
      </button>
    {/each}
  </div>
{/if}

{#if premium.length === 0}
{:else if loading}
  <p class="muted">{t('Loading…')}</p>
{:else if !data}
  <p class="muted">{t('Could not load the skin and cape of this account.')}</p>
{:else}
  <div class="wrap">
    <section class="stage">
      {#if shown}
        <SkinViewer skin={shown.png} slim={shown.slim} cape={activeCape?.png ?? null} turn={back} arrive={false} size={380} />
      {/if}
      <div class="seg" role="group">
        <button aria-pressed={!back} onclick={() => (back = false)}>{t('Front')}</button>
        <button aria-pressed={back} onclick={() => (back = true)}>{t('Back')}</button>
      </div>
    </section>

    <div class="side">
      <section class="box">
        <h2>{t('Skin')}</h2>
        <p class="muted small">{t('Pick a 64x64 or 64x32 PNG from your computer.')}</p>
        <input bind:this={input} type="file" accept="image/png" hidden onchange={choose} />
        <div class="actions">
          <button onclick={() => input?.click()} disabled={busy}>{t('Choose file…')}</button>
          {#if picked}
            <div class="seg" role="group">
              <button aria-pressed={!picked.slim} onclick={() => picked && (picked.slim = false)}>{t('Classic')}</button>
              <button aria-pressed={picked.slim} onclick={() => picked && (picked.slim = true)}>{t('Slim')}</button>
            </div>
          {/if}
        </div>
        {#if picked}
          <div class="actions">
            <button class="primary" onclick={applySkin} disabled={busy}>{t('Apply skin')}</button>
            <button class="ghost" onclick={() => (picked = null)} disabled={busy}>{t('Cancel')}</button>
          </div>
        {/if}
      </section>

      <section class="box">
        <h2>{t('Cape')}</h2>
        <div class="capes">
          <button class="cape" class:on={!activeCape} aria-pressed={!activeCape} onclick={() => chooseCape(null)} disabled={busy}>
            <span class="none"><Icon name="x" size={20} /></span>
            <span class="nm">{t('No cape')}</span>
          </button>
          {#each data.capes as c (c.id)}
            <button class="cape" class:on={c.active} aria-pressed={c.active} onclick={() => chooseCape(c.id)} disabled={busy}>
              <canvas width="10" height="16" use:paint={c.png} aria-hidden="true"></canvas>
              <span class="nm">{c.name}</span>
            </button>
          {/each}
        </div>
        {#if data.capes.length === 0}
          <p class="muted small">{t('This account has no capes yet.')}</p>
        {/if}
      </section>
    </div>
  </div>
{/if}

<style>
  .row { display: flex; align-items: center; gap: 14px; margin-top: 14px; }
  .accs { display: flex; flex-wrap: wrap; gap: 10px; margin-bottom: 22px; }
  .acc { display: flex; align-items: center; gap: 10px; padding: 6px 16px 6px 8px; border-radius: 999px; }
  .acc[aria-pressed='true'] { border-color: var(--text); background: var(--surface-2); }
  .wrap { display: grid; grid-template-columns: minmax(280px, 400px) minmax(0, 1fr); gap: 28px; align-items: start; }
  .stage { display: grid; justify-items: center; gap: 14px; padding: 22px; border: 1px solid var(--line); border-radius: 26px; background: var(--surface); }
  .side { display: grid; gap: 16px; }
  .box { display: grid; gap: 12px; padding: 22px; border: 1px solid var(--line); border-radius: 26px; background: var(--surface); }
  .box h2 { margin: 0; font-size: 15px; font-weight: 650; color: var(--text); }
  .box p { margin: 0; }
  .actions { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .capes { display: grid; grid-template-columns: repeat(auto-fill, minmax(96px, 1fr)); gap: 10px; }
  .cape { display: grid; justify-items: center; gap: 8px; padding: 12px 8px; border-radius: 16px; }
  .cape.on { border-color: var(--text); background: var(--surface-2); }
  .cape canvas { width: 40px; height: 64px; image-rendering: pixelated; }
  .none { display: grid; place-items: center; width: 40px; height: 64px; border: 1px dashed var(--line-strong); border-radius: 6px; color: var(--muted); }
  .nm { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; color: var(--muted); }
  .cape.on .nm { color: var(--text); }
  @media (max-width: 900px) { .wrap { grid-template-columns: 1fr; } }
</style>
