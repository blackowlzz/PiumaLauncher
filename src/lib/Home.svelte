<script lang="ts">
  import { thumb } from './img'
  import Art from './Art.svelte'
  import Cover from './Cover.svelte'
  import Count from './Count.svelte'
  import Icon from './Icon.svelte'
  import InstanceMenu from './InstanceMenu.svelte'
  import ProjectCard from './ProjectCard.svelte'
  import Strip from './Strip.svelte'
  import { reveal, spotlight } from './actions'
  import { locale, t } from './i18n.svelte'
  import { onMount } from 'svelte'
  import { activeAccount, app, installModpack, isBusy, openInstance, openProject, play, refreshEffective, selectedInstance, stop, weeklyPlaytime } from './store.svelte'
  import { formatDuration, loaderNames, profileName, relativeTime } from './meta'

  const inst = $derived(selectedInstance())
  const game = $derived(inst ? app.games[inst.id] : undefined)
  const running = $derived(game?.state === 'running')
  const busy = $derived(isBusy(inst?.id ?? null))
  const samples = $derived(inst ? (app.stats[inst.id] ?? []) : [])
  const eff = $derived(inst ? app.effective[inst.id] : undefined)
  const ram = $derived(running && samples.length ? samples[samples.length - 1] : null)
  $effect(() => {
    if (!inst) return
    void [inst.customPerformance, inst.memoryMaxMb, inst.flagProfile, app.settings.autoOptimize, app.settings.defaultMemoryMaxMb, app.settings.defaultFlagProfile, running]
    refreshEffective(inst.id)
  })
  const shownInstances = $derived(app.instances.slice(0, 8))
  const account = $derived(activeAccount())
  const week = $derived(weeklyPlaytime())
  const weekTotal = $derived(week.reduce((n, d) => n + d.secs, 0))
  const weekMax = $derived(Math.max(1, ...week.map((d) => d.secs)))
  const dayName = (d: Date) => d.toLocaleDateString(locale(), { weekday: 'short' }).replace('.', '')
  const isToday = (d: Date) => d.toDateString() === new Date().toDateString()
  const words = $derived((inst?.name ?? t('Your Minecraft, lightweight.')).split(/\s+/))
  const headlines = $derived(app.news.slice(0, 4))

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
    <p class="greet" use:reveal>{#if inst}{t('Your instance')}{:else}{t('Welcome')}{account ? `, ${account.name}` : ''}{/if}</p>
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
        {#if ram !== null}<li class="live"><span class="dot"></span>{t('RAM in use: {ram} MB', { ram })}</li>{/if}
      </ul>
      <div class="cta" use:reveal={4}>
        {#if running}
          <button class="big glass" onclick={() => stop(inst.id)}>{t('Stop')}</button>
        {:else}
          <button class="primary big" onclick={() => play(inst.id)} disabled={busy}>
            <span class="ic"><Icon name="play" size={15} /></span>{game?.state === 'preparing' ? t('Preparing…') : t('Play')}
          </button>
        {/if}
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

<div class="cols">
  <div class="main">
    {#if app.featured.length}
      <Strip icon="flame" title={t('Popular modpacks')} action={t('View all')} onAction={() => (app.page = 'explore')} cols="minmax(236px, 262px)">
        {#each app.featured as h, n (h.projectId)}
          <div use:reveal={n}>
            <ProjectCard hit={h} busy={app.installingId === h.projectId} disabled={app.installingId !== null} onInstall={() => installModpack(h.projectId, undefined, h.latestMc)} onOpen={() => openProject(h)} />
          </div>
        {/each}
      </Strip>
    {/if}

    {#if shownInstances.length}
      <section>
        <div class="shead">
          <h2><Icon name="box" size={20} />{t('Your instances')}</h2>
          <button class="ghost" onclick={() => (app.page = 'library')}>{t('View all')} →</button>
        </div>
        <div class="inst">
          {#each shownInstances as i, n (i.id)}
            {@const g = app.games[i.id]}
            <div class="mini" class:sel={i.id === inst?.id} use:spotlight use:reveal={n}>
              <button class="thumb" onclick={() => openInstance(i.id)} aria-label={t('Open {name}', { name: i.name })}><Cover inst={i} /></button>
              <div class="mt">
                <span class="mn">{i.name}</span>
                <span class="muted small">{i.mcVersion}{i.loader !== 'vanilla' ? ` · ${loaderNames[i.loader]}` : ''}</span>
                {#if g?.state === 'running'}
                  <button class="sm stopb" onclick={() => stop(i.id)}>{t('Stop')}</button>
                {:else}
                  <button class="sm playb" onclick={() => play(i.id)} disabled={isBusy(i.id)}><Icon name="play" size={12} />{g?.state === 'preparing' ? t('Preparing…') : t('Play')}</button>
                {/if}
              </div>
              <InstanceMenu inst={i} />
            </div>
          {/each}
        </div>
      </section>
    {/if}
  </div>

  <aside class="side">
    <section class="panel" use:reveal={1}>
      <div class="phead"><h2><Icon name="clock" size={18} />{t('Playtime')}</h2><span class="muted small">{t('Last 7 days')}</span></div>
      <div class="total"><Count value={weekTotal} format={(n) => formatDuration(n)} /></div>
      <div class="bars" role="img" aria-label={t('Playtime over the last 7 days')}>
        {#each week as d, i}
          <div class="bar" class:today={isToday(d.date)} class:l={i < 2} class:r={i > 4}>
            <div class="col"><div class="fill" style:--h="{d.secs ? Math.max(6, (d.secs / weekMax) * 100) : 0}%" style:--i={i}></div></div>
            <span class="lab">{dayName(d.date)}</span>
            <span class="tip" role="tooltip"><span class="td">{d.date.toLocaleDateString(locale(), { weekday: 'long', day: 'numeric', month: 'short' })}</span><span class="tv">{d.secs ? formatDuration(d.secs) : t('No playtime')}</span></span>
          </div>
        {/each}
      </div>
      {#if !weekTotal}<p class="muted small">{t('The chart fills in after your first game.')}</p>{/if}
    </section>

    {#if headlines.length}
      <section class="panel newsblock" use:reveal={2}>
        <div class="phead"><h2><Icon name="news" size={18} />{t('Minecraft news')}</h2></div>
        <ul class="news">
          {#each headlines as n}
            <li>
              {#if n.imageUrl}<img use:thumb={{ src: n.imageUrl, w: 200 }} alt="" width="64" height="64" decoding="async" />{:else}<span class="nph"></span>{/if}
              <div class="grow">
                <div class="nt">{n.title}</div>
                <div class="muted small">{relativeTime(Date.parse(n.date))} · {n.kind === 'release' ? 'Release' : 'Snapshot'}</div>
              </div>
              <span class="go"><Icon name="chevron" size={15} /></span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  </aside>
</div>

<style>
  .hero { position: relative; display: flex; align-items: flex-end; justify-content: space-between; gap: 24px; min-height: 270px; padding: 34px 38px; border-radius: 22px; overflow: hidden; border: 1px solid var(--line); flex: none; color: #fff; isolation: isolate; }
  .bg { position: absolute; inset: 0; z-index: -1; overflow: hidden; background: #000; }
  .slide { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; opacity: 0; transition: opacity 1600ms ease; }
  .slide.on { opacity: 1; }
  .credit { position: relative; align-self: flex-end; display: grid; gap: 2px; justify-items: end; text-align: right; }
  .ct { font-size: 15px; font-weight: 600; letter-spacing: -0.02em; }
  .cb { font-size: 12px; opacity: 0.7; margin-bottom: 10px; }
  .scrim { position: absolute; inset: 0; background: linear-gradient(90deg, rgba(0, 0, 0, 0.82) 0%, rgba(0, 0, 0, 0.45) 50%, rgba(0, 0, 0, 0.05) 100%), linear-gradient(to top, rgba(0, 0, 0, 0.6), transparent 50%); }
  .hero:not(.photo) .scrim { background: linear-gradient(90deg, rgba(0, 0, 0, 0.55) 0%, rgba(0, 0, 0, 0.25) 50%, transparent 100%); }
  .hero :global(.art) { background: #000; color: #fff; --fade: #000; --glow: 255, 255, 255; }
  .greet { font-size: 11px; font-weight: 600; letter-spacing: 0.08em; text-transform: uppercase; color: rgba(255, 255, 255, 0.75); }
  .title { font-size: clamp(30px, 3.4vw, 46px); font-weight: 650; letter-spacing: -0.04em; line-height: 1.05; }
  .hero .facts { color: rgba(255, 255, 255, 0.85); }
  .hero .live { color: #fff; }
  .hero .dot { background: #fff; }
  .hero button { --text: #fff; --line-strong: rgba(255, 255, 255, 0.35); --faint: rgba(255, 255, 255, 0.6); }
  .hero button.primary { background: var(--inv-bg); color: var(--inv-text); border-color: var(--inv-bg); }
  .glass { background: rgba(255, 255, 255, 0.1); color: #fff; backdrop-filter: blur(6px); }
  .glass:hover:not(:disabled) { background: rgba(255, 255, 255, 0.2); }
  .copy { display: grid; gap: 14px; max-width: 760px; min-width: 0; align-content: end; }
  .w { display: inline-block; overflow: hidden; vertical-align: top; padding: 0 0.04em 0.12em 0; margin-bottom: -0.12em; }
  .w > span { display: inline-block; transform: translateY(108%); animation: up 950ms var(--spring) forwards; animation-delay: calc(var(--i) * 90ms + 120ms); }
  @keyframes up { to { transform: none; } }
  .facts { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 8px 22px; color: var(--muted); font-size: 14px; }
  .facts li { display: inline-flex; align-items: center; gap: 8px; }
  .live { display: inline-flex; align-items: center; gap: 8px; color: var(--text); }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--text); animation: pulse 1.4s infinite; }
  @keyframes pulse { 50% { opacity: 0.25; } }
  .lead { font-size: 15px; max-width: 460px; line-height: 1.5; color: rgba(255, 255, 255, 0.75); }
  .cta { display: flex; flex-wrap: wrap; gap: 12px; margin-top: 4px; }
  .cta button { display: inline-flex; align-items: center; gap: 9px; }
  .cta .big { padding: 12px 30px; }
  .ic { display: inline-grid; }
  .cta .primary:not(:disabled)::after { animation: glint 5s ease-in-out 2s infinite; }
  .cta .primary:not(:disabled):hover::after { animation: none; }
  @keyframes glint { 0%, 70% { transform: translateX(-120%); } 100% { transform: translateX(120%); } }

  .cols { display: grid; grid-template-columns: minmax(0, 1fr) 320px; gap: 26px; align-items: start; }
  .main { display: grid; grid-template-columns: minmax(0, 1fr); gap: 28px; min-width: 0; }
  .side { display: grid; gap: 16px; }
  .shead { display: flex; align-items: center; justify-content: space-between; margin-bottom: 14px; }
  .shead h2 { margin: 0; display: flex; align-items: center; gap: 10px; color: var(--text); font-size: 18px; font-weight: 600; letter-spacing: -0.03em; }
  .phead { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: 12px; }
  .phead h2 { margin: 0; display: flex; align-items: center; gap: 10px; font-size: 15px; font-weight: 600; color: var(--text); }
  .panel { border-radius: 18px; }
  .total { font-size: 34px; font-weight: 650; letter-spacing: -0.05em; }
  .bars { display: grid; grid-template-columns: repeat(7, 1fr); gap: 8px; margin-top: 14px; }
  .bar { position: relative; display: grid; gap: 6px; justify-items: center; cursor: default; }
  .bar:hover .fill { background: var(--inv-bg); }
  .bar:hover .lab { color: var(--text); }
  .tip { position: absolute; bottom: calc(100% + 8px); left: 50%; z-index: 5; display: grid; gap: 1px; white-space: nowrap; padding: 7px 11px; border-radius: 10px; background: var(--inv-bg); color: var(--inv-text); box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4); opacity: 0; transform: translate(-50%, 4px); pointer-events: none; transition: opacity 140ms ease, transform 180ms var(--spring); }
  .bar.l .tip { left: 0; transform: translate(0, 4px); }
  .bar.r .tip { left: auto; right: 0; transform: translate(0, 4px); }
  .bar:hover .tip { opacity: 1; transform: translate(-50%, 0); }
  .bar.l:hover .tip, .bar.r:hover .tip { transform: none; }
  .td { font-size: 11px; opacity: 0.7; text-transform: capitalize; }
  .tv { font-size: 13px; font-weight: 650; }
  .col { height: 84px; width: 100%; display: flex; align-items: flex-end; border-radius: 6px; overflow: hidden; }
  .fill { width: 100%; height: var(--h); background: var(--faint); border-radius: 5px; transform-origin: bottom; animation: grow 900ms var(--spring) both; animation-delay: calc(var(--i) * 60ms + 300ms); }
  @keyframes grow { from { transform: scaleY(0); } }
  .bar.today .fill { background: var(--inv-bg); }
  .lab { font-size: 11px; color: var(--muted); text-transform: capitalize; }
  .bar.today .lab { color: var(--text); }
  .news { list-style: none; padding: 0; margin: 0; display: grid; }
  .news li { display: flex; gap: 14px; align-items: center; padding: 12px 0; border-top: 1px solid var(--line); }
  .news li:first-child { border-top: 0; padding-top: 2px; }
  .news li:last-child { padding-bottom: 0; }
  .news img, .nph { width: 64px; height: 64px; border-radius: 12px; object-fit: cover; flex: none; background: var(--surface-2); }
  .nt { font-size: 13.5px; font-weight: 600; line-height: 1.35; margin-bottom: 6px; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .go { color: var(--faint); display: inline-grid; }

  .inst { display: grid; grid-template-columns: repeat(auto-fill, minmax(236px, 1fr)); gap: 14px; }
  .mini { display: flex; align-items: flex-start; gap: 12px; padding: 12px; border-radius: 16px; background: var(--surface); border: 1px solid var(--line); transition: border-color var(--ease); }
  .mini:hover { border-color: var(--line-strong); }
  .mini.sel { border-color: var(--inv-bg); }
  .thumb { position: relative; width: 56px; height: 56px; padding: 0; border-radius: 12px; overflow: hidden; flex: none; display: grid; place-items: center; }
  .mt { display: grid; gap: 2px; min-width: 0; flex: 1; justify-items: start; }
  .mn { font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 100%; }
  .sm { display: inline-flex; align-items: center; gap: 6px; padding: 5px 16px; margin-top: 8px; font-size: 13px; }
  .playb { background: var(--surface-2); }

  @media (max-width: 1150px) { .cols { grid-template-columns: minmax(0, 1fr); } }
  @media (max-width: 900px) { .credit { display: none; } .hero { min-height: 240px; } }
</style>
