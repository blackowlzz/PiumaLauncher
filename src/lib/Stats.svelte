<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type CrashEntry } from './api'
  import Count from './Count.svelte'
  import Cover from './Cover.svelte'
  import LineChart from './LineChart.svelte'
  import Icon from './Icon.svelte'
  import Sparkline from './Sparkline.svelte'
  import { causes } from './crashinfo'
  import { locale, t } from './i18n.svelte'
  import { formatDuration, relativeTime } from './meta'
  import { byHour, byWeekday, perDay, perInstance, streaks, totals, weekCompare } from './statsmath'
  import { inview, reveal } from './actions'
  import { app, openInstance, refreshStartup, seconds } from './store.svelte'

  let crashes: CrashEntry[] = $state([])
  let now = $state(Date.now())

  onMount(() => {
    api.crashHistory().then((c) => (crashes = c.slice().sort((a, b) => b.atMs - a.atMs))).catch(() => {})
    for (const i of app.instances) refreshStartup(i.id)
    const id = setInterval(() => (now = Date.now()), 60_000)
    return () => clearInterval(id)
  })

  const sum = $derived(totals(app.sessions, now))
  const streak = $derived(streaks(app.sessions, now))
  const days = $derived(perDay(app.sessions, 30, now))
  const peak = $derived(Math.max(1, ...days.map((d) => d.secs)))
  const crashCount = $derived(crashes.reduce((m, c) => m.set(c.instanceId, (m.get(c.instanceId) ?? 0) + 1), new Map<string, number>()))
  const rows = $derived.by(() => {
    const per = perInstance(app.sessions)
    return app.instances
      .map((inst) => ({ inst, ...(per.get(inst.id) ?? { secs: 0, count: 0, last: 0 }), crashes: crashCount.get(inst.id) ?? 0, startup: app.startup[inst.id] }))
      .filter((r) => r.count || r.crashes || r.startup?.lastMs)
      .sort((a, b) => b.secs - a.secs)
  })
  const top = $derived(Math.max(1, ...rows.map((r) => r.secs)))
  const names = $derived(new Map(app.instances.map((i) => [i.id, i.name])))

  const dayLabel = (ms: number, opts: Intl.DateTimeFormatOptions) => new Date(ms).toLocaleDateString(locale(), opts)
  const days_ = (n: number) => (n === 1 ? t('1 day') : t('{n} days', { n }))
  const tiles = $derived([
    { label: t('Last 7 days'), value: sum.week, format: formatDuration },
    { label: t('Last 30 days'), value: sum.month, format: formatDuration },
    { label: t('Sessions'), value: sum.count, format: (n: number) => String(Math.round(n)) },
    { label: t('Average session'), value: sum.average, format: formatDuration },
    { label: t('Day streak'), value: streak.current, format: (n: number) => days_(Math.round(n)), note: streak.longest > streak.current ? t('best {n}', { n: streak.longest }) : '' },
  ] as { label: string; value: number; format: (n: number) => string; note?: string }[])

  const week = $derived(weekCompare(app.sessions, now))
  const favourite = $derived(rows.find((r) => r.secs > 0) ?? null)
  const share = $derived(favourite && sum.all ? favourite.secs / sum.all : 0)
  const weekdays = $derived(byWeekday(app.sessions))
  const hours = $derived(byHour(app.sessions))
  const weekPeak = $derived(Math.max(1, ...weekdays))
  const hourPeak = $derived(Math.max(1, ...hours))
  const weekdayName = (i: number) => new Date(2024, 0, 1 + i).toLocaleDateString(locale(), { weekday: 'short' }).replace('.', '')
  const favouriteDay = $derived(weekdays.some((v) => v > 0) ? new Date(2024, 0, 1 + weekdays.indexOf(weekPeak)).toLocaleDateString(locale(), { weekday: 'long' }) : '')
  const favouriteHour = $derived(hours.some((v) => v > 0) ? hours.indexOf(hourPeak) : null)

  const palette = ['var(--text)', '#9b7bff', '#3ecf6e', '#e5a03d']
  const startupSeries = $derived(
    rows
      .filter((r) => (r.startup?.history.length ?? 0) > 1)
      .slice(0, 4)
      .map((r, i) => ({ label: r.inst.name, color: palette[i], values: r.startup!.history.map((ms) => ms / 1000) })),
  )
  const startupLabels = $derived(Array.from({ length: Math.max(0, ...startupSeries.map((s) => s.values.length)) }, (_, i) => t('Launch {n}', { n: i + 1 })))
