<script lang="ts">
  import { thumb } from './img'
  import type { ProjectHit } from './api'
  import Placeholder from './Placeholder.svelte'
  import Icon from './Icon.svelte'
  import { locale, t } from './i18n.svelte'
  import { api } from './api'
  import { copyText, ctx, type MenuItem } from './contextmenu'
  import { notify, photoFor } from './store.svelte'

  let { hit, busy = false, disabled = false, installed = false, update = false, index, onInstall, onOpen }: {
    hit: ProjectHit
    busy?: boolean
    disabled?: boolean
    installed?: boolean
    update?: boolean
    index?: number
    onInstall: () => void
    onOpen?: () => void
  } = $props()

  const photo = $derived(hit.bannerUrl ?? photoFor(hit.latestMc ?? undefined, hit.title))
  const fmt = (n: number) => new Intl.NumberFormat(locale(), { notation: 'compact' }).format(n)
  const tags = $derived(hit.categories.slice(0, 3))
  const link = $derived(`https://modrinth.com/project/${hit.slug}`)
  const menu = (): MenuItem[] => [
    ...(onOpen ? [{ label: t('Details'), icon: 'list' as const, onclick: onOpen }] : []),
    { label: t('Install'), icon: 'download', onclick: onInstall, disabled: busy || disabled },
    'sep',
    { label: t('Open on Modrinth'), icon: 'globe', onclick: () => void api.openUrl(link).catch(notify) },
    { label: t('Copy Modrinth link'), icon: 'copy', onclick: () => void copyText(link).then((ok) => ok && notify(t('Copied'))) },
  ]
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1)
</script>

<article class="card" use:ctx={menu} class:rise={index !== undefined} style:--d="{(index ?? 0) * 35}ms">
  <div class="media">
    {#if photo}
      <img class="photo" use:thumb={{ src: photo, w: 640 }} alt="" decoding="async" />
    {:else}
      <Placeholder />
    {/if}
    <span class="dl"><Icon name="download" size={13} />{fmt(hit.downloads)}</span>
    {#if installed}<span class="ins"><Icon name="check" size={12} />{t('Installed')}</span>{/if}
  </div>

  {#if onOpen}<button class="open" onclick={onOpen} aria-label={t('Open {name}', { name: hit.title })}></button>{/if}

  <div class="body">
    <div class="title">{hit.title}</div>
    <p class="desc">{hit.description}</p>
    <div class="tags">
      {#each tags as c}<span class="tag">{cap(c)}</span>{/each}
    </div>
    <div class="foot">
      <button class="sm" class:primary={!installed || update} onclick={onInstall} {disabled}>{busy ? t('Installing…') : update ? t('Update') : installed ? t('Installed') : t('Install')}</button>
      {#if hit.latestMc}<span class="ver">{hit.latestMc}</span>{/if}
    </div>
  </div>
</article>

<style>
  .card { position: relative; contain: paint; display: grid; grid-template-rows: 120px 1fr; height: 100%; border-radius: 18px; overflow: hidden; border: 1px solid var(--line); background: var(--surface); transition: transform 380ms var(--spring), border-color var(--ease); }
  .card.rise { animation: rise 520ms var(--spring) backwards var(--d); }
  @keyframes rise { from { opacity: 0; transform: translateY(22px) scale(0.97); } }
  .card:hover { transform: translateY(-4px); border-color: var(--line-strong); }
  .media { position: relative; overflow: hidden; background: #0a0a0a; }
  .photo { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; transition: transform 900ms var(--spring); }
  .card:hover .photo { transform: scale(1.06); }
  .dl { position: absolute; top: 10px; right: 10px; display: inline-flex; align-items: center; gap: 5px; font-size: 12px; color: #fff; background: rgba(0, 0, 0, 0.6); border-radius: 999px; padding: 4px 10px; }
  .open { position: absolute; inset: 0; width: 100%; height: 100%; padding: 0; border: 0; border-radius: 0; background: transparent; cursor: pointer; }
  .open:hover:not(:disabled) { background: transparent; }
  .open:focus-visible { outline: 2px solid var(--text); outline-offset: -4px; }
  .body { display: grid; align-content: start; gap: 6px; padding: 14px 14px 14px; pointer-events: none; }
  .title { font-size: 15.5px; font-weight: 650; letter-spacing: -0.025em; line-height: 1.2; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; min-height: 2.4em; }
  .desc { font-size: 12.5px; line-height: 1.4; color: var(--muted); margin: 0; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; min-height: 2.8em; }
  .tags { display: flex; flex-wrap: wrap; gap: 6px; min-height: 22px; margin-top: 2px; }
  .foot { display: flex; align-items: center; justify-content: space-between; margin-top: 6px; }
  .foot button { pointer-events: auto; }
  .ver { font-size: 12.5px; color: var(--muted); }
  .sm { padding: 7px 20px; }
  .ins { position: absolute; bottom: 10px; left: 10px; display: inline-flex; align-items: center; gap: 5px; font-size: 11px; font-weight: 600; color: var(--inv-text); background: var(--inv-bg); border-radius: 999px; padding: 3px 10px 3px 8px; }
</style>
