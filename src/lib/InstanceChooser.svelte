<script lang="ts">
  import { fade } from 'svelte/transition'
  import type { SavedServer } from './api'
  import { app, isBusy, play } from './store.svelte'
  import { t } from './i18n.svelte'
  import { loaderNames } from './meta'
  import { pingOf, rankInstances } from './quickplay.svelte'
  import Cover from './Cover.svelte'
  import Icon from './Icon.svelte'

  let { server, onClose }: { server: SavedServer; onClose: () => void } = $props()

  const p = $derived(pingOf(server.address))
  const status = $derived(p?.state === 'online' ? p.status : null)
  const ranked = $derived(rankInstances(app.instances, server.instances, status?.version))
  const icon = $derived(status?.favicon ?? server.icon)
  let chosen = $state<string | null>(null)
  let list: HTMLElement | undefined = $state()

  $effect(() => {
    list?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus()
  })

  async function go(id: string) {
    if (chosen) return
    chosen = id
    await play(id, { kind: 'server', address: server.address })
    setTimeout(onClose, 420)
  }

  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Escape' && !chosen) {
      e.preventDefault()
      onClose()
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="backdrop" role="presentation" transition:fade={{ duration: 160 }} onclick={() => !chosen && onClose()}></div>
<div class="dialog" role="dialog" aria-modal="true" aria-labelledby="ic-title" out:fade={{ duration: 160 }}>
  <div class="head">
    <div class="fav">{#if icon}<img src={icon} alt="" />{:else}<Icon name="server" size={22} />{/if}</div>
    <div class="grow">
      <h3 id="ic-title">{t('Join {name}', { name: server.name })}</h3>
      <p class="muted small">{t('Choose which instance to play with.')}{#if status?.version}<span class="tag ver">{status.version}</span>{/if}</p>
    </div>
    <button class="ghost iconbtn" onclick={onClose} disabled={!!chosen} aria-label={t('Close')}><Icon name="x" size={16} /></button>
  </div>

  {#if ranked.length}
    <ul class="list" class:picked={!!chosen} bind:this={list}>
      {#each ranked as r, n (r.inst.id)}
        {@const busy = isBusy(r.inst.id)}
        <li style:--n={n}>
          <button class="opt" class:on={chosen === r.inst.id} disabled={busy || !!chosen} onclick={() => go(r.inst.id)}>
            {@render row(r, busy)}
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="muted">{t('Create an instance first.')}</p>
  {/if}
</div>

{#snippet row(r: (typeof ranked)[number], busy: boolean)}
  <span class="thumb"><Cover inst={r.inst} /></span>
  <span class="meta">
    <span class="nm">{r.inst.name}</span>
    <span class="muted small">{r.inst.mcVersion}{r.inst.loader !== 'vanilla' ? ` · ${loaderNames[r.inst.loader]}` : ''}</span>
  </span>
  <span class="badges">
    {#if busy}<span class="tag">{t('Running')}</span>{/if}
    {#if r.match === 'exact'}<span class="tag good">{t('Same version')}</span>{:else if r.match === 'supported'}<span class="tag good">{t('Supported')}</span>{/if}
    {#if r.saved}<span class="tag">{t('Saved here')}</span>{/if}
  </span>
  <span class="go"><Icon name="play" size={14} /></span>
{/snippet}

<style>
  .backdrop { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.62); z-index: 50; }
  .dialog { position: fixed; z-index: 51; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(500px, calc(100vw - 32px)); max-height: calc(100vh - 48px); display: grid; grid-template-rows: auto minmax(0, 1fr); gap: 16px; padding: 22px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 20px; box-shadow: 0 30px 90px rgba(0, 0, 0, 0.55); animation: ic-pop 300ms var(--spring) both; }
  .head { display: flex; align-items: center; gap: 14px; }
  .fav { width: 48px; height: 48px; flex: none; display: grid; place-items: center; border-radius: 12px; overflow: hidden; background: var(--surface-2); border: 1px solid var(--line); color: var(--muted); animation: gopop 420ms var(--spring) 80ms backwards; }
  .fav img { width: 100%; height: 100%; image-rendering: pixelated; }
  h3 { margin: 0 0 3px; font-size: 18px; font-weight: 650; letter-spacing: -0.03em; overflow-wrap: anywhere; }
  .ver { margin-left: 8px; font-size: 10.5px; padding: 1px 8px; vertical-align: 1px; }
  .list { list-style: none; margin: -4px -6px; padding: 4px 6px; display: grid; gap: 6px; overflow: auto; }
  .list li { animation: swapin 360ms var(--spring) calc(var(--n) * 40ms + 60ms) backwards; }
  .opt { display: flex; align-items: center; gap: 12px; width: 100%; text-align: left; padding: 8px 12px 8px 8px; border-radius: 14px; border-color: var(--line); transition: background var(--ease), border-color var(--ease), opacity 260ms ease, transform 260ms var(--spring); }
  .opt:disabled { color: inherit; }
  .picked .opt:not(.on) { opacity: 0.3; transform: scale(0.98); }
  .opt.on { border-color: var(--text); }
  .opt.on::after { content: ''; position: absolute; inset: 0; background: linear-gradient(100deg, transparent 30%, rgba(var(--glow), 0.14) 50%, transparent 70%); animation: shimmer 700ms ease-in-out infinite; }
  .thumb { position: relative; width: 44px; height: 44px; border-radius: 10px; overflow: hidden; flex: none; display: grid; place-items: center; }
  .meta { display: grid; min-width: 0; flex: 1; }
  .nm { font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .badges { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 4px; }
  .badges .tag { font-size: 10.5px; padding: 1px 8px; }
  .good { color: var(--text); border-color: var(--text); }
  .go { display: inline-grid; place-items: center; width: 30px; height: 30px; border-radius: 50%; flex: none; color: var(--muted); transition: transform 300ms var(--spring), background var(--ease), color var(--ease); }
  .opt:hover:not(:disabled) .go, .opt.on .go { background: var(--inv-bg); color: var(--inv-text); transform: translateX(2px) scale(1.06); }
  @keyframes ic-pop { from { opacity: 0; transform: translate(-50%, -47%) scale(0.97); } }
</style>
