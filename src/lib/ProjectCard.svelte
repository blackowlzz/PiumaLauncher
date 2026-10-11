<script lang="ts">
  import { thumb } from './img'
  import type { ProjectHit } from './api'
  import LoaderCover from './LoaderCover.svelte'
  import Icon from './Icon.svelte'
  import { locale, t } from './i18n.svelte'
  import { photoFor } from './store.svelte'

  let { hit, busy = false, disabled = false, index, onInstall, onOpen }: {
    hit: ProjectHit
    busy?: boolean
    disabled?: boolean
    index?: number
    onInstall: () => void
    onOpen?: () => void
  } = $props()

  const photo = $derived(hit.bannerUrl ?? photoFor(hit.latestMc ?? undefined, hit.title))
  const fmt = (n: number) => new Intl.NumberFormat(locale(), { notation: 'compact' }).format(n)
</script>

<article class="card" class:rise={index !== undefined} style:--d="{(index ?? 0) * 35}ms">
  {#if photo}
    <img class="photo" use:thumb={{ src: photo, w: 640 }} alt="" decoding="async" />
  {:else}
    <LoaderCover loader="vanilla" />
  {/if}
  <div class="veil"></div>
  {#if onOpen}<button class="open" onclick={onOpen} aria-label={t('Open {name}', { name: hit.title })}></button>{/if}

  <div class="top">
    {#if hit.iconUrl}<img class="icon" use:thumb={{ src: hit.iconUrl, w: 96 }} alt="" width="40" height="40" decoding="async" />{:else}<span></span>{/if}
    <span class="dl"><Icon name="download" size={13} />{fmt(hit.downloads)}</span>
  </div>

  <div class="bottom">
    <div class="title">{hit.title}</div>
    <div class="by">{hit.author}{hit.latestMc ? ` · ${hit.latestMc}` : ''}</div>
    <p class="desc">{hit.description}</p>
    <button class="primary sm" onclick={onInstall} {disabled}>{busy ? t('Installing…') : t('Install')}</button>
  </div>
</article>

<style>
  .card { position: relative; contain: paint; aspect-ratio: 4 / 5; border-radius: 24px; overflow: hidden; border: 1px solid var(--line); background: #0a0a0a; color: #fff; isolation: isolate; transition: transform 380ms var(--spring), border-color var(--ease); }
  .card.rise { animation: rise 520ms var(--spring) backwards var(--d); }
  @keyframes rise { from { opacity: 0; transform: translateY(22px) scale(0.97); } }
  .card:hover { transform: translateY(-5px); border-color: rgba(255, 255, 255, 0.35); }
  .photo { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; z-index: -2; transition: transform 900ms var(--spring); }
  .card:hover .photo { transform: scale(1.06); }
  .veil { position: absolute; inset: 0; z-index: -1; background: linear-gradient(to top, rgba(0, 0, 0, 0.92) 0%, rgba(0, 0, 0, 0.55) 38%, rgba(0, 0, 0, 0) 68%); transition: opacity 300ms; }
  .card:hover .icon { transform: scale(1.12) rotate(-5deg); }
  .card:hover .veil { background: linear-gradient(to top, rgba(0, 0, 0, 0.95) 0%, rgba(0, 0, 0, 0.78) 55%, rgba(0, 0, 0, 0.1) 100%); }
  .open { position: absolute; inset: 0; width: 100%; height: 100%; padding: 0; border: 0; border-radius: 0; background: transparent; cursor: pointer; }
  .open:hover:not(:disabled) { background: transparent; }
  .open:focus-visible { outline: 2px solid #fff; outline-offset: -4px; }
  .top, .desc { pointer-events: none; }
  .top { position: absolute; top: 14px; left: 14px; right: 14px; display: flex; align-items: center; justify-content: space-between; }
  .icon { transition: transform 420ms var(--spring); border-radius: 12px; border: 2px solid rgba(255, 255, 255, 0.9); background: #111; object-fit: cover; }
  .dl { display: inline-flex; align-items: center; gap: 5px; font-size: 12px; background: rgba(0, 0, 0, 0.55); border-radius: 999px; padding: 4px 10px; }
  .bottom { position: absolute; left: 16px; right: 16px; bottom: 16px; display: grid; gap: 4px; pointer-events: none; }
  .bottom button { pointer-events: auto; }
  .title { font-size: 19px; font-weight: 650; letter-spacing: -0.035em; line-height: 1.15; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .by { font-size: 12px; opacity: 0.7; }
  .desc { position: absolute; left: 0; right: 0; bottom: calc(100% + 8px); font-size: 13px; line-height: 1.4; margin: 0; opacity: 0; transform: translateY(6px); transition: opacity 200ms, transform 250ms var(--spring); pointer-events: none; display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; text-shadow: 0 1px 8px rgba(0, 0, 0, 0.8); }
  .card:hover .desc, .card:focus-within .desc { opacity: 0.92; transform: none; }
  .sm { justify-self: start; padding: 8px 18px; margin-top: 6px; }
</style>
