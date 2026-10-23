<script lang="ts">
  import type { Snippet } from 'svelte'
  import Art from './Art.svelte'
  import Count from './Count.svelte'
  import Icon from './Icon.svelte'
  import Motd from './Motd.svelte'
  import { ctx, type MenuItem } from './contextmenu'
  import { locale, t } from './i18n.svelte'
  import { pingOf, signal } from './serverstatus.svelte'
  import { thumb } from './img'
  import { photoFor } from './store.svelte'

  let {
    name,
    address,
    icon = null,
    note = '',
    tags = [],
    sponsored = false,
    index = 0,
    menu,
    actions,
  }: {
    name: string
    address: string
    icon?: string | null
    note?: string
    tags?: string[]
    sponsored?: boolean
    index?: number
    menu: () => MenuItem[]
    actions: Snippet
  } = $props()

  const p = $derived(pingOf(address))
  const st = $derived(p?.state === 'online' ? p.status : null)
  const fav = $derived(st?.favicon ?? icon)
  const level = $derived(st ? signal(st.latencyMs) : 0)
  const offline = $derived(p?.state === 'offline')
  const photo = $derived(photoFor(undefined, address))
</script>

<article class="card" class:off={offline} style:--d="{Math.min(index, 12) * 40}ms" use:ctx={menu}>
  <div class="cover">
    {#if fav}
      <img class="bg" src={fav} alt="" />
      <img class="fav" src={fav} alt="" />
    {:else if photo}
      <img class="photo" data-bare use:thumb={{ src: photo, w: 640 }} alt="" decoding="async" />
    {:else}
      <Art seed={address} cubes={false} />
    {/if}
    <span class="shade"></span>
    <span class="status" class:online={!!st}>
      <span class="dot"></span>{st ? t('Online') : offline ? t('Offline') : '…'}
    </span>
    <span class="ping" class:wave={!p || p.state === 'pending'} title={st ? `${st.latencyMs} ms` : ''}>
      <svg viewBox="0 0 20 16" aria-hidden="true">
        {#each [0, 1, 2, 3] as b}<rect x={b * 5 + 1} y={12 - b * 3.6} width="3.2" height={b * 3.6 + 3.5} rx="1" class:on={b < level} style:--b={b} />{/each}
      </svg>
      {#if st}{st.latencyMs} ms{/if}
    </span>
    {#if sponsored}<span class="spon">{t('Sponsored')}</span>{/if}
  </div>
  <div class="body">
    <div class="title">
      <span class="nm" title={name}>{name}</span>
      <span class="addr muted small" title={address}>{address}</span>
    </div>
    <div class="motd">
      {#if note}<span class="note">{note}</span>
      {:else if st}<Motd text={st.motd || st.version} />
      {:else if offline}<span class="muted">{t('Offline or unreachable')}</span>
      {:else}<span class="ghostline"></span><span class="ghostline short"></span>{/if}
    </div>
    <div class="facts">
      {#if st}
        <span class="players" title={st.sample.join('\n')}>
          <Icon name="user" size={13} /><Count value={st.online} format={(v) => Math.round(v).toLocaleString(locale())} /><span class="muted">/ {st.max.toLocaleString(locale())}</span>
        </span>
      {/if}
      {#each tags as tag}<span class="tag">{tag}</span>{/each}
    </div>
    {#if st}<span class="meter"><span style:--w="{st.max > 0 ? Math.min(1, st.online / st.max) * 100 : 0}%"></span></span>{/if}
  </div>
  <div class="acts">{@render actions()}</div>
</article>

<style>
  .card { position: relative; display: grid; grid-template-rows: auto 1fr auto; border-radius: 22px; border: 1px solid var(--line); background: var(--surface); overflow: hidden; transition: border-color var(--ease), transform 380ms var(--spring); animation: cardin 460ms var(--spring) var(--d) backwards; min-width: 0; }
  .card:hover { border-color: var(--line-strong); transform: translateY(-3px); }
  @keyframes cardin { from { opacity: 0; transform: translateY(12px); } }
  .cover { position: relative; aspect-ratio: 16 / 7; display: grid; place-items: center; overflow: hidden; background: #000; }
  .cover :global(.art) { position: absolute; inset: 0; }
  .bg { position: absolute; inset: -10%; width: 120%; height: 120%; object-fit: cover; opacity: 0.55; image-rendering: auto; }
  .fav { position: relative; z-index: 1; width: 64px; height: 64px; border-radius: 14px; image-rendering: pixelated; box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45); }
  .photo { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; transition: transform 900ms var(--spring); }
  .card:hover .photo { transform: scale(1.05); }
  .shade { position: absolute; inset: 0; background: linear-gradient(to bottom, rgba(0, 0, 0, 0.35), transparent 45%, rgba(0, 0, 0, 0.35)); }
  .status, .ping, .spon { position: absolute; z-index: 2; display: inline-flex; align-items: center; gap: 6px; height: 24px; padding: 0 10px; border-radius: 999px; font-size: 11.5px; color: #fff; background: rgba(0, 0, 0, 0.55); }
  .status { top: 10px; left: 10px; }
  .status .dot { width: 7px; height: 7px; border-radius: 50%; background: rgba(255, 255, 255, 0.4); }
  .status.online .dot { background: #3ecf6e; }
  .ping { top: 10px; right: 10px; }
  .ping svg { width: 18px; height: 14px; }
  .ping rect { fill: rgba(255, 255, 255, 0.3); }
  .ping rect.on { fill: #fff; }
  .ping.wave rect { animation: wave 1s ease-in-out calc(var(--b) * 120ms) infinite; }
  @keyframes wave { 0%, 100% { fill: rgba(255, 255, 255, 0.3); } 40% { fill: rgba(255, 255, 255, 0.8); } }
  .spon { bottom: 10px; left: 10px; color: #1a1300; background: #f4c542; font-weight: 600; }
  .body { display: grid; gap: 10px; padding: 14px 16px 4px; min-width: 0; align-content: start; }
  .title { display: grid; min-width: 0; }
  .nm { font-weight: 600; font-size: 15px; letter-spacing: -0.02em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .addr { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .motd { min-height: 38px; font-size: 13px; line-height: 1.45; color: var(--muted); overflow: hidden; display: grid; gap: 6px; align-content: start; }
  .note { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .ghostline { display: block; width: 80%; height: 10px; border-radius: 6px; background: var(--surface-2); }
  .ghostline.short { width: 50%; }
  .facts { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 8px; min-height: 22px; }
  .players { display: inline-flex; align-items: center; gap: 5px; font-size: 13px; font-weight: 500; margin-right: 4px; }
  .meter { display: block; height: 3px; border-radius: 3px; background: var(--line); overflow: hidden; }
  .meter span { display: block; height: 100%; width: var(--w); border-radius: inherit; background: var(--text); transition: width 600ms var(--spring); }
  .acts { display: flex; align-items: center; gap: 8px; padding: 12px 16px 16px; }
  .acts :global(button) { display: inline-flex; align-items: center; justify-content: center; gap: 7px; height: 36px; padding: 0 16px; font-size: 13.5px; line-height: 1; }
  .acts :global(button.iconbtn) { width: 36px; padding: 0; margin-left: auto; }
  .off .body { opacity: 0.6; }
</style>
