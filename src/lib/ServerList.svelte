<script lang="ts">
  import type { Snippet } from 'svelte'
  import { api, type SavedServer } from './api'
  import { app, askConfirm, invite, isBusy, notify, play } from './store.svelte'
  import { t } from './i18n.svelte'
  import { reveal } from './actions'
  import { ping } from './serverstatus.svelte'
  import { copyText, ctx, type MenuItem } from './contextmenu'
  import AddServerModal from './AddServerModal.svelte'
  import ServerCard from './ServerCard.svelte'
  import Icon from './Icon.svelte'

  let { instanceId, heading, head, onload }: { instanceId?: string; heading?: string; head?: Snippet<[number]>; onload?: (count: number) => void } = $props()

  let servers = $state<SavedServer[]>([])
  let loaded = $state(false)
  let adding = $state(false)
  let spin = $state(0)

  const names = $derived(Object.fromEntries(app.instances.map((i) => [i.id, i.name])))
  const busy = $derived(instanceId ? isBusy(instanceId) : false)
  const ids = $derived(instanceId ? [instanceId] : app.instances.map((i) => i.id))
  const playing = $derived(ids.filter((id) => app.games[id]?.state === 'running').join())

  async function load(force = false) {
    servers = await api.listServers(instanceId).catch(() => [])
    loaded = true
    onload?.(servers.length)
    for (const s of servers) ping(s.address, force)
  }

  $effect(() => {
    void instanceId
    void app.instances.length
    void playing
    load()
  })

  function refresh() {
    spin++
    load(true)
  }

  function join(s: SavedServer) {
    if (instanceId) play(instanceId, { kind: 'server', address: s.address })
    else app.joining = s
  }

  function serverMenu(s: SavedServer): MenuItem[] {
    return [
      { label: t('Join'), icon: 'play', onclick: () => join(s), disabled: busy || (!!instanceId && !app.accounts.active) },
      { label: t('Invite friends'), icon: 'share', onclick: () => invite(s, instanceId), disabled: !instanceId && !s.instances.length },
      { label: t('Copy address'), icon: 'copy', onclick: () => void copyText(s.address).then((ok) => ok && notify(t('Copied'))) },
      { label: t('Refresh'), icon: 'refresh', onclick: () => ping(s.address, true) },
      'sep',
      { label: t('Remove'), icon: 'trash', danger: true, onclick: () => void remove(s), disabled: busy },
    ]
  }

  async function remove(s: SavedServer) {
    const targets = instanceId ? [instanceId] : s.instances
    const msg = instanceId
      ? t('Remove "{name}" from this instance?', { name: s.name })
      : targets.length === 1
        ? t('Remove "{name}" from {instance}?', { name: s.name, instance: names[targets[0]] ?? '' })
        : t('Remove "{name}" from {n} instances?', { name: s.name, n: targets.length })
    if (!(await askConfirm(msg, t('Remove')))) return
    try {
      await api.removeServer(targets, s.address)
      servers = servers.filter((x) => x !== s)
    } catch (e) {
      notify(e)
    }
  }
</script>

<section class="servers">
  <div class="shead">
    {#if head}{@render head(servers.length)}{:else}<h2><Icon name="server" size={20} />{heading ?? t('Servers')}{#if servers.length}<span class="count">{servers.length}</span>{/if}</h2>{/if}
    <div class="row ctl">
      {#if servers.length}
        <button class="ghost iconbtn" onclick={refresh} aria-label={t('Refresh')} title={t('Refresh')}>
          <span class="spin" style:--r="{spin * 360}deg"><Icon name="refresh" size={16} /></span>
        </button>
      {/if}
      <button onclick={() => (adding = true)} disabled={busy}><Icon name="plus" size={15} />{t('Add a server')}</button>
    </div>
  </div>

  {#if loaded && !servers.length}
    <div class="empty" use:reveal>
      <p>{t('No servers yet')}</p>
      <p class="muted small">{instanceId ? t('Servers you add here or in the game show up in this list.') : t('Add a server once and join it from any instance.')}</p>
    </div>
  {/if}

  {#if servers.length}
    <div class="grid">
      {#each servers as s, n (s.address)}
        <ServerCard
          name={s.name}
          address={s.address}
          icon={s.icon}
          index={n}
          menu={() => serverMenu(s)}
        >
          {#snippet actions()}
            <button class="primary" onclick={() => join(s)} disabled={busy || !app.instances.length || !app.accounts.active}><Icon name="play" size={13} />{t('Play')}</button>
            <button onclick={() => invite(s, instanceId)} disabled={!instanceId && !s.instances.length}><Icon name="share" size={13} />{t('Invite')}</button>
            <button class="ghost iconbtn" onclick={() => remove(s)} disabled={busy} aria-label={t('Remove')} title={t('Remove')}><Icon name="trash" size={16} /></button>
          {/snippet}
        </ServerCard>
      {/each}
    </div>
  {/if}
</section>

{#if adding}<AddServerModal {instanceId} onClose={() => (adding = false)} onAdded={() => load()} />{/if}

<style>
  .servers { display: grid; gap: 14px; }
  .shead { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .shead h2 { margin: 0; display: flex; align-items: center; gap: 10px; color: var(--text); font-size: 18px; font-weight: 600; letter-spacing: -0.03em; }
  .count { font-size: 12px; font-weight: 500; color: var(--muted); border: 1px solid var(--line-strong); border-radius: 999px; padding: 1px 8px; }
  .ctl { gap: 8px; }
  .ctl button { display: inline-flex; align-items: center; gap: 8px; }
  .spin { display: grid; place-items: center; line-height: 0; transform: rotate(var(--r)); transition: transform 700ms var(--spring); }
  .spin :global(svg) { display: block; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(270px, 1fr)); gap: 16px; }
  .empty { display: grid; justify-items: center; gap: 6px; text-align: center; padding: 34px 20px; border: 1px dashed var(--line-strong); border-radius: 20px; }
</style>
