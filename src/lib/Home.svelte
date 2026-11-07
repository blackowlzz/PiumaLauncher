<script lang="ts">
  import { thumb } from './img'
  import Art from './Art.svelte'
  import Cover from './Cover.svelte'
  import Count from './Count.svelte'
  import Icon from './Icon.svelte'
  import { ctx } from './contextmenu'
  import { instanceItems } from './instanceMenu'
  import ProjectCard from './ProjectCard.svelte'
  import Strip from './Strip.svelte'
  import { reveal } from './actions'
  import { locale, t } from './i18n.svelte'
  import { onMount } from 'svelte'
  import { activeAccount, app, installModpack, isBusy, openInstance, openProject, play, playInfo, playedTodaySecs, refreshEffective, refreshStartup, seconds, selectedInstance, stop, weeklyPlaytime } from './store.svelte'
  import { formatDuration, loaderNames, profileName } from './meta'

  const inst = $derived(selectedInstance())
  const game = $derived(inst ? app.games[inst.id] : undefined)
  const running = $derived(game?.state === 'running')
  const samples = $derived(inst ? (app.stats[inst.id] ?? []) : [])
  const eff = $derived(inst ? app.effective[inst.id] : undefined)
  const ram = $derived(running && samples.length ? samples[samples.length - 1] : null)
  $effect(() => {
    if (!inst) return
    void [inst.customPerformance, inst.memoryMaxMb, inst.flagProfile, app.settings.autoOptimize, app.settings.defaultMemoryMaxMb, app.settings.defaultFlagProfile, running]
    refreshEffective(inst.id)
  })
  const startup = $derived(inst ? app.startup[inst.id] : undefined)
  $effect(() => {
    if (inst?.id) refreshStartup(inst.id)
  })
  function openLimits() {
    app.section = 'playtime'
    app.page = 'settings'
  }
  let today = $state(0)
  $effect(() => {
    void app.sessions
    void app.games
    today = playedTodaySecs()
    const id = setInterval(() => (today = playedTodaySecs()), 30_000)
    return () => clearInterval(id)
  })
  const shownInstances = $derived(
    app.instances
      .map((i) => ({ inst: i, last: playInfo(i.id).last }))
      .sort((a, b) => b.last - a.last || a.inst.name.localeCompare(b.inst.name))
      .slice(0, 4),
  )
  const tiles = $derived(
    app.instances
      .filter((i) => i.id !== inst?.id)
      .map((i) => ({ inst: i, last: playInfo(i.id).last }))
      .sort((a, b) => b.last - a.last || a.inst.name.localeCompare(b.inst.name))
      .slice(0, 4),
  )
  const account = $derived(activeAccount())
  const week = $derived(weeklyPlaytime())
  const weekTotal = $derived(week.reduce((n, d) => n + d.secs, 0))
  const weekMax = $derived(Math.max(1, ...week.map((d) => d.secs)))
  const dayName = (d: Date) => d.toLocaleDateString(locale(), { weekday: 'short' }).replace('.', '')
  const isToday = (d: Date) => d.toDateString() === new Date().toDateString()
  const weekAvg = $derived(weekTotal / 7)
  const limitSecs = $derived(app.settings.dailyLimitMinutes * 60)
  const words = $derived((inst?.name ?? t('Your Minecraft, lightweight.')).split(/\s+/))

  const slides = $derived(app.featured.filter((h) => h.bannerUrl).slice(0, 6))
  let slideIdx = $state(0)
  const current = $derived(slides.length ? slides[slideIdx % slides.length] : null)
  const shown = $derived.by(() => {
    const n = slides.length
    if (!n) return []
    const cur = slideIdx % n
    const prev = (((slideIdx - 1) % n) + n) % n
    return slides.map((h, i) => ({ h, i })).filter(({ i }) => i === cur || i === prev)
  })
  let ready = $state<Record<string, boolean>>({})
  function slide(node: HTMLImageElement, id: string) {
    const show = () => requestAnimationFrame(() => requestAnimationFrame(() => (ready[id] = true)))
    if (node.complete && node.naturalWidth) show()
    else node.addEventListener('load', show, { once: true })
    return { destroy: () => delete ready[id] }
  }
  onMount(() => {
    const id = setInterval(() => {
      if (!document.hidden) slideIdx++
    }, 6500)
    return () => clearInterval(id)
  })
</script>

