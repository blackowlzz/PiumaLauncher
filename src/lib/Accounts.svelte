<script lang="ts">
  import Head from './Head.svelte'
  import Verified from './Verified.svelte'
  import Wardrobe from './Wardrobe.svelte'
  import { reveal, spotlight } from './actions'
  import { api, type DeviceLogin } from './api'
  import { t } from './i18n.svelte'
  import { app, notify, reloadAccounts } from './store.svelte'

  let name = $state('')
  let styling = $state<string | null>(null)
  const styled = $derived(app.accounts.accounts.find((a) => a.id === styling) ?? null)
  let login = $state<DeviceLogin | null>(null)
  let starting = $state(false)
  let copied = $state(false)
  let cancelled = false

  async function guard(fn: () => Promise<unknown>) {
    try {
      await fn()
      await reloadAccounts()
    } catch (e) {
      notify(e)
    }
  }

  const add = () =>
    guard(async () => {
      await api.addOfflineAccount(name)
      name = ''
    })

  async function signIn() {
    starting = true
    cancelled = false
    try {
      login = await api.microsoftLoginStart()
      copied = false
      api.openUrl(login.verificationUri).catch(() => {})
      await api.microsoftLoginWait()
      await reloadAccounts()
    } catch (e) {
      if (!cancelled) notify(e)
    } finally {
      login = null
      starting = false
    }
  }

  function cancelLogin() {
    cancelled = true
    login = null
    api.microsoftLoginCancel().catch(() => {})
  }

  async function copyCode() {
    if (!login) return
    try {
      await navigator.clipboard.writeText(login.userCode)
      copied = true
    } catch {}
  }

  const count = $derived(app.accounts.accounts.length)
</script>

{#if styled}
  <Wardrobe accountId={styled.id} name={styled.name} onBack={() => (styling = null)} />
{:else}
<header class="head">
  <h1>{t('Accounts')}</h1>
</header>

<div class="grid">
  {#each app.accounts.accounts as a, n (a.id)}
    {@const active = app.accounts.active === a.id}
    <article class="acard" class:active use:spotlight use:reveal={n}>
      <div class="top">
        <Head name={a.name} size={68} />
        <div class="tags">
          <span class="tag">{a.kind === 'offline' ? t('Offline') : t('Premium')}</span>
          {#if active}<span class="tag on">{t('In use')}</span>{/if}
        </div>
      </div>
      <div>
        <div class="name"><span class="nm">{a.name}</span>{#if a.kind === 'microsoft'}<Verified size={22} />{/if}</div>
        <div class="uuid" title={a.uuid}>{a.uuid}</div>
      </div>
      <div class="acts">
        {#if a.kind === 'microsoft'}<button onclick={() => (styling = a.id)}>{t('Skin')}</button>{/if}
        {#if !active}<button class="primary" onclick={() => guard(() => api.setActiveAccount(a.id))}>{t('Use')}</button>{/if}
        <button class="ghost" onclick={() => guard(() => api.removeAccount(a.id))}>{t('Remove')}</button>
      </div>
    </article>
  {/each}

  <form class="acard add" use:reveal={count} onsubmit={(e) => { e.preventDefault(); add() }}>
    <div class="plus" aria-hidden="true">+</div>
    <label>
      {t('New offline account')}
      <input bind:value={name} placeholder={t('Player name')} maxlength="16" autocomplete="off" />
    </label>
    <div class="acts"><button class="primary" type="submit" disabled={name.trim().length < 3}>{t('Add')}</button></div>
  </form>

  <article class="acard soon" class:waiting={!!login} use:reveal={count + 1}>
    <div class="top"><span class="ms" aria-hidden="true">
      <svg width="30" height="30" viewBox="0 0 24 24"><path fill="currentColor" d="M3 3h8.5v8.5H3zM12.5 3H21v8.5h-8.5zM3 12.5h8.5V21H3zM12.5 12.5H21V21h-8.5z"/></svg>
    </span><span class="tag">{t('Premium')}</span></div>
    {#if login}
      <div>
        <div class="code" title={t('Copy')}>
          <button class="ghost codebtn" onclick={copyCode}>{login.userCode}</button>
        </div>
        <p class="muted small">
          {t('Enter this code at microsoft.com/link and approve the sign-in. Waiting…')}
          {#if copied}<strong>{t('Copied')}</strong>{/if}
        </p>
      </div>
      <div class="acts">
        <button class="primary" onclick={() => login && api.openUrl(login.verificationUri)}>{t('Open page')}</button>
        <button class="ghost" onclick={cancelLogin}>{t('Cancel')}</button>
      </div>
    {:else}
      <div>
        <div class="name">Microsoft</div>
        <p class="muted small">{t('For playing with your official Minecraft account. Sign in with a code, no password in the launcher.')}</p>
      </div>
      <div class="acts"><button class="primary" onclick={signIn} disabled={starting}>{t('Sign in')}</button></div>
    {/if}
  </article>
</div>
{/if}

<style>
  .head { display: grid; gap: 6px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 16px; }
  .acard { display: grid; gap: 22px; align-content: space-between; min-height: 230px; padding: 22px; border: 1px solid var(--line); border-radius: 26px; background: var(--surface); transition: border-color var(--ease), transform 300ms var(--spring); }
  .acard:hover { transform: translateY(-3px); }
  .acard.active { border-color: var(--text); }
  .top { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
  .tags { display: flex; flex-wrap: wrap; gap: 6px; justify-content: flex-end; }
  .tag.on { background: var(--inv-bg); color: var(--inv-text); border-color: var(--inv-bg); }
  .name { display: flex; align-items: center; gap: 8px; min-width: 0; font-size: 24px; font-weight: 650; letter-spacing: -0.04em; }
  .nm { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .uuid { margin-top: 4px; font-family: inherit; font-size: 11px; color: var(--faint); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .acts { display: flex; gap: 8px; }
  .add { border-style: dashed; background: transparent; }
  .plus { display: grid; place-items: center; width: 68px; height: 68px; border-radius: 16px; border: 1px dashed var(--line-strong); font-size: 30px; color: var(--muted); font-weight: 300; }
  .soon { background: transparent; }
  .soon.waiting { border-color: var(--text); }
  .code { margin-bottom: 8px; }
  .codebtn { font-size: 26px; font-weight: 650; letter-spacing: 0.12em; padding: 6px 12px; }
  .ms { display: grid; place-items: center; width: 68px; height: 68px; border-radius: 16px; border: 1px solid var(--line-strong); color: var(--muted); }
</style>
