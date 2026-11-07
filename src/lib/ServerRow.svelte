<script lang="ts">
  import Art from './Art.svelte'
  import Count from './Count.svelte'
  import Icon from './Icon.svelte'
  import Motd from './Motd.svelte'
  import { ctx, type MenuItem } from './contextmenu'
  import { locale, t } from './i18n.svelte'
  import { pingOf, signal } from './serverstatus.svelte'
  import { app } from './store.svelte'

  let {
    name,
    address,
    icon = null,
    note = '',
    tags = [],
    sponsored = false,
    index = 0,
    canPlay = true,
    menu,
    onplay,
  }: {
    name: string
    address: string
    icon?: string | null
    note?: string
    tags?: string[]
    sponsored?: boolean
    index?: number
    canPlay?: boolean
    menu: () => MenuItem[]
    onplay: () => void
  } = $props()

  const p = $derived(pingOf(address))
  const st = $derived(p?.state === 'online' ? p.status : null)
  const fav = $derived(st?.favicon ?? icon)
  const level = $derived(st ? signal(st.latencyMs) : 0)
  const offline = $derived(p?.state === 'offline')
  const generic = $derived(/^(minecraft[ -]server|server di minecraft|servidor (de|do) minecraft|serveur minecraft|serwer minecraft|сервер minecraft)$/i.test(name.trim()))
  const title = $derived(generic ? address : name)

  function more(e: MouseEvent) {
    e.stopPropagation()
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
    app.ctx = { x: r.left, y: r.bottom + 6, items: menu() }
  }
</script>

<article class="srow" class:off={offline} style:--d="{Math.min(index, 12) * 35}ms" use:ctx={menu}>
  <div class="ico">
    {#if fav}<img src={fav} alt="" />{:else}<Art seed={address} cubes={false} />{/if}
    <span class="dot" class:on={!!st} class:pending={!p || p.state === 'pending'}></span>
  </div>
  <div class="main">
    <div class="top">
      <span class="nm" title={name}>{title}</span>
      {#if sponsored}<span class="spon">{t('Sponsored')}</span>{/if}
      {#each tags as tag}<span class="tag">{tag}</span>{/each}
    </div>
    <div class="motd">
      {#if note}<span class="note">{note}</span>
      {:else if st}<Motd text={st.motd || st.version} compact />
      {:else if offline}<span class="muted">{t('Offline or unreachable')}</span>
      {:else}<span class="ghostline"></span>{/if}
    </div>
    <div class="sub muted small">
      <span class="ping" class:wave={!p || p.state === 'pending'} title={st ? `${st.latencyMs} ms` : ''}>
        <svg viewBox="0 0 20 16" aria-hidden="true">
          {#each [0, 1, 2, 3] as b}<rect x={b * 5 + 1} y={12 - b * 3.6} width="3.2" height={b * 3.6 + 3.5} rx="1" class:on={b < level} style:--b={b} />{/each}
        </svg>
        {#if st}{st.latencyMs} ms{/if}
      </span>
      {#if st}
        <span class="players" title={st.sample.join('\n')}><Icon name="user" size={12} /><Count value={st.online} format={(v) => Math.round(v).toLocaleString(locale())} /><span>/ {st.max.toLocaleString(locale())}</span></span>
      {/if}
      {#if !generic}<span class="addr" title={address}>{address}</span>{/if}
    </div>
  </div>
  <div class="acts">
    <button class="primary play" onclick={onplay} disabled={!canPlay}><Icon name="play" size={12} />{t('Play')}</button>
    <button class="ghost iconbtn" onclick={more} aria-label={t('More')} title={t('More')}><Icon name="dots" size={16} /></button>
  </div>
</article>

<style>
  .srow { display: flex; align-items: center; gap: 14px; padding: 12px 14px 12px 12px; border-radius: 18px; border: 1px solid var(--line); background: var(--surface); min-width: 0; transition: border-color var(--ease), background var(--ease); animation: rowin 420ms var(--spring) var(--d) backwards; }
  .srow:hover { border-color: var(--line-strong); background: var(--surface-2); }
  @keyframes rowin { from { opacity: 0; transform: translateY(8px); } }
  .ico { position: relative; flex: none; width: 56px; height: 56px; border-radius: 14px; overflow: hidden; background: #000; }
  .ico img { width: 100%; height: 100%; image-rendering: pixelated; display: block; }
  .ico :global(.art) { position: absolute; inset: 0; }
  .dot { position: absolute; right: 4px; bottom: 4px; width: 10px; height: 10px; border-radius: 50%; background: #6e6e6e; box-shadow: 0 0 0 2px var(--surface); }
  .dot.on { background: #3ecf6e; }
  .dot.pending { animation: blink 1s ease-in-out infinite; }
  @keyframes blink { 50% { opacity: 0.35; } }
  .srow:hover .dot { box-shadow: 0 0 0 2px var(--surface-2); }
  .main { flex: 1; min-width: 0; display: grid; gap: 3px; }
  .top { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .nm { font-weight: 600; font-size: 15px; letter-spacing: -0.02em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .spon { flex: none; font-size: 10.5px; font-weight: 600; padding: 1px 8px; border-radius: 999px; color: #1a1300; background: #f4c542; }
  .tag { flex: none; }
  .motd { min-height: 19px; font-size: 13px; color: var(--muted); overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .note { display: block; overflow: hidden; text-overflow: ellipsis; }
  .ghostline { display: block; width: 60%; height: 9px; margin-top: 5px; border-radius: 6px; background: var(--surface-2); }
  .sub { display: flex; align-items: center; gap: 14px; min-width: 0; margin-top: 2px; }
  .players { display: inline-flex; align-items: center; gap: 4px; color: var(--text); font-weight: 500; flex: none; }
  .players span { color: var(--muted); font-weight: 400; }
  .addr { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .ping { flex: none; display: inline-flex; align-items: center; gap: 5px; }
  .ping svg { width: 16px; height: 12px; }
  .ping rect { fill: var(--line-strong); }
  .ping rect.on { fill: var(--text); }
  .ping.wave rect { animation: wave 1s ease-in-out calc(var(--b) * 120ms) infinite; }
  @keyframes wave { 0%, 100% { fill: var(--line-strong); } 40% { fill: var(--faint); } }
  .acts { flex: none; display: flex; align-items: center; gap: 4px; }
  .play { display: inline-flex; align-items: center; gap: 7px; height: 34px; padding: 0 18px; font-size: 13.5px; }
  .iconbtn { width: 34px; height: 34px; padding: 0; display: grid; place-items: center; }
  .off .main { opacity: 0.6; }
</style>