<div class="home">
  <section class="hero" class:photo={!!inst?.bannerUrl || (!inst && slides.length > 0)}>
    <div class="bg">
      {#if inst}
        <Cover {inst} big />
      {:else if slides.length}
        {#each shown as { h, i } (h.projectId)}
          <img class="slide" class:on={i === slideIdx % slides.length && ready[h.projectId]} use:thumb={{ src: h.bannerUrl ?? '', w: 1280 }} alt="" decoding="async" use:slide={h.projectId} />
        {/each}
      {:else}
        <Art seed="piuma" />
      {/if}
      <div class="scrim"></div>
    </div>
    <div class="copy">
      {#if !inst}<p class="greet" use:reveal>{t('Welcome')}{account ? `, ${account.name}` : ''}</p>{/if}
      {#key inst?.id ?? 'none'}
        <h1 class="title" aria-label={words.join(' ')}>
          {#each words as w, i}<span class="w" aria-hidden="true"><span style:--i={i}>{w}</span></span>{' '}{/each}
        </h1>
      {/key}

      {#if inst}
        <ul class="facts" use:reveal={3}>
          <li><Icon name="box" size={16} />Minecraft {inst.mcVersion}</li>
          {#if inst.loader !== 'vanilla'}<li><Icon name="wrench" size={16} />{loaderNames[inst.loader]}</li>{/if}
          <li><Icon name="cpu" size={16} />{eff?.memoryMaxMb ?? inst.memoryMaxMb} MB · {profileName(eff?.flagProfile ?? inst.flagProfile)}</li>
          {#if startup?.usualMs ?? startup?.lastMs}<li><Icon name="clock" size={16} />{t('Starts in about {time}', { time: seconds((startup?.usualMs ?? startup?.lastMs)!) })}</li>{/if}
          {#if ram !== null}<li class="live"><span class="dot"></span>{t('RAM in use: {ram} MB', { ram })}</li>{/if}
          {#if app.packUpdates[inst.id]}<li><button class="updlink" onclick={() => openInstance(inst.id)}><Icon name="download" size={14} />{t('Update available')}: {app.packUpdates[inst.id].versionNumber}</button></li>{/if}
        </ul>
        <div class="cta" use:reveal={4}>
          <button class="big glass" onclick={() => openInstance(inst.id)}><Icon name="settings" size={16} />{t('Manage instance')}</button>
        </div>
      {:else}
        <p class="lead" use:reveal={3}>{t('Create an instance or install a modpack: the launcher downloads the right Java for you.')}</p>
        <div class="cta" use:reveal={4}>
          <button class="primary big" onclick={() => (app.page = 'library')}>{t('Create an instance')}</button>
          <button class="big glass" onclick={() => (app.page = 'explore')}>{t('Browse modpacks')}</button>
        </div>
      {/if}
    </div>
    {#if !inst && current}
      <div class="credit" use:reveal={5}>
        <div class="ct">{current.title}</div>
        <div class="cb">{t('by {author}', { author: current.author })}</div>
        <div class="row">
          <button class="glass" onclick={() => installModpack(current.projectId, undefined, current.latestMc)} disabled={app.installingId !== null}>
            {app.installingId === current.projectId ? t('Installing…') : t('Install')}
          </button>
          <button class="glass" onclick={() => openProject(current)}>{t('Details')}</button>
        </div>
      </div>
    {/if}
  </section>

  {#if shownInstances.length}
  <div class="duo" class:solo={!tiles.length}>
    {#if tiles.length}
    <section class="block" use:reveal={1}>
      <div class="shead">
        <h2>{t('Your instances')}</h2>
        <button class="ghost more link" onclick={() => (app.page = 'library')}>{t('View all')} →</button>
      </div>
      <div class="panel tiles">
        {#each tiles as { inst: i } (i.id)}
          {@const g = app.games[i.id]}
          {@const secs = playInfo(i.id).secs}
          <div class="tile" role="button" tabindex="0" title={i.name} onclick={() => (app.selectedId = i.id)} ondblclick={() => openInstance(i.id)} onkeydown={(e) => e.key === 'Enter' && (app.selectedId = i.id)} use:ctx={() => instanceItems(i)}>
            <div class="tcover"><Cover inst={i} /></div>
            <span class="tshade"></span>
            {#if g?.state === 'running'}<span class="tlive"><span class="dot"></span>{t('Running')}</span>{/if}
            <div class="tinfo">
              <span class="tname">{i.name}</span>
              <span class="tmeta">{i.mcVersion}{i.loader !== 'vanilla' ? ` · ${loaderNames[i.loader]}` : ''}{secs >= 60 ? ` · ${formatDuration(secs)}` : ''}</span>
            </div>
            {#if g?.state === 'running'}
              <button class="tplay" onclick={(e) => (e.stopPropagation(), stop(i.id))} aria-label={t('Stop')} title={t('Stop')}><Icon name="x" size={16} /></button>
            {:else}
              <button class="tplay" onclick={(e) => (e.stopPropagation(), play(i.id))} disabled={isBusy(i.id)} aria-label={t('Play')} title={t('Play')}><Icon name="play" size={16} /></button>
            {/if}
          </div>
        {/each}
      </div>
    </section>
    {/if}

    {@render playtime()}
  </div>
  {/if}

  {#if app.featured.length}{@render popular()}{/if}
</div>

{#snippet popular()}
  <Strip icon="flame" title={t('Popular modpacks')} action={t('View all')} onAction={() => (app.page = 'explore')}>
    {#each app.featured as h, n (h.projectId)}
      <div use:reveal={n}>
        <ProjectCard hit={h} busy={app.installingId === h.projectId} disabled={app.installingId !== null} onInstall={() => installModpack(h.projectId, undefined, h.latestMc)} onOpen={() => openProject(h)} />
      </div>
    {/each}
  </Strip>
{/snippet}

{#snippet playtime()}
  <section class="block" use:reveal={2}>
    <div class="shead"><h2><Icon name="clock" size={20} />{t('Playtime')}</h2><span class="muted small">{t('Last 7 days')}</span></div>
    <div class="panel play p2">
      <div class="phead">
        <div class="total"><Count value={weekTotal} format={(n) => formatDuration(n)} /></div>
        <span class="muted small">{weekAvg >= 60 ? t('{time} a day on average', { time: formatDuration(weekAvg) }) : t('No games in the last 7 days')}</span>
      </div>
      <div class="bars" role="img" aria-label={t('Playtime over the last 7 days')}>
        {#each week as d, i}
          <div class="bar" class:today={isToday(d.date)} class:l={i < 2} class:r={i > 4}>
            <div class="col"><div class="fill" style:--h="{d.secs ? Math.max(6, (d.secs / weekMax) * 100) : 0}%" style:--i={i}></div></div>
            <span class="lab">{dayName(d.date)}</span>
            <span class="tip" role="tooltip"><span class="td">{d.date.toLocaleDateString(locale(), { weekday: 'long', day: 'numeric', month: 'short' })}</span><span class="tv">{d.secs ? formatDuration(d.secs) : t('No playtime')}</span></span>
          </div>
        {/each}
      </div>
      <div class="foot">
        <div class="row tl">
          <span class="muted">{t('Today')}</span>
          <span class="tnum">{formatDuration(today)}{#if limitSecs > 0}<span class="muted"> / {formatDuration(limitSecs)}</span>{/if}</span>
          <span class="grow"></span>
          <button class="link" onclick={openLimits}>{t('Breaks and limits')}<Icon name="chevron" size={12} /></button>
        </div>
        {#if limitSecs > 0}<div class="line"><div style:width="{Math.min(100, (today / limitSecs) * 100)}%"></div></div>{/if}
      </div>
    </div>
  </section>
{/snippet}

<style>
  .home { display: flex; flex-direction: column; gap: 22px; width: 100%; max-width: 1400px; margin: 0 auto; }
  .hero { position: relative; display: flex; align-items: flex-end; justify-content: space-between; gap: 24px; min-height: 300px; padding: 40px 44px; border-radius: 20px; overflow: hidden; border: 1px solid var(--line); flex: none; color: #fff; isolation: isolate; }
  .bg { position: absolute; inset: 0; z-index: -1; overflow: hidden; background: #000; }
  .slide { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; opacity: 0; transition: opacity 1600ms ease; }
  .slide.on { opacity: 1; }
  .credit { position: relative; align-self: flex-end; display: grid; gap: 2px; justify-items: end; text-align: right; }
  .ct { font-size: 15px; font-weight: 600; letter-spacing: -0.02em; }
  .cb { font-size: 12px; opacity: 0.7; margin-bottom: 10px; }
  .scrim { position: absolute; inset: 0; background: linear-gradient(90deg, rgba(0, 0, 0, 0.82) 0%, rgba(0, 0, 0, 0.45) 50%, rgba(0, 0, 0, 0.05) 100%), linear-gradient(to top, rgba(0, 0, 0, 0.6), transparent 50%); }
  .hero:not(.photo) .scrim { background: linear-gradient(90deg, rgba(0, 0, 0, 0.55) 0%, rgba(0, 0, 0, 0.25) 50%, transparent 100%); }
  .hero :global(.art) { background: #000; color: #fff; --fade: #000; --glow: 255, 255, 255; }
  .greet { font-size: 15px; font-weight: 500; color: rgba(255, 255, 255, 0.8); }
  .title { font-size: clamp(36px, 4.4vw, 60px); font-weight: 650; letter-spacing: -0.04em; line-height: 1.05; }
  .hero .facts { color: rgba(255, 255, 255, 0.85); }
  .hero .live { color: #fff; }
  .hero .dot { background: #fff; }
  .hero button { --text: #fff; --line-strong: rgba(255, 255, 255, 0.35); --faint: rgba(255, 255, 255, 0.6); }
  .hero button.primary { --text: #000; background: #fff; color: #000; border-color: #fff; }
  .hero button.primary:hover:not(:disabled) { background: #fff; border-color: #fff; }
  .glass { background: rgba(255, 255, 255, 0.1); color: #fff; backdrop-filter: blur(6px); }
  .glass:hover:not(:disabled) { background: rgba(255, 255, 255, 0.2); }
  .copy { display: grid; gap: 14px; max-width: 760px; min-width: 0; align-content: end; }
  .w { display: inline-block; overflow: hidden; vertical-align: top; padding: 0 0.04em 0.12em 0; margin-bottom: -0.12em; }
  .w > span { display: inline-block; transform: translateY(108%); animation: up 950ms var(--spring) forwards; animation-delay: calc(var(--i) * 90ms + 120ms); }
  @keyframes up { to { transform: none; } }
  .facts { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 8px 22px; color: var(--muted); font-size: 14px; }
  .facts li { display: inline-flex; align-items: center; gap: 8px; }
  .updlink { display: inline-flex; align-items: center; gap: 7px; padding: 3px 12px; font-size: 13px; color: #fff; background: rgba(255, 255, 255, 0.14); border-color: rgba(255, 255, 255, 0.35); }
  .live { display: inline-flex; align-items: center; gap: 8px; color: var(--text); }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--text); animation: pulse 1.4s infinite; }
  @keyframes pulse { 50% { opacity: 0.25; } }
  .lead { font-size: 15px; max-width: 460px; line-height: 1.5; color: rgba(255, 255, 255, 0.75); }
  .cta { display: flex; flex-wrap: wrap; gap: 12px; margin-top: 4px; }
  .cta button { display: inline-flex; align-items: center; gap: 9px; }
  .cta .big { padding: 12px 30px; }
  .cta .primary:not(:disabled)::after { animation: glint 5s ease-in-out 2s infinite; }
  .cta .primary:not(:disabled):hover::after { animation: none; }
  @keyframes glint { 0%, 70% { transform: translateX(-120%); } 100% { transform: translateX(120%); } }

  .duo.solo { grid-template-columns: minmax(0, 320px); }
  .duo { display: grid; grid-template-columns: minmax(0, 1fr) 320px; gap: 20px; }
  .block { display: flex; flex-direction: column; min-width: 0; }
  .block > .panel { flex: 1; }
  .shead { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 38px; margin-bottom: 14px; }
  .shead h2 { margin: 0; display: flex; align-items: center; gap: 10px; color: var(--text); font-size: 18px; font-weight: 600; letter-spacing: -0.03em; }
  .play { display: flex; flex-direction: column; }
  .play .bars { flex: 1; min-height: 110px; }
  .play .bar { grid-template-rows: minmax(0, 1fr) auto; }
  .play .col { height: 100%; }
  .more.link { padding: 4px 6px; font-size: 13px; color: var(--muted); border-color: transparent; }
  .more.link:hover:not(:disabled) { color: var(--text); background: transparent; border-color: transparent; }
  .tiles { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; padding: 10px; }
  .tile:first-child:nth-last-child(odd) { grid-column: 1 / -1; }
  .tile { position: relative; height: 128px; border-radius: 14px; overflow: hidden; cursor: pointer; isolation: isolate; outline: none; }
  .tcover { position: absolute; inset: 0; z-index: -2; display: grid; transition: transform 700ms var(--spring); }
  .tcover :global(img), .tcover :global(.art) { width: 100%; height: 100%; object-fit: cover; }
  .tile:hover .tcover { transform: scale(1.04); }
  .tshade { position: absolute; inset: 0; z-index: -1; background: linear-gradient(to top, rgba(0, 0, 0, 0.78), rgba(0, 0, 0, 0.1) 65%); }
  .tile::after { content: ''; position: absolute; inset: 0; border-radius: inherit; box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08); pointer-events: none; transition: box-shadow 200ms ease; }
  .tile:focus-visible::after { box-shadow: inset 0 0 0 2px #fff; }
  .tinfo { position: absolute; left: 14px; right: 60px; bottom: 12px; display: grid; gap: 2px; color: #fff; }
  .tname { font-weight: 650; font-size: 15px; letter-spacing: -0.02em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tmeta { font-size: 12px; color: rgba(255, 255, 255, 0.75); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tlive { position: absolute; top: 10px; left: 10px; display: inline-flex; align-items: center; gap: 6px; height: 24px; padding: 0 10px; border-radius: 999px; font-size: 11.5px; color: #fff; background: rgba(0, 0, 0, 0.55); }
  .tlive .dot { background: #3ecf6e; }
  .tplay { position: absolute; right: 12px; bottom: 12px; display: grid; place-items: center; width: 38px; height: 38px; padding: 0; border-radius: 50%; border: 0; background: #fff; color: #000; opacity: 0; transform: translateY(4px); transition: opacity 160ms ease, transform 260ms var(--spring); }
  .tplay:hover:not(:disabled) { background: #fff; border-color: transparent; }
  .tile:hover .tplay, .tile:focus-within .tplay { opacity: 1; transform: none; }
  .panel { border-radius: 16px; }
  .total { font-size: 34px; font-weight: 650; letter-spacing: -0.05em; }
  .bars { display: grid; grid-template-columns: repeat(7, 1fr); gap: 8px; margin-top: 14px; }
  .bar { position: relative; display: grid; gap: 6px; justify-items: center; cursor: default; }
  .bar:hover .fill { background: var(--inv-bg); }
  .bar:hover .lab { color: var(--text); }
  .tip { position: absolute; bottom: calc(100% + 8px); left: 50%; z-index: 5; display: grid; gap: 1px; white-space: nowrap; padding: 7px 11px; border-radius: 10px; background: var(--inv-bg); color: var(--inv-text); box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4); opacity: 0; visibility: hidden; transform: translate(-50%, 4px); pointer-events: none; transition: opacity 140ms ease, transform 180ms var(--spring), visibility 0s linear 180ms; }
  .bar.l .tip { left: 0; transform: translate(0, 4px); }
  .bar.r .tip { left: auto; right: 0; transform: translate(0, 4px); }
  .bar:hover .tip { opacity: 1; visibility: visible; transform: translate(-50%, 0); transition-delay: 0s; }
  .bar.l:hover .tip, .bar.r:hover .tip { transform: none; }
  .td { font-size: 11px; opacity: 0.7; text-transform: capitalize; }
  .tv { font-size: 13px; font-weight: 650; }
  .col { height: 84px; width: 100%; display: flex; align-items: flex-end; border-radius: 6px; overflow: hidden; }
  .fill { width: 100%; height: var(--h); background: var(--faint); border-radius: 5px; transform-origin: bottom; animation: grow 900ms var(--spring) both; animation-delay: calc(var(--i) * 60ms + 300ms); }
  @keyframes grow { from { transform: scaleY(0); } }
  .bar.today .fill { background: var(--inv-bg); }
  .lab { font-size: 11px; color: var(--muted); text-transform: capitalize; }
  .bar.today .lab { color: var(--text); }
  .foot { display: grid; gap: 8px; margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--line); font-size: 13px; }
  .tl { gap: 8px; }
  .phead { display: grid; gap: 2px; }
  .p2 .col { background: var(--surface-2); }
  .p2 .bars { margin-top: 18px; }
  .p2 .foot { margin-top: 16px; }
  .tnum { font-weight: 600; font-variant-numeric: tabular-nums; }
  .link { display: inline-flex; align-items: center; gap: 4px; padding: 0; font-size: 12px; color: var(--muted); background: transparent; border: 0; border-radius: 0; }
  .link:hover:not(:disabled) { color: var(--text); background: transparent; border-color: transparent; }


  @media (max-width: 1150px) { .duo { grid-template-columns: minmax(0, 1fr); } }
  @media (max-width: 900px) { .credit { display: none; } .hero { min-height: 260px; } }
</style>
