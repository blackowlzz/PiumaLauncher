<script lang="ts">
  import { thumb } from './img'
  import Art from './Art.svelte'
  import Cover from './Cover.svelte'
  import Count from './Count.svelte'
  import Icon from './Icon.svelte'
  import ProjectCard from './ProjectCard.svelte'
  import Strip from './Strip.svelte'
  import { reveal, spotlight } from './actions'
  import { locale, t } from './i18n.svelte'
  import { onMount } from 'svelte'
  import { activeAccount, app, installModpack, isBusy, openInstance, openProject, play, selectedInstance, stop, weeklyPlaytime } from './store.svelte'
  import { formatDuration, loaderNames, profileName, relativeTime } from './meta'

  const inst = $derived(selectedInstance())
  const game = $derived(inst ? app.games[inst.id] : undefined)
  const running = $derived(game?.state === 'running')
  const busy = $derived(isBusy(inst?.id ?? null))
  const samples = $derived(inst ? (app.stats[inst.id] ?? []) : [])
  const ram = $derived(running && samples.length ? samples[samples.length - 1] : null)
  const others = $derived(app.instances.filter((i) => i.id !== inst?.id))
  const account = $derived(activeAccount())
  const week = $derived(weeklyPlaytime())
  const weekTotal = $derived(week.reduce((n, d) => n + d.secs, 0))
  const weekMax = $derived(Math.max(1, ...week.map((d) => d.secs)))
  const hour = new Date().getHours()
  const greeting = $derived(hour < 6 ? t('Good night') : hour < 13 ? t('Good morning') : hour < 19 ? t('Good afternoon') : t('Good evening'))
  const dayName = (d: Date) => d.toLocaleDateString(locale(), { weekday: 'short' }).replace('.', '')
  const isToday = (d: Date) => d.toDateString() === new Date().toDateString()
  const words = $derived((inst?.name ?? t('Your Minecraft, lightweight.')).split(/\s+/))
  const lead = $derived(app.news.find((n) => n.imageUrl) ?? app.news[0])
  const rest = $derived(app.news.filter((n) => n !== lead).slice(0, 3))

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
    <p class="greet" use:reveal>{inst ? greeting : t('Welcome')}{account ? `, ${account.name}` : ''}</p>
    {#key inst?.id ?? 'none'}
      <h1 class="display" aria-label={words.join(' ')}>
        {#each words as w, i}<span class="w" aria-hidden="true"><span style:--i={i}>{w}</span></span>{' '}{/each}
      </h1>
    {/key}

    {#if inst}
      <ul class="facts" use:reveal={3}>
        <li>Minecraft {inst.mcVersion}{inst.loader !== 'vanilla' ? ` · ${loaderNames[inst.loader]}` : ''}</li>
        <li>{inst.memoryMaxMb} MB · {profileName(inst.flagProfile)}</li>
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
        <button class="big glass" onclick={() => openInstance(inst.id)}>{t('Details')}</button>
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
      <Strip title={t('Popular modpacks')} action={t('Explore all')} onAction={() => (app.page = 'explore')} cols="minmax(250px, 290px)">
        {#each app.featured as h, n (h.projectId)}
          <div use:reveal={n}>
            <ProjectCard hit={h} busy={app.installingId === h.projectId} disabled={app.installingId !== null} onInstall={() => installModpack(h.projectId, undefined, h.latestMc)} onOpen={() => openProject(h)} />
          </div>
        {/each}
      </Strip>
    {/if}

    {#if others.length}
      <Strip title={t('Your instances')} action={t('See all')} onAction={() => (app.page = 'library')} cols="minmax(210px, 240px)">
        {#each others as i, n (i.id)}
          <button class="mini" use:spotlight use:reveal={n} onclick={() => openInstance(i.id)}>
            <span class="thumb"><Cover inst={i} /></span>
            <span class="mt">
              <span class="mn">{i.name}</span>
              <span class="muted small">{i.mcVersion}{i.loader !== 'vanilla' ? ` · ${loaderNames[i.loader]}` : ''}</span>
            </span>
          </button>
        {/each}
      </Strip>
    {/if}
  </div>

  <aside class="side">
    <section class="panel" use:reveal={1}>
      <div class="phead"><h2>{t('Playtime')}</h2><span class="muted small">{t('Last 7 days')}</span></div>
      <div class="total"><Count value={weekTotal} format={(n) => formatDuration(n)} /></div>
      <div class="bars" role="img" aria-label={t('Playtime over the last 7 days')}>
        {#each week as d, i}
          <div class="bar" class:today={isToday(d.date)} title="{d.date.toLocaleDateString(locale())} · {formatDuration(d.secs)}">
            <div class="col"><div class="fill" style:--h="{d.secs ? Math.max(6, (d.secs / weekMax) * 100) : 0}%" style:--i={i}></div></div>
            <span class="lab">{dayName(d.date)}</span>
          </div>
        {/each}
      </div>
      {#if !weekTotal}<p class="muted small">{t('The chart fills in after your first game.')}</p>{/if}
    </section>

    {#if lead}
      <section class="newsblock" use:reveal={2}>
        <h2 class="nh">{t('Minecraft news')}</h2>
        <article class="feat">
          {#if lead.imageUrl}<img use:thumb={{ src: lead.imageUrl, w: 960 }} alt="" decoding="async" />{/if}
          <div class="veil"></div>
          <div class="ft">
            <div class="ftt">{lead.title}</div>
            <div class="fts">{relativeTime(Date.parse(lead.date))} · {lead.kind === 'release' ? 'Release' : 'Snapshot'}</div>
          </div>
        </article>
        <ul class="news">
          {#each rest as n}
            <li>
              {#if n.imageUrl}<img use:thumb={{ src: n.imageUrl, w: 160 }} alt="" width="56" height="56" decoding="async" />{:else}<span class="nph"></span>{/if}
              <div>
                <div class="nt">{n.title}</div>
                <div class="muted small">{relativeTime(Date.parse(n.date))} · {n.kind === 'release' ? 'Release' : 'Snapshot'}</div>
              </div>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  </aside>
</div>

<style>
  .hero { position: relative; display: flex; align-items: flex-end; justify-content: space-between; gap: 24px; min-height: 440px; padding: 44px; border-radius: 30px; overflow: hidden; border: 1px solid var(--line); flex: none; color: #fff; isolation: isolate; }
  .bg { position: absolute; inset: 0; z-index: -1; overflow: hidden; background: #000; }
  .slide { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; opacity: 0; transition: opacity 1600ms ease; }
  .slide.on { opacity: 1; }
  .credit { position: relative; align-self: flex-end; display: grid; gap: 2px; justify-items: end; text-align: right; }
  .ct { font-size: 15px; font-weight: 600; letter-spacing: -0.02em; }
  .cb { font-size: 12px; opacity: 0.7; margin-bottom: 10px; }
  .scrim { position: absolute; inset: 0; background: linear-gradient(90deg, rgba(0, 0, 0, 0.86) 0%, rgba(0, 0, 0, 0.55) 45%, rgba(0, 0, 0, 0.1) 100%), linear-gradient(to top, rgba(0, 0, 0, 0.7), transparent 45%); }
  .hero:not(.photo) .scrim { background: linear-gradient(90deg, rgba(0, 0, 0, 0.55) 0%, rgba(0, 0, 0, 0.25) 50%, transparent 100%); }
  .hero :global(.art) { background: #000; color: #fff; --fade: #000; --glow: 255, 255, 255; }
  .greet { font-size: 19px; font-weight: 500; letter-spacing: -0.02em; color: rgba(255, 255, 255, 0.82); }
  .hero .facts { color: rgba(255, 255, 255, 0.72); }
  .hero .live { color: #fff; }
  .hero .dot { background: #fff; }
  .hero button { --text: #fff; --line-strong: rgba(255, 255, 255, 0.35); --faint: rgba(255, 255, 255, 0.6); }
  .hero button.primary { background: #fff; color: #000; border-color: #fff; }
  .glass { background: rgba(255, 255, 255, 0.1); color: #fff; }
  .glass:hover:not(:disabled) { background: rgba(255, 255, 255, 0.2); }
  .copy { display: grid; gap: 22px; max-width: 760px; min-width: 0; align-content: end; }
  .w { display: inline-block; overflow: hidden; vertical-align: top; padding: 0 0.04em 0.12em 0; margin-bottom: -0.12em; }
  .w > span { display: inline-block; transform: translateY(108%); animation: up 950ms var(--spring) forwards; animation-delay: calc(var(--i) * 90ms + 120ms); }
  @keyframes up { to { transform: none; } }
  .facts { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; color: var(--muted); font-size: 15px; }
  .live { display: inline-flex; align-items: center; gap: 8px; color: var(--text); }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--text); animation: pulse 1.4s infinite; }
  @keyframes pulse { 50% { opacity: 0.25; } }
  .lead { font-size: 16px; max-width: 460px; line-height: 1.5; color: rgba(255, 255, 255, 0.75); }
  .cta { display: flex; flex-wrap: wrap; gap: 12px; }
  .cta button { display: inline-flex; align-items: center; gap: 9px; }
  .ic { display: inline-grid; }
  .cta .primary:not(:disabled)::after { animation: glint 5s ease-in-out 2s infinite; }
  .cta .primary:not(:disabled):hover::after { animation: none; }
  @keyframes glint { 0%, 70% { transform: translateX(-120%); } 100% { transform: translateX(120%); } }

  .cols { display: grid; grid-template-columns: minmax(0, 1fr) 310px; gap: 28px; align-items: start; }
  .main { display: grid; grid-template-columns: minmax(0, 1fr); gap: 34px; min-width: 0; }
  .side { display: grid; gap: 16px; }
  .phead { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; margin-bottom: 10px; }
  .phead h2 { margin: 0; font-size: 14px; font-weight: 600; color: var(--text); }
  .total { font-size: 34px; font-weight: 650; letter-spacing: -0.05em; }
  .bars { display: grid; grid-template-columns: repeat(7, 1fr); gap: 8px; margin-top: 14px; }
  .bar { display: grid; gap: 6px; justify-items: center; }
  .col { height: 76px; width: 100%; display: flex; align-items: flex-end; background: var(--surface-2); border-radius: 999px; overflow: hidden; }
  .fill { width: 100%; height: var(--h); background: var(--faint); border-radius: 999px; transform-origin: bottom; animation: grow 900ms var(--spring) both; animation-delay: calc(var(--i) * 60ms + 300ms); }
  @keyframes grow { from { transform: scaleY(0); } }
  .bar.today .fill { background: var(--text); }
  .lab { font-size: 11px; color: var(--muted); text-transform: capitalize; }
  .bar.today .lab { color: var(--text); }
  .newsblock { display: grid; gap: 14px; }
  .nh { margin: 0; font-size: 14px; font-weight: 600; color: var(--text); }
  .feat { position: relative; aspect-ratio: 16 / 10; border-radius: 22px; overflow: hidden; isolation: isolate; color: #fff; border: 1px solid var(--line); background: #0a0a0a; }
  .feat img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; z-index: -2; }
  .veil { position: absolute; inset: 0; z-index: -1; background: linear-gradient(to top, rgba(0, 0, 0, 0.92), rgba(0, 0, 0, 0.1) 65%); }
  .ft { position: absolute; left: 16px; right: 16px; bottom: 14px; display: grid; gap: 2px; }
  .ftt { font-size: 18px; font-weight: 650; letter-spacing: -0.03em; line-height: 1.15; }
  .fts { font-size: 12px; opacity: 0.7; }
  .news { list-style: none; padding: 0; margin: 0; display: grid; gap: 14px; }
  .news li { display: flex; gap: 12px; align-items: center; }
  .news img, .nph { width: 56px; height: 56px; border-radius: 14px; object-fit: cover; flex: none; background: var(--surface-2); }
  .nt { font-size: 13px; font-weight: 500; line-height: 1.35; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }

  .mini { display: flex; align-items: center; gap: 12px; text-align: left; padding: 10px; border-radius: 20px; background: var(--surface); border-color: var(--line); }
  .mini:hover:not(:disabled) { background: var(--surface); border-color: var(--line-strong); }
  .thumb { position: relative; width: 52px; height: 52px; border-radius: 14px; overflow: hidden; flex: none; display: grid; place-items: center; border: 1px solid var(--line); }
  .mt { display: grid; min-width: 0; }
  .mn { font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  @media (max-width: 1150px) { .cols { grid-template-columns: minmax(0, 1fr); } }
  @media (max-width: 900px) { .credit { display: none; } .hero { min-height: 300px; } }
</style>
