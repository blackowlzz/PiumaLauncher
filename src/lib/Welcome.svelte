<script lang="ts">
  import { fade, fly } from 'svelte/transition'
  import Icon from './Icon.svelte'
  import SkinViewer from './SkinViewer.svelte'
  import { api } from './api'
  import { LANGUAGES, i18n, setLanguage, suggestedLanguage, t, translateError } from './i18n.svelte'
  import { makeSkin } from './skin'
  import { ensureSkin, skinOf } from './skins.svelte'
  import { app, reloadAccounts } from './store.svelte'

  const GREET: Record<string, { hello: string; pick: string }> = {
    en: { hello: 'Welcome', pick: 'Choose your language' },
    it: { hello: 'Benvenuto', pick: 'Scegli la tua lingua' },
    es: { hello: 'Bienvenido', pick: 'Elige tu idioma' },
    fr: { hello: 'Bienvenue', pick: 'Choisis ta langue' },
    de: { hello: 'Willkommen', pick: 'Wähle deine Sprache' },
    pt: { hello: 'Bem-vindo', pick: 'Escolha seu idioma' },
    sq: { hello: 'Mirë se vini', pick: 'Zgjidhni gjuhën tuaj' },
    hu: { hello: 'Üdvözlünk', pick: 'Válaszd ki a nyelvet' },
    ru: { hello: 'Добро пожаловать', pick: 'Выберите язык' },
    bg: { hello: 'Добре дошли', pick: 'Изберете език' },
    uk: { hello: 'Ласкаво просимо', pick: 'Оберіть мову' },
    pl: { hello: 'Witamy', pick: 'Wybierz swój język' },
    ro: { hello: 'Bun venit', pick: 'Alege limba' },
    sr: { hello: 'Dobrodošli', pick: 'Izaberite jezik' },
    el: { hello: 'Καλώς ήρθατε', pick: 'Επιλέξτε γλώσσα' },
    tr: { hello: 'Hoş geldiniz', pick: 'Dilinizi seçin' },
  }

  let visible = $state(false)
  let decided = false
  let step: 'lang' | 'name' = $state('lang')
  let cameFromLang = $state(false)
  let picked: string | null = $state(null)
  let hover: string | null = $state(null)
  let shown = $state(0)
  let leaving = $state(false)

  let name = $state('')
  let busy = $state(false)
  let error = $state('')
  let input: HTMLInputElement | undefined = $state()
  let vh = $state(innerHeight)
  const figureSize = $derived(Math.max(260, Math.min(470, Math.round(vh * 0.5))))

  const suggested = suggestedLanguage()
  const activeCode = $derived(hover ?? LANGUAGES[shown % LANGUAGES.length].code)
  const valid = $derived(/^[A-Za-z0-9_]{3,16}$/.test(name))
  const generated = $derived(makeSkin(name.trim() || 'Steve'))
  const real = $derived(valid ? skinOf(name.trim()) : undefined)
  const skin = $derived(real?.state === 'ready' ? real.png : generated)
  const slim = $derived(real?.state === 'ready' && real.slim)
  const skinNote = $derived(
    !valid || error
      ? ''
      : real?.state === 'loading' || !real
        ? t('Looking for your skin…')
        : real.state === 'ready'
          ? t('Skin found on Mojang')
          : t('No skin for this name: using a generated one'),
  )
  $effect(() => {
    if (!valid) return
    const n = name.trim()
    const id = setTimeout(() => ensureSkin(n), 450)
    return () => clearTimeout(id)
  })

  $effect(() => {
    if (decided || !app.loaded || !i18n.ready) return
    decided = true
    const needLang = !i18n.chosen
    const needName = app.accounts.accounts.length === 0
    if (needLang || needName) {
      visible = true
      cameFromLang = needLang
      step = needLang ? 'lang' : 'name'
    }
  })

  $effect(() => {
    if (!visible || step !== 'lang') return
    const id = setInterval(() => {
      if (!hover && !picked) shown++
    }, 2300)
    return () => clearInterval(id)
  })

  async function choose(code: string) {
    if (picked) return
    picked = code
    await setLanguage(code)
    await new Promise((r) => setTimeout(r, 800))
    if (app.accounts.accounts.length === 0) {
      step = 'name'
      picked = null
    } else {
      await finish()
    }
  }

  async function finish() {
    leaving = true
    await new Promise((r) => setTimeout(r, 1100))
    visible = false
  }

  async function submit() {
    if (!valid || busy) return
    busy = true
    error = ''
    try {
      await api.addOfflineAccount(name.trim())
      await reloadAccounts()
      await finish()
    } catch (e) {
      error = translateError(e)
    } finally {
      busy = false
    }
  }

  $effect(() => {
    if (step === 'name') {
      const id = setTimeout(() => input?.focus(), 1400)
      return () => clearTimeout(id)
    }
  })
