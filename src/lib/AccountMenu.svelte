<script lang="ts">
  import Head from './Head.svelte'
  import Icon from './Icon.svelte'
  import { api } from './api'
  import { t } from './i18n.svelte'
  import { activeAccount, app, notify, reloadAccounts } from './store.svelte'

  let open = $state(false)
  let root: HTMLElement | undefined = $state()
  const account = $derived(activeAccount())

  async function choose(id: string) {
    open = false
    try {
      await api.setActiveAccount(id)
      await reloadAccounts()
    } catch (e) {
      notify(e)
    }
  }
  const manage = () => {
    open = false
    app.page = 'accounts'
  }
  const onWindowClick = (e: MouseEvent) => {
    if (open && root && !root.contains(e.target as Node)) open = false
  }
</script>

<svelte:window onclick={onWindowClick} onkeydown={(e) => e.key === 'Escape' && (open = false)} />

<div class="wrap" bind:this={root}>
  {#if account}
    <button class="trigger ghost" onclick={() => (open = !open)} aria-haspopup="menu" aria-expanded={open} aria-label={t('Account: {name}', { name: account.name })}>
      <Head name={account.name} size={34} />
      <span class="n">{account.name}</span>
      <span class="chev" class:up={open}><Icon name="chevron" size={13} /></span>
    </button>
  {:else}
    <button class="primary" onclick={manage}>{t('Add account')}</button>
  {/if}

  {#if open}
    <div class="menu" role="menu">
      {#each app.accounts.accounts as a, n (a.id)}
        <button class="opt" role="menuitemradio" aria-checked={a.id === app.accounts.active} onclick={() => choose(a.id)} style:--n={n}>
          <Head name={a.name} size={32} />
          <span class="who"><span class="n">{a.name}</span><span class="kind">{a.kind === 'offline' ? t('Offline') : t('Premium')}</span></span>
          {#if a.id === app.accounts.active}<span class="tick"><Icon name="check" size={15} /></span>{/if}
        </button>
      {/each}
      <button class="opt manage" role="menuitem" onclick={manage}>{t('Manage accounts')}</button>
    </div>
  {/if}
</div>

<style>
  .wrap { position: relative; }
  .trigger { display: flex; align-items: center; gap: 10px; padding: 5px 12px 5px 5px; text-align: left; color: var(--text); }
  .who { display: grid; line-height: 1.2; }
  .n { font-weight: 600; }
  .kind { font-size: 12px; color: var(--muted); }
  .chev { display: inline-grid; color: var(--muted); transform: rotate(-90deg); transition: transform 250ms var(--spring); }
  .chev.up { transform: rotate(90deg); }
  .menu { position: absolute; right: 0; bottom: calc(100% + 10px); z-index: 30; min-width: 250px; padding: 6px; display: grid; gap: 2px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 20px; box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5); animation: pop 220ms var(--spring) both; transform-origin: bottom right; }
  .opt { display: flex; align-items: center; gap: 12px; width: 100%; border-color: transparent; border-radius: 14px; padding: 8px 10px; text-align: left; animation: item 260ms var(--spring) both; animation-delay: calc(var(--n, 0) * 30ms); }
  .opt[aria-checked='true'] { background: var(--surface-2); }
  .tick { margin-left: auto; display: inline-grid; }
  .manage { color: var(--muted); border-top: 1px solid var(--line); border-radius: 0 0 14px 14px; margin-top: 4px; padding: 11px 14px; animation: none; }
  @keyframes pop { from { opacity: 0; transform: translateY(8px) scale(0.96); } }
  @keyframes item { from { opacity: 0; transform: translateY(6px); } }
</style>