</script>

<header class="head">
  <h1>{t('Statistics')}</h1>
  <p class="muted">{t('How much you play, how fast your instances start and what went wrong.')}</p>
</header>

{#if !app.sessions.length && !crashes.length}
  <div class="panel empty">
    <Icon name="chart" size={28} />
    <div>
      <div class="et">{t('Nothing to show yet')}</div>
      <p class="muted">{t('Play a game and your playtime, sessions and startup times start appearing here.')}</p>
    </div>
  </div>
{:else}
  <section class="hero">
    <div class="hcopy">
      <div class="heyebrow">{t('Total playtime')}</div>
      <div class="hbig"><Count value={sum.all} format={formatDuration} /></div>
      <div class="hchips">
        <span class="hchip"><Icon name="play" size={12} />{t('{n} sessions', { n: sum.count })}</span>
        {#if streak.current}<span class="hchip"><Icon name="flame" size={13} />{t('Day streak')}: {days_(streak.current)}</span>{/if}
        {#if week.change !== null}
          <span class="hchip trend" class:up={week.change > 0} class:down={week.change < 0}>{week.change > 0 ? '↑' : week.change < 0 ? '↓' : '='} {Math.abs(week.change)}% {t('vs the previous week')}</span>
        {/if}
      </div>
    </div>
    <div class="hline">
      <LineChart bare area height={86} label={t('Playtime over the last 30 days')} series={[{ label: t('Playtime'), color: 'var(--text)', values: days.map((d) => d.secs) }]} labels={days.map((d) => dayLabel(d.day, { weekday: 'long', day: 'numeric', month: 'long' }))} format={(v) => (v >= 60 ? formatDuration(v) : t('No playtime'))} />
    </div>
  </section>

  <section class="tiles">
    {#each tiles as tile, i}
      <div class="tile" style:--i={i}>
        <div class="eyebrow">{tile.label}</div>
        <div class="tv"><Count value={tile.value} format={tile.format} />{#if tile.note}<span class="tn muted small">{tile.note}</span>{/if}</div>
      </div>
    {/each}
  </section>

  <section class="panel" use:reveal>
    <div class="phead"><h2><Icon name="chart" size={18} />{t('Last 30 days')}</h2><span class="muted small">{formatDuration(sum.month)}</span></div>
    <LineChart
      area
      height={190}
      label={t('Playtime over the last 30 days')}
      series={[{ label: t('Playtime'), color: 'var(--text)', values: days.map((d) => d.secs) }]}
      labels={days.map((d) => dayLabel(d.day, { weekday: 'long', day: 'numeric', month: 'long' }))}
      format={(v) => (v >= 60 ? formatDuration(v) : t('No playtime'))}
    />
    <div class="axis muted small">
      <span>{dayLabel(days[0].day, { day: 'numeric', month: 'short' })}</span>
      <span>{dayLabel(days[15].day, { day: 'numeric', month: 'short' })}</span>
      <span>{t('Today')}</span>
    </div>
  </section>

  <div class="trio">
    {#if favourite}
      <section class="panel favcard" use:reveal={1} use:inview>
        <div class="phead"><h2><Icon name="flame" size={18} />{t('Most played')}</h2></div>
        <div class="ringwrap">
          <svg viewBox="0 0 120 120" class="ring" aria-hidden="true">
            <circle class="track" cx="60" cy="60" r="50" pathLength="100" />
            <circle class="prog" cx="60" cy="60" r="50" pathLength="100" style:--p={Math.round(share * 100)} />
          </svg>
          <div class="rc">
            <span class="pct"><Count value={share * 100} format={(n) => `${Math.round(n)}%`} /></span>
            <span class="muted small">{t('of your playtime')}</span>
          </div>
        </div>
        <button class="favname" onclick={() => openInstance(favourite.inst.id)}>
          <span class="fthumb"><Cover inst={favourite.inst} /></span>
          <span class="fn"><span class="nm">{favourite.inst.name}</span><span class="muted small">{formatDuration(favourite.secs)} · {t('{n} sessions', { n: favourite.count })}</span></span>
        </button>
      </section>
    {/if}
    <section class="panel" use:reveal={2}>
      <div class="phead"><h2><Icon name="clock" size={18} />{t('Days of the week')}</h2>{#if favouriteDay}<span class="muted small">{t('You play most on {day}', { day: favouriteDay })}</span>{/if}</div>
      <div class="bars week" use:inview>
        {#each weekdays as v, i}
          <div class="bcol" title={`${weekdayName(i)} · ${formatDuration(v)}`}>
            <div class="btrack"><div class="bfill" class:top={v === weekPeak && v > 0} style:--h="{v ? Math.max(4, (v / weekPeak) * 100) : 0}%" style:--i={i}></div></div>
            <span class="blab">{weekdayName(i)}</span>
          </div>
        {/each}
      </div>
    </section>
    <section class="panel" use:reveal={3}>
      <div class="phead"><h2><Icon name="sun" size={18} />{t('Time of day')}</h2>{#if favouriteHour !== null}<span class="muted small">{t('Most often around {hour}:00', { hour: favouriteHour })}</span>{/if}</div>
      <div class="bars hours" use:inview>
        {#each hours as v, h}
          <div class="bcol" title={`${h}:00 · ${formatDuration(v)}`}>
            <div class="btrack"><div class="bfill" class:top={v === hourPeak && v > 0} style:--h="{v ? Math.max(4, (v / hourPeak) * 100) : 0}%" style:--i={h}></div></div>
            <span class="blab">{h % 6 === 0 ? h : ''}</span>
          </div>
        {/each}
      </div>
    </section>
  </div>

  {#if startupSeries.length}
    <section class="panel" use:reveal>
      <div class="phead"><h2><Icon name="cpu" size={18} />{t('Startup times')}</h2><span class="muted small">{t('Time from Play to the main menu, launch after launch')}</span></div>
      <LineChart series={startupSeries} labels={startupLabels} height={170} label={t('Startup times')} format={(v) => `${v.toFixed(1)} s`} />
      <div class="legend">
        {#each startupSeries as s}<span><span class="sw" style:background={s.color}></span>{s.label}</span>{/each}
      </div>
    </section>
  {/if}

  {#if rows.length}
    <section class="panel" use:reveal>
      <div class="phead"><h2><Icon name="grid" size={18} />{t('By instance')}</h2></div>
      <div class="table">
        <div class="tr th muted small">
          <span>{t('Instance')}</span>
          <span>{t('Playtime')}</span>
          <span class="c">{t('Sessions')}</span>
          <span>{t('Startup')}</span>
          <span class="c">{t('Crashes')}</span>
        </div>
        {#each rows as r (r.inst.id)}
          <button class="tr row" onclick={() => openInstance(r.inst.id)}>
            <span class="who">
              <span class="thumb"><Cover inst={r.inst} /></span>
              <span class="wn">
                <span class="nm">{r.inst.name}</span>
                <span class="muted small">{r.last ? relativeTime(r.last) : t('Never played')}</span>
              </span>
            </span>
            <span class="time">
              <span>{formatDuration(r.secs)}</span>
              <span class="meter"><span style:--w="{(r.secs / top) * 100}%"></span></span>
            </span>
            <span class="c">{r.count}</span>
            <span class="start">
              {#if r.startup?.usualMs ?? r.startup?.lastMs}
                <span class:warn={!!r.startup?.slower}>{seconds((r.startup?.usualMs ?? r.startup?.lastMs)!)}</span>
                {#if (r.startup?.history.length ?? 0) > 1}<span class="spark"><Sparkline values={r.startup!.history.map((ms) => ms / 1000)} limit={0} height={26} /></span>{/if}
              {:else}<span class="muted">—</span>{/if}
            </span>
            <span class="c" class:warn={r.crashes > 0}>{r.crashes}</span>
          </button>
        {/each}
      </div>
    </section>
  {/if}

  <section class="panel" use:reveal>
    <div class="phead"><h2><Icon name="flame" size={18} />{t('Recent crashes')}</h2><span class="muted small">{crashes.length}</span></div>
    {#if crashes.length}
      <ul class="crashes">
        {#each crashes.slice(0, 12) as c}
          <li>
            <span class="cdot" aria-hidden="true"></span>
            <span class="grow">
              <span class="ct">{t(causes[c.cause]?.[0] ?? 'Unknown cause')}{c.subject ? ` · ${c.subject}` : ''}</span>
              <span class="muted small">{names.get(c.instanceId) ?? c.instanceId} · {relativeTime(c.atMs)}</span>
            </span>
            {#if names.has(c.instanceId)}<button class="ghost sm" onclick={() => openInstance(c.instanceId)}>{t('Open')}</button>{/if}
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted small">{t('No crashes recorded. Great!')}</p>
    {/if}
  </section>
{/if}

<style>
  .head p { margin-top: 6px; }
  .empty { display: flex; align-items: center; gap: 18px; padding: 26px; }
  .et { font-weight: 600; font-size: 16px; margin-bottom: 4px; }
  .hero { position: relative; display: grid; align-content: space-between; gap: 18px; min-height: 250px; padding: 28px 32px 0; border-radius: 24px; overflow: hidden; border: 1px solid var(--line); background: var(--surface); animation: heroin 700ms var(--spring) backwards; }
  @keyframes heroin { from { opacity: 0; transform: translateY(14px) scale(0.99); } }
  .heyebrow { font-size: 13px; color: var(--muted); animation: swapin 500ms var(--spring) 150ms backwards; }
  .hbig { font-size: clamp(44px, 6vw, 72px); font-weight: 700; letter-spacing: -0.055em; line-height: 1; margin-top: 6px; animation: swapin 600ms var(--spring) 250ms backwards; }
  .hchips { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 16px; }
  .hchip { display: inline-flex; align-items: center; gap: 7px; height: 30px; padding: 0 13px; border-radius: 999px; font-size: 13px; background: var(--surface-2); border: 1px solid var(--line-strong); animation: swapin 500ms var(--spring) backwards; }
  .hchip:nth-child(1) { animation-delay: 380ms; }
  .hchip:nth-child(2) { animation-delay: 460ms; }
  .hchip:nth-child(3) { animation-delay: 540ms; }
  .trend.up { color: #2fbf63; border-color: rgba(62, 207, 110, 0.5); background: rgba(62, 207, 110, 0.12); }
  .trend.down { color: #e5484d; border-color: rgba(229, 72, 77, 0.5); background: rgba(229, 72, 77, 0.12); }
  .hline { margin: 0 -32px; }
  .favcard { display: grid; gap: 14px; align-content: start; }
  .ringwrap { position: relative; width: 170px; height: 170px; justify-self: center; }
  .ring { width: 100%; height: 100%; transform: rotate(-90deg); }
  .ring circle { fill: none; stroke-width: 10; }
  .track { stroke: var(--line); }
  .prog { stroke: var(--inv-bg); stroke-linecap: round; stroke-dasharray: 100; stroke-dashoffset: 100; transition: stroke-dashoffset 1600ms cubic-bezier(0.3, 0.7, 0.2, 1) 200ms; }
  .favcard:global(.seen) .prog { stroke-dashoffset: calc(100 - var(--p)); }
  .rc { position: absolute; inset: 22px; display: grid; place-content: center; justify-items: center; text-align: center; gap: 2px; }
  .rc .small { max-width: 92px; font-size: 11.5px; line-height: 1.3; }
  .pct { font-size: 30px; font-weight: 700; letter-spacing: -0.04em; }
  .favname { display: flex; align-items: center; gap: 12px; text-align: left; padding: 8px; border-radius: 14px; border-color: var(--line); }
  .fthumb { position: relative; width: 42px; height: 42px; flex: none; border-radius: 10px; overflow: hidden; display: grid; place-items: center; }
  .fn { display: grid; min-width: 0; }
  .tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 12px; }
  .tile { padding: 16px; border-radius: 16px; border: 1px solid var(--line); background: var(--surface); animation: tilein 520ms var(--spring) calc(var(--i) * 60ms) backwards; }
  @keyframes tilein { from { opacity: 0; transform: translateY(10px) scale(0.98); } }
  .tv { margin-top: 6px; font-size: 24px; font-weight: 650; letter-spacing: -0.04em; }
  .tn { margin-left: 8px; font-size: 12px; font-weight: 500; letter-spacing: 0; }
  .phead { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: 14px; flex-wrap: wrap; }
  .phead h2 { margin: 0; display: flex; align-items: center; gap: 10px; font-size: 15px; font-weight: 600; color: var(--text); }
  .axis { display: flex; justify-content: space-between; margin-top: 8px; }
  .trio { display: grid; grid-template-columns: minmax(0, 0.9fr) minmax(0, 1fr) minmax(0, 1.3fr); gap: 16px; }
  .bars { display: grid; gap: 6px; height: 130px; align-items: end; }
  .bars.week { grid-template-columns: repeat(7, minmax(0, 1fr)); }
  .bars.hours { grid-template-columns: repeat(24, minmax(0, 1fr)); gap: 3px; }
  .bcol { display: grid; grid-template-rows: 1fr auto; gap: 6px; height: 100%; cursor: default; }
  .btrack { display: flex; align-items: flex-end; height: 100%; }
  .bfill { width: 100%; height: var(--h); min-height: 2px; border-radius: 5px; background: var(--faint); transform-origin: bottom; transform: scaleY(0); transition: transform 900ms var(--spring) calc(var(--i) * 35ms + 150ms), background var(--ease); }
  .bars:global(.seen) .bfill { transform: none; }
  .bfill.top { background: var(--inv-bg); }
  .bcol:hover .bfill { background: var(--text); }
  .blab { font-size: 11px; color: var(--muted); text-align: center; text-transform: capitalize; min-height: 14px; }
  .legend { display: flex; flex-wrap: wrap; gap: 8px 18px; margin-top: 12px; font-size: 13px; }
  .legend > span { display: inline-flex; align-items: center; gap: 7px; }
  .sw { width: 10px; height: 10px; border-radius: 50%; flex: none; }
  .table { display: grid; }
  .tr { display: grid; grid-template-columns: minmax(0, 2.2fr) minmax(0, 1.6fr) 80px minmax(0, 1.3fr) 70px; gap: 16px; align-items: center; padding: 10px 8px; }
  .th { padding-top: 0; }
  .row { width: 100%; text-align: left; border: 0; border-top: 1px solid var(--line); border-radius: 0; background: transparent; }
  .row:hover:not(:disabled) { background: var(--surface-2); border-color: var(--line); }
  .c { text-align: center; }
  .who { display: flex; align-items: center; gap: 12px; min-width: 0; }
  .thumb { position: relative; width: 40px; height: 40px; flex: none; border-radius: 10px; overflow: hidden; display: grid; place-items: center; }
  .wn { display: grid; min-width: 0; }
  .nm { font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .time { display: grid; gap: 6px; }
  .meter { height: 4px; border-radius: 4px; background: var(--line); overflow: hidden; }
  .meter span { display: block; width: var(--w); height: 100%; background: var(--text); border-radius: 4px; }
  .start { display: flex; align-items: center; gap: 10px; min-width: 0; }
  .spark { flex: 1; min-width: 40px; max-width: 120px; opacity: 0.8; }
  .warn { color: #e5a03d; }
  .crashes { list-style: none; margin: 0; padding: 0; display: grid; }
  .crashes li { display: flex; align-items: center; gap: 12px; padding: 10px 0; border-top: 1px solid var(--line); }
  .crashes li:first-child { border-top: 0; padding-top: 0; }
  .crashes .grow { display: grid; gap: 2px; min-width: 0; }
  .ct { font-weight: 500; }
  .cdot { width: 8px; height: 8px; flex: none; border-radius: 50%; background: #e5484d; }
  .sm { padding: 5px 12px; font-size: 13px; }
  @media (max-width: 1150px) { .trio { grid-template-columns: 1fr 1fr; } .favcard { grid-column: 1 / -1; } }
  @media (max-width: 760px) { .trio { grid-template-columns: 1fr; } }
  @media (max-width: 900px) { .tr { grid-template-columns: minmax(0, 2fr) minmax(0, 1.4fr) 60px minmax(0, 1fr) 50px; gap: 10px; } }
</style>
