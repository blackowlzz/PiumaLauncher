<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type SponsoredServer } from './api'
  import Icon from './Icon.svelte'
  import ServerCard from './ServerCard.svelte'
  import ServerList from './ServerList.svelte'
  import { copyText, type MenuItem } from './contextmenu'
  import { t } from './i18n.svelte'
  import { ping } from './serverstatus.svelte'
  import { app, notify } from './store.svelte'

  const VIEW_KEY = 'piuma-servers-view'
  let view: 'mine' | 'sponsored' = $state(readView())
  let mine = $state(0)
  const hasSponsored = $derived(!!app.sponsored?.length)

  function readView(): 'mine' | 'sponsored' {
    try {
      return localStorage.getItem(VIEW_KEY) === 'sponsored' ? 'sponsored' : 'mine'
    } catch {
      return 'mine'
    }
  }

  function setView(next: 'mine' | 'sponsored') {
    view = next
    try {
      localStorage.setItem(VIEW_KEY, next)
    } catch {}
  }

  onMount(() => {
    api.listServers().then((l) => (mine = l.length)).catch(() => {})
    if (app.sponsored === null) {
      api.sponsoredServers()
        .then((list) => (app.sponsored = list))
        .catch(() => (app.sponsored = []))
    }
  })

  $effect(() => {
    for (const s of app.sponsored ?? []) ping(s.address)
  })

  const join = (s: SponsoredServer) => (app.joining = { name: s.name, address: s.address, icon: null, instances: [] })

  const menu = (s: SponsoredServer): MenuItem[] => [
    { label: t('Join'), icon: 'play', onclick: () => join(s), disabled: !app.instances.length || !app.accounts.active },
    { label: t('Copy address'), icon: 'copy', onclick: () => void copyText(s.address).then((ok) => ok && notify(t('Copied'))) },
    { label: t('Refresh'), icon: 'refresh', onclick: () => ping(s.address, true) },
  ]
</script>

<header class="intro">
  <h1>{t('Servers')}</h1>
  <h2>{t('Your servers and the ones we recommend, with live players and ping.')}</h2>
</header>

{#snippet toggle(count: number)}
  <div class="seg views" role="group" aria-label={t('Servers')}>
    <button aria-pressed={view === 'mine'} onclick={() => setView('mine')}><Icon name="server" size={16} />{t('Your servers')}<span class="count">{count}</span></button>
    <button aria-pressed={view === 'sponsored'} onclick={() => setView('sponsored')}><Icon name="flame" size={16} />{t('Sponsored servers')}<span class="count">{app.sponsored?.length ?? 0}</span></button>
  </div>
{/snippet}

{#if view === 'mine' || !hasSponsored}
  <ServerList heading={t('Your servers')} head={hasSponsored ? toggle : undefined} onload={(n) => (mine = n)} />
{:else}
  <section class="sponsored">
    <div class="shead">
      {@render toggle(mine)}
      <span class="muted small">{t('Picked by the PiumaLauncher team.')}</span>
    </div>
    <div class="grid">
      {#each app.sponsored ?? [] as s, n (s.address)}
        <ServerCard name={s.name} address={s.address} note={s.description} tags={s.version ? [s.version] : []} sponsored index={n} menu={() => menu(s)}>
          {#snippet actions()}
            <button class="primary" onclick={() => join(s)} disabled={!app.instances.length || !app.accounts.active}><Icon name="play" size={13} />{t('Play')}</button>
            <button onclick={() => void copyText(s.address).then((ok) => ok && notify(t('Copied')))}><Icon name="copy" size={13} />{t('Copy address')}</button>
          {/snippet}
        </ServerCard>
      {/each}
    </div>
  </section>
{/if}

<style>
  .intro h1 { animation: swapin 500ms var(--spring) backwards; }
  .sponsored { display: grid; gap: 14px; }
  .shead { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; }
  .views { padding: 4px; }
  .views button { display: inline-flex; align-items: center; gap: 8px; height: 38px; padding: 0 16px; font-size: 14.5px; font-weight: 600; letter-spacing: -0.01em; }
  .views button[aria-pressed='true'] .count { color: var(--text); border-color: var(--faint); }
  .count { font-size: 12px; font-weight: 500; color: var(--muted); border: 1px solid var(--line-strong); border-radius: 999px; padding: 1px 8px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(270px, 1fr)); gap: 16px; }
</style>
