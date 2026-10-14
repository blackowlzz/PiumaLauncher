<script lang="ts">
  import { api, type SavedServer } from './api'
  import { app, askConfirm, isBusy, notify, play } from './store.svelte'
  import { locale, t } from './i18n.svelte'
  import { reveal } from './actions'
  import { ping, pingOf, signal } from './serverstatus.svelte'
  import { copyText, ctx, type MenuItem } from './contextmenu'
  import AddServerModal from './AddServerModal.svelte'
  import Count from './Count.svelte'
  import Icon from './Icon.svelte'
  import Motd from './Motd.svelte'

  let { instanceId }: { instanceId?: string } = $props()

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
    <h2><Icon name="server" size={20} />{t('Servers')}{#if servers.length}<span class="count">{servers.length}</span>{/if}</h2>
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
    <div class="table" class:solo={!!instanceId}>
      <div class="th">
        <span></span>
        <span>{t('Server')}</span>
        <span>{t('Players')}</span>
        <span>{t('Ping')}</span>
        {#if !instanceId}<span>{t('Instance')}</span>{/if}
        <span class="r">{t('Actions')}</span>
      </div>
      {#each servers as s, n (s.address)}
        {@const p = pingOf(s.address)}
        {@const st = p?.state === 'online' ? p.status : null}
        {@const level = st ? signal(st.latencyMs) : 0}
        <div class="tr" class:off={p?.state === 'offline'} class:pending={!p || p.state === 'pending'} class:anim={n < 12} style:--d="{n * 30}ms" use:ctx={() => serverMenu(s)}>
          <div class="fav">
            {#if st?.favicon ?? s.icon}<img src={st?.favicon ?? s.icon} alt="" />{:else}<Icon name="server" size={20} />{/if}
          </div>
          <div class="info">
            <div class="top"><span class="nm">{s.name}</span><span class="addr muted small">{s.address}</span></div>
            <div class="d">
              {#if st}<Motd text={st.motd || st.version} compact />
              {:else if p?.state === 'offline'}<span class="muted">{t('Offline or unreachable')}</span>
              {:else}<span class="ghostline"></span>{/if}
            </div>
          </div>
          <div class="players">
            {#if st}
              <div class="pn" title={st.sample.join('\n')}><Count value={st.online} format={(v) => Math.round(v).toLocaleString(locale())} /> <span class="muted">/ {st.max.toLocaleString(locale())}</span></div>
              <span class="meter"><span style:--w="{st.max > 0 ? Math.min(1, st.online / st.max) * 100 : 0}%"></span></span>
              {#if st.version}<div class="ver muted small">{st.version}</div>{/if}
            {:else}
              <span class="muted">—</span>
            {/if}
          </div>
          <span class="sig" class:wave={!p || p.state === 'pending'} title={st ? `${st.latencyMs} ms` : ''} aria-label={st ? `${st.latencyMs} ms` : t('Offline or unreachable')}>
            <svg viewBox="0 0 20 16" aria-hidden="true">
              {#each [0, 1, 2, 3] as b}<rect x={b * 5 + 1} y={12 - b * 3.6} width="3.2" height={b * 3.6 + 3.5} rx="1" class:on={b < level} style:--b={b} />{/each}
            </svg>
            {#if st}<span class="ms">{st.latencyMs} ms</span>{/if}
          </span>
          {#if !instanceId}
            <div class="inst">
              {#each s.instances.slice(0, 1) as id}<span class="iname" title={s.instances.map((x) => names[x] ?? x).join(', ')}>{names[id] ?? id}</span>{/each}
              {#if s.instances.length > 1}<span class="muted small more">+{s.instances.length - 1}</span>{/if}
            </div>
          {/if}
          <div class="act">
            <button class="ghost iconbtn" onclick={() => remove(s)} disabled={busy} aria-label={t('Remove')} title={t('Remove')}><Icon name="trash" size={16} /></button>
            <button class="primary joinb" onclick={() => join(s)} disabled={busy || !app.instances.length || !app.accounts.active}><Icon name="play" size={13} />{t('Play')}</button>
          </div>
        </div>
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
  .table { border: 1px solid var(--line); border-radius: 20px; background: var(--surface); overflow: hidden; }
  .th, .tr { display: grid; grid-template-columns: 44px minmax(0, 2fr) minmax(0, 1fr) 96px minmax(0, 0.8fr) 156px; align-items: center; gap: 14px; padding: 0 18px 0 14px; }
  .solo .th, .solo .tr { grid-template-columns: 44px minmax(0, 2fr) minmax(0, 1fr) 96px 156px; }
  .th { height: 44px; border-bottom: 1px solid var(--line); font-size: 13px; font-weight: 600; color: var(--muted); }
  .th .r { text-align: right; }
  .tr { min-height: 72px; padding-block: 10px; border-bottom: 1px solid var(--line); transition: background var(--ease), opacity 300ms ease; }
  .tr:last-child { border-bottom: 0; }
  .tr:hover { background: var(--surface-2); }
  .tr.anim { animation: rowin 460ms var(--spring) backwards var(--d); }
  @keyframes rowin { from { opacity: 0; transform: translateY(10px); } }
  .tr.off .info, .tr.off .players { opacity: 0.5; }
  .fav { width: 44px; height: 44px; display: grid; place-items: center; border-radius: 12px; overflow: hidden; background: var(--surface-2); color: var(--faint); }
  .fav img { width: 100%; height: 100%; image-rendering: pixelated; }
  .info { min-width: 0; }
  .top { display: flex; align-items: baseline; gap: 10px; min-width: 0; }
  .nm { font-weight: 600; letter-spacing: -0.02em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .addr { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .d { color: var(--muted); font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-height: 18px; }
  .ghostline { display: block; width: 55%; height: 10px; margin-top: 4px; border-radius: 6px; background: var(--surface-2); }
  .players { min-width: 0; display: grid; gap: 5px; }
  .pn { font-weight: 500; }
  .meter { width: 100%; max-width: 110px; height: 3px; border-radius: 3px; background: var(--line); overflow: hidden; }
  .meter span { display: block; height: 100%; width: var(--w); border-radius: inherit; background: var(--text); }
  .ver { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sig { display: inline-flex; align-items: center; gap: 8px; color: var(--muted); font-size: 12px; }
  .sig svg { width: 22px; height: 18px; flex: none; }
  .sig rect { fill: var(--line-strong); }
  .sig rect.on { fill: var(--text); }
  .off .sig rect { fill: var(--line); }
  .sig.wave rect { animation: wave 1s ease-in-out calc(var(--b) * 120ms) infinite; }
  @keyframes wave { 0%, 100% { fill: var(--line-strong); } 40% { fill: var(--muted); } }
  .inst { display: flex; align-items: center; gap: 6px; min-width: 0; }
  .iname { font-size: 13px; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .more { flex: none; }
  .act { display: flex; align-items: center; justify-content: flex-end; gap: 6px; }
  .joinb { display: inline-flex; align-items: center; gap: 7px; padding: 8px 18px; }
  .empty { display: grid; justify-items: center; gap: 6px; text-align: center; padding: 34px 20px; border: 1px dashed var(--line-strong); border-radius: 20px; }
  @media (max-width: 900px) {
    .th { display: none; }
    .tr, .solo .tr { grid-template-columns: 44px minmax(0, 1fr) 156px; }
    .players, .inst { display: none; }
  }
</style>
