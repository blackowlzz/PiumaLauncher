<script lang="ts">
  import { api, type RecentWorld } from './api'
  import { app, isBusy, play } from './store.svelte'
  import { t } from './i18n.svelte'
  import { reveal, spotlight } from './actions'
  import { relativeTime } from './meta'
  import { opensWorlds } from './serverstatus.svelte'
  import Art from './Art.svelte'
  import Icon from './Icon.svelte'
  import ServerList from './ServerList.svelte'
  import Strip from './Strip.svelte'

  const modes = ['Survival', 'Creative', 'Adventure', 'Spectator']
  let worlds = $state<RecentWorld[]>([])
  const byId = $derived(Object.fromEntries(app.instances.map((i) => [i.id, i])))
  const shown = $derived(worlds.filter((w) => byId[w.instanceId]))
  const playing = $derived(Object.values(app.games).filter((g) => g.state === 'running').length)

  $effect(() => {
    void playing
    void app.instances.length
    api.recentWorlds(12).then((w) => (worlds = w)).catch(() => {})
  })

  function open(w: RecentWorld) {
    const inst = byId[w.instanceId]
    if (inst && opensWorlds(inst.mcVersion)) play(w.instanceId, { kind: 'world', folder: w.folder })
    else play(w.instanceId)
  }
</script>

<header class="intro">
  <h1>{t('Quick Play')}</h1>
  <h2>{t('Jump straight into a world or a server.')}</h2>
</header>

{#if shown.length}
  <Strip title={t('Recent worlds')} icon="clock" cols="minmax(230px, 250px)">
    {#each shown as w, n (`${w.instanceId}/${w.folder}`)}
      {@const inst = byId[w.instanceId]}
      {@const direct = opensWorlds(inst.mcVersion)}
      <div class="world" use:spotlight use:reveal={n}>
        <div class="pic">
          {#if w.icon}<img src={w.icon} alt="" />{:else}<Art seed={w.folder} />{/if}
          {#if w.hardcore}<span class="hc">{t('Hardcore')}</span>{/if}
        </div>
        <div class="wb">
          <div class="wn">{w.name}</div>
          <div class="muted small line1">{inst.name}</div>
          <div class="muted small">{w.mode !== null && modes[w.mode] ? t(modes[w.mode]) : ''}{w.mode !== null && modes[w.mode] ? ' · ' : ''}{relativeTime(w.modifiedMs)}</div>
        </div>
        <button class="primary go" onclick={() => open(w)} disabled={isBusy(w.instanceId) || !app.accounts.active} title={direct ? '' : t('Opening a world directly needs Minecraft 1.20 or newer')}>
          <Icon name="play" size={13} />{direct ? t('Play') : t('Launch')}
        </button>
      </div>
    {/each}
  </Strip>
{/if}

<ServerList />

<style>
  .intro h1 { animation: swapin 500ms var(--spring) backwards; }
  .world { position: relative; display: grid; grid-template-rows: auto 1fr auto; gap: 12px; padding: 12px; height: 100%; border-radius: 18px; background: var(--surface); border: 1px solid var(--line); transition: border-color var(--ease), transform 300ms var(--spring); }
  .world:hover { border-color: var(--line-strong); transform: translateY(-2px); }
  .pic { position: relative; aspect-ratio: 16 / 9; border-radius: 12px; overflow: hidden; background: #000; display: grid; }
  .pic img { width: 100%; height: 100%; object-fit: cover; image-rendering: pixelated; transition: transform 600ms var(--spring); }
  .pic :global(.art) { position: absolute; inset: 0; }
  .world:hover .pic img { transform: scale(1.06); }
  .hc { position: absolute; top: 8px; left: 8px; font-size: 10.5px; font-weight: 650; letter-spacing: 0.04em; text-transform: uppercase; color: #fff; background: #c4262b; border-radius: 999px; padding: 2px 9px; animation: gopop 360ms var(--spring) 200ms backwards; }
  .wb { display: grid; gap: 2px; min-width: 0; }
  .wn { font-weight: 650; letter-spacing: -0.02em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .line1 { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .go { display: inline-flex; align-items: center; justify-content: center; gap: 7px; padding: 8px 16px; }
</style>