</script>

<svelte:window onresize={() => (vh = innerHeight)} />

{#if visible}
  <div class="welcome" class:leaving role="dialog" aria-modal="true" aria-label="Piuma">
    {#if step === 'lang'}
      <section class="pane lang" class:gone={picked}>
        <ul class="menu" aria-label="Language">
          {#each LANGUAGES as l, n}
            <li style:--n={n}>
              <button
                class="item"
                class:on={activeCode === l.code}
                class:sel={picked === l.code}
                onpointerenter={() => (hover = l.code)}
                onpointerleave={() => (hover = null)}
                onfocus={() => (hover = l.code)}
                onblur={() => (hover = null)}
                onclick={() => choose(l.code)}
              >
                <span class="name">{l.name}</span>
                <span class="go"><Icon name="chevron" size={34} /></span>
                {#if l.code === suggested}<span class="dot" title="★"></span>{/if}
              </button>
            </li>
          {/each}
        </ul>

        <div class="side" aria-live="polite">
          {#key activeCode}
            <div class="hello" in:fly={{ y: 24, duration: 600 }} out:fade={{ duration: 180 }}>
              <div class="h">{GREET[activeCode].hello}</div>
              <div class="p">{GREET[activeCode].pick}</div>
            </div>
          {/key}
        </div>
      </section>
    {:else}
      <section class="pane naming">
        <h1 class="q" in:fly={{ y: -20, duration: 700, delay: 250 }}>{t('What should we call you?')}</h1>

        <div class="figure" class:jump={leaving}>
          <label class="plate">
            <span class="sr">{t('Your player name')}</span>
            <input
              bind:this={input}
              bind:value={name}
              style:width="{Math.max(9, name.length + 2)}ch"
              placeholder="Steve"
              maxlength="16"
              autocomplete="off"
              spellcheck="false"
              onkeydown={(e) => e.key === 'Enter' && submit()}
            />
          </label>
          <SkinViewer {skin} {slim} size={figureSize} />
        </div>

        <div class="foot">
          <p class="hint" class:bad={error || (name && !valid)}>{error || (valid ? skinNote : t('Use 3–16 characters: letters, numbers and _.'))}</p>
          <div class="cta">
            {#if cameFromLang}<button class="ghost" onclick={() => ((step = 'lang'), (picked = null))} disabled={busy}>{t('Back')}</button>{/if}
            <button class="primary big" onclick={submit} disabled={!valid || busy}>{busy ? t('Creating…') : t('Start playing')}</button>
          </div>
          <p class="muted small note">{t('This is your offline player. You can add more accounts any time.')}</p>
        </div>
      </section>
    {/if}
  </div>
{/if}

<style>
  .welcome { position: fixed; inset: 40px 0 0 0; z-index: 100; background: var(--bg); overflow: hidden; animation: in 700ms var(--spring) both; }
  .welcome.leaving { animation: out 600ms 650ms ease forwards; pointer-events: none; }
  .pane { position: absolute; inset: 0; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }

  .lang { display: grid; grid-template-columns: minmax(0, 1.2fr) minmax(0, 0.8fr); align-items: center; padding: 30px clamp(36px, 7vw, 120px) 40px; animation: paneIn 700ms var(--spring) both; }
  .lang.gone { animation: paneOut 650ms 150ms var(--spring) forwards; }
  .menu { list-style: none; margin: 0; padding: 0; display: grid; }
  .menu li { animation: rise 800ms var(--spring) both; animation-delay: calc(var(--n) * 45ms + 250ms); }
  .item { position: relative; display: flex; align-items: center; gap: 18px; width: 100%; border: 0; border-radius: 0; background: transparent; padding: 0; font-size: clamp(24px, 5vh, 104px); font-weight: 650; letter-spacing: -0.055em; line-height: 1.04; text-align: left; color: var(--faint); transition: color 220ms, transform 450ms var(--spring); overflow: visible; }
  .item:hover:not(:disabled) { background: transparent; border: 0; }
  .item.on { color: var(--text); transform: translateX(18px); }
  .item .go { display: inline-grid; opacity: 0; transform: translateX(-14px); transition: opacity 250ms, transform 450ms var(--spring); }
  .item.on .go { opacity: 1; transform: none; }
  .item .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--faint); align-self: flex-start; margin-top: 0.28em; flex: none; opacity: 0.8; }
  .item.on .dot { background: var(--text); }
  .gone .item:not(.sel) { opacity: 0; transform: translateX(-30px); transition: opacity 400ms, transform 500ms; }
  .gone .item.sel { transform: translateX(34px); }
  .side { display: grid; justify-items: end; text-align: right; align-self: end; padding-bottom: 6vh; }
  .side > :global(*) { grid-area: 1 / 1; }
  .hello .h { font-size: clamp(30px, 5.2vh, 56px); font-weight: 650; letter-spacing: -0.045em; }
  .hello .p { margin-top: 6px; color: var(--muted); font-size: 16px; }

  .naming { display: grid; grid-template-rows: auto 1fr auto; justify-items: center; align-items: center; padding: 30px 40px 36px; animation: paneIn 800ms var(--spring) both; }
  .q { margin: 0; font-size: clamp(34px, 5.6vh, 66px); font-weight: 650; letter-spacing: -0.05em; text-align: center; }
  .figure { position: relative; display: grid; justify-items: center; margin-top: 26px; }
  .figure.jump { animation: jump 700ms 100ms cubic-bezier(0.3, 0.8, 0.3, 1); }
  .plate { position: absolute; top: -5%; left: 50%; transform: translateX(-50%); z-index: 3; animation: plateIn 800ms 1.3s var(--spring) both; }
  .plate input { font-family: inherit; font-size: 22px; font-weight: 600; text-align: center; color: #fff; background: rgba(0, 0, 0, 0.62); border: 1px solid rgba(255, 255, 255, 0.28); border-radius: 8px; padding: 7px 8px; min-width: 9ch; caret-color: #fff; transition: border-color var(--ease), box-shadow var(--ease); }
  .plate input::placeholder { color: rgba(255, 255, 255, 0.4); }
  .plate input:focus { outline: none; border-color: #fff; box-shadow: 0 0 0 4px rgba(255, 255, 255, 0.12); background: rgba(0, 0, 0, 0.7); }
  .foot { display: grid; justify-items: center; gap: 12px; text-align: center; }
  .hint { font-size: 13px; color: var(--muted); transition: color var(--ease); }
  .hint.bad { color: var(--text); }
  .cta { display: flex; align-items: center; gap: 12px; }
  .note { max-width: 420px; line-height: 1.5; }

  @keyframes in { from { opacity: 0; } }
  @keyframes out { to { opacity: 0; transform: scale(1.04); } }
  @keyframes paneIn { from { opacity: 0; transform: translateX(40px); } }
  @keyframes paneOut { to { opacity: 0; transform: translateX(-60px) scale(0.97); } }
  @keyframes rise { from { opacity: 0; transform: translateY(24px); } }
  @keyframes plateIn { from { opacity: 0; transform: translateX(-50%) translateY(14px) scale(0.92); } }
  @keyframes jump { 35% { transform: translateY(-46px); } 70% { transform: translateY(4px); } }

  @media (max-width: 900px) {
    .lang { grid-template-columns: 1fr; align-content: center; }
    .side { display: none; }
  }
</style>
