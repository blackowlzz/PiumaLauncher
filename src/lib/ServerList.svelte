<script lang="ts">
  import { api, type SavedServer } from './api'
  import { app, askConfirm, isBusy, notify, play } from './store.svelte'
  import { locale, t } from './i18n.svelte'
  import { reveal, spotlight } from './actions'
  import { ping, pingOf, signal } from './quickplay.svelte'
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
      <div class="stack" aria-hidden="true"><span></span><span></span><span></span></div>
      <p>{t('No servers yet')}</p>
      <p class="muted small">{instanceId ? t('Servers you add here or in the game show up in this list.') : t('Add a server once and join it from any instance.')}</p>
    </div>
  {/if}

  <div class="list">
    {#each servers as s, n (s.address)}
      {@const p = pingOf(s.address)}
      {@const st = p?.state === 'online' ? p.status : null}
      {@const level = st ? signal(st.latencyMs) : 0}
      <div class="srv" class:pending={!p || p.state === 'pending'} class:off={p?.state === 'offline'} use:spotlight use:reveal={n}>
        <div class="fav">
          {#if st?.favicon ?? s.icon}<img src={st?.favicon ?? s.icon} alt="" />{:else}<Icon name="server" size={22} />{/if}
        </div>
        <div class="body">
          <div class="top">
            <span class="nm">{s.name}</span>
            <span class="addr muted small">{s.address}</span>
          </div>
          {#if st}
            <Motd text={st.motd || st.version} />
            <div class="facts muted small">
              <span class="players" title={st.sample.join('\n')}>
                <span class="live"></span><Count value={st.online} format={(v) => Math.round(v).toLocaleString(locale())} /> / {st.max.toLocaleString(locale())}
                <span class="meter"><span style:--w="{st.max > 0 ? Math.min(1, st.online / st.max) * 100 : 0}%"></span></span>
              </span>
              {#if st.version}<span class="ver">{st.version}</span>{/if}
            </div>
          {:else if p?.state === 'offline'}
            <div class="muted small">{t('Offline or unreachable')}</div>
          {:else}
            <div class="ghostline"></div>
          {/if}
          {#if !instanceId}
            <div class="chips">
              {#each s.instances as id}<span class="tag">{names[id] ?? id}</span>{/each}
            </div>
          {/if}
        </div>
        <div class="side">
          <span class="sig" class:wave={!p || p.state === 'pending'} title={st ? `${st.latencyMs} ms` : ''} aria-label={st ? `${st.latencyMs} ms` : t('Offline or unreachable')}>
            <svg viewBox="0 0 20 16" aria-hidden="true">
              {#each [0, 1, 2, 3] as b}<rect x={b * 5 + 1} y={12 - b * 3.6} width="3.2" height={b * 3.6 + 3.5} rx="1" class:on={b < level} style:--b={b} />{/each}
            </svg>
            {#if st}<span class="ms">{st.latencyMs} ms</span>{/if}
          </span>
          <div class="acts">
            <button class="ghost iconbtn" onclick={() => remove(s)} disabled={busy} aria-label={t('Remove')} title={t('Remove')}><Icon name="trash" size={15} /></button>
            <button class="primary joinb" onclick={() => join(s)} disabled={busy || !app.instances.length || !app.accounts.active}><Icon name="play" size={13} />{t('Play')}</button>
          </div>
        </div>
      </div>
    {/each}
  </div>
</section>

{#if adding}<AddServerModal {instanceId} onClose={() => (adding = false)} onAdded={() => load()} />{/if}

<style>
  .servers { display: grid; gap: 14px; }
  .shead { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .shead h2 { margin: 0; display: flex; align-items: center; gap: 10px; color: var(--text); font-size: 18px; font-weight: 600; letter-spacing: -0.03em; }
  .count { font-size: 12px; font-weight: 500; color: var(--muted); border: 1px solid var(--line-strong); border-radius: 999px; padding: 1px 8px; animation: gopop 320ms var(--spring) backwards; }
  .ctl { gap: 8px; }
  .ctl button { display: inline-flex; align-items: center; gap: 8px; }
  .spin { display: grid; place-items: center; line-height: 0; transform: rotate(var(--r)); transition: transform 700ms var(--spring); }
  .spin :global(svg) { display: block; }
  .list { display: grid; gap: 10px; }
  .srv { display: grid; grid-template-columns: 64px minmax(0, 1fr) auto; gap: 16px; align-items: start; padding: 14px; border-radius: 18px; background: var(--surface); border: 1px solid var(--line); overflow: hidden; transition: border-color var(--ease), opacity 300ms ease; }
  .srv:hover { border-color: var(--line-strong); }
  .srv.off { opacity: 0.62; }
  .srv.pending::before { content: ''; position: absolute; inset: 0; z-index: 0; pointer-events: none; background: linear-gradient(100deg, transparent 35%, rgba(var(--glow), 0.05) 50%, transparent 65%); animation: shimmer 1.3s ease-in-out infinite; }
  .fav { width: 64px; height: 64px; display: grid; place-items: center; border-radius: 14px; overflow: hidden; background: var(--surface-2); border: 1px solid var(--line); color: var(--faint); }
  .fav img { width: 100%; height: 100%; image-rendering: pixelated; animation: gopop 420ms var(--spring) backwards; }
  .body { display: grid; gap: 8px; min-width: 0; }
  .top { display: flex; align-items: baseline; gap: 10px; min-width: 0; }
  .nm { font-weight: 650; font-size: 15.5px; letter-spacing: -0.02em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .addr { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .facts { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 16px; animation: swapin 360ms var(--spring) 120ms backwards; }
  .players { display: inline-flex; align-items: center; gap: 6px; color: var(--text); }
  .live { width: 7px; height: 7px; border-radius: 50%; background: #3ecf6e; animation: beat 1.8s ease-out infinite; }
  @keyframes beat { 0% { box-shadow: 0 0 0 0 rgba(62, 207, 110, 0.55); } 70%, 100% { box-shadow: 0 0 0 6px rgba(62, 207, 110, 0); } }
  .meter { width: 54px; height: 4px; border-radius: 4px; background: var(--line); overflow: hidden; margin-left: 4px; }
  .meter span { display: block; height: 100%; width: var(--w); border-radius: inherit; background: var(--text); animation: fillw 1s var(--spring) 200ms backwards; }
  @keyframes fillw { from { width: 0; } }
  .ver { max-width: 260px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .ghostline { height: 38px; border-radius: 10px; background: var(--surface-2); }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .chips .tag { font-size: 11px; }
  .side { display: grid; justify-items: end; align-content: space-between; gap: 14px; height: 100%; }
  .sig { display: inline-flex; align-items: center; gap: 8px; color: var(--muted); font-size: 12px; }
  .sig svg { width: 22px; height: 18px; }
  .sig rect { fill: var(--line-strong); transform-box: fill-box; transform-origin: bottom; transition: fill 300ms ease; }
  .sig rect.on { fill: var(--text); animation: bar 520ms var(--spring) calc(var(--b) * 80ms) backwards; }
  .off .sig rect { fill: var(--line); }
  .sig.wave rect { animation: wave 1s ease-in-out calc(var(--b) * 120ms) infinite; }
  @keyframes bar { from { transform: scaleY(0); } }
  @keyframes wave { 0%, 100% { fill: var(--line-strong); } 40% { fill: var(--muted); } }
  .ms { min-width: 46px; text-align: right; animation: swapin 300ms var(--spring) backwards; }
  .acts { display: flex; align-items: center; gap: 6px; }
  .joinb { display: inline-flex; align-items: center; gap: 7px; padding: 8px 18px; }
  .empty { display: grid; justify-items: center; gap: 6px; text-align: center; padding: 34px 20px; border: 1px dashed var(--line-strong); border-radius: 18px; }
  .stack { display: grid; gap: 4px; margin-bottom: 10px; }
  .stack span { width: 64px; height: 14px; border-radius: 5px; border: 1px solid var(--line-strong); background: var(--surface-2); animation: float 2.6s ease-in-out calc(var(--i, 0) * 200ms) infinite; }
  .stack span:nth-child(2) { --i: 1; }
  .stack span:nth-child(3) { --i: 2; }
  @keyframes float { 50% { transform: translateX(4px); } }
  @media (max-width: 700px) { .srv { grid-template-columns: 48px minmax(0, 1fr); } .fav { width: 48px; height: 48px; } .side { grid-column: 1 / -1; grid-auto-flow: column; justify-content: space-between; align-items: center; } }
</style>
