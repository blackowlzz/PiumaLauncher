<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type SponsoredServer } from './api'
  import ServerList from './ServerList.svelte'
  import ServerRow from './ServerRow.svelte'
  import { copyText, type MenuItem } from './contextmenu'
  import { t } from './i18n.svelte'
  import { ping } from './serverstatus.svelte'
  import { app, notify } from './store.svelte'

  let { filter = '', instance = 'all' }: { filter?: string; instance?: string } = $props()

  let list: ServerList | undefined = $state()
  export const add = () => list?.add()
  export const refresh = () => list?.reload()

  onMount(() => {
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

  const spon = $derived.by(() => {
    const q = filter.trim().toLowerCase()
    return (app.sponsored ?? []).filter((s) => !q || `${s.name} ${s.address} ${s.description}`.toLowerCase().includes(q))
  })
</script>

<ServerList bind:this={list} heading={t('Your servers')} {filter} {instance} rows controls={false} />
{#if spon.length}
  <section class="sponsored">
    <div class="shead">
      <h2>{t('Sponsored servers')}</h2>
      <span class="muted small">{t('Picked by the PiumaLauncher team.')}</span>
    </div>
    <div class="rowsgrid">
      {#each spon as s, n (s.address)}
        <ServerRow name={s.name} address={s.address} note={s.description} tags={s.version ? [s.version] : []} sponsored index={n} menu={() => menu(s)} onplay={() => join(s)} canPlay={app.instances.length > 0 && !!app.accounts.active} />
      {/each}
    </div>
  </section>
{/if}

<style>
  .sponsored { display: grid; gap: 14px; }
  .shead h2 { margin: 0; color: var(--text); font-size: 18px; font-weight: 600; letter-spacing: -0.03em; }
  .shead { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; }
  .rowsgrid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 440px), 1fr)); gap: 12px; }
</style>
