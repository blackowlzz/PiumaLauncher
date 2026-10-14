<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type Gc, type Recommendation, type SystemInfo } from './api'
  import { t } from './i18n.svelte'
  import Select from './Select.svelte'
  import PixelField from './PixelField.svelte'
  import Slider from './Slider.svelte'
  import { profiles } from './meta'
  import { app, saveSettings } from './store.svelte'

  let sys = $state<SystemInfo | null>(null)
  let rec = $state<Recommendation | null>(null)
  onMount(() => {
    api.systemInfo().then((s) => (sys = s)).catch(() => {})
    api.recommendPerformance().then((r) => (rec = r)).catch(() => {})
  })

  const s = app.settings
  const totalMb = $derived(sys?.totalMemoryMb ?? 16384)
  const ceiling = $derived(Math.max(1024, Math.floor(totalMb / 256) * 256))
  const cores = $derived(sys?.cpuCores ?? 8)
  const tooMuch = $derived(sys !== null && s.defaultMemoryMaxMb > totalMb * 0.75)
  const gb = (mb: number) => `${(mb / 1024).toFixed(mb % 1024 ? 1 : 0)} GB`
  const presets = $derived([2048, 4096, 6144, 8192, 12288, 16384].filter((m) => m <= ceiling))

  const tierName = $derived(
    rec
      ? {
          weak: t('Light hardware'),
          modest: t('Modest hardware'),
          capable: t('Capable hardware'),
          strong: t('Powerful hardware'),
        }[rec.tier]
      : '',
  )

  const level = $derived(rec ? { weak: 0.3, modest: 0.55, capable: 0.8, strong: 1 }[rec.tier] : 0.5)

  const isSaved = $derived(
    rec !== null &&
      s.defaultMemoryMaxMb === rec.memoryMaxMb &&
      s.defaultMemoryMinMb === rec.memoryMinMb &&
      s.defaultFlagProfile === rec.flagProfile &&
      s.tuning.gcThreads === rec.tuning.gcThreads &&
      s.tuning.disableExplicitGc === rec.tuning.disableExplicitGc &&
      s.tuning.stringDedup === rec.tuning.stringDedup,
  )

  function applyRecommendation() {
    if (!rec) return
    s.defaultMemoryMaxMb = rec.memoryMaxMb
    s.defaultMemoryMinMb = rec.memoryMinMb
    s.defaultFlagProfile = rec.flagProfile
    s.tuning = { ...s.tuning, gcThreads: rec.tuning.gcThreads, disableExplicitGc: rec.tuning.disableExplicitGc, stringDedup: rec.tuning.stringDedup, preTouch: rec.tuning.preTouch }
    saveSettings()
  }

  function setMax(mb: number) {
    s.defaultMemoryMaxMb = mb
    if (s.defaultMemoryMinMb > mb) s.defaultMemoryMinMb = mb
    saveSettings()
  }

  function setMin(mb: number) {
    s.defaultMemoryMinMb = Math.min(mb, s.defaultMemoryMaxMb)
    saveSettings()
  }

  const collectors = $derived<{ value: Gc; label: string }[]>([
    { value: 'auto', label: t('Follow the JVM profile') },
    { value: 'g1', label: 'G1' },
    { value: 'zgc', label: 'ZGC' },
    { value: 'parallel', label: 'Parallel' },
    { value: 'serial', label: 'Serial' },
  ])

  const collectorHint = $derived(
    {
      auto: t('Each instance uses the collector of its JVM profile.'),
      g1: t('Balanced between smoothness and throughput. A good choice for almost everyone.'),
      zgc: t('Very short pauses, uses more RAM. Needs Java 17 or newer.'),
      parallel: t('Maximum throughput, but longer pauses. Good for short sessions.'),
      serial: t('Single thread and the least memory. Only for very weak computers.'),
    }[s.tuning.gc],
  )

  const pauseCapable = $derived(s.tuning.gc === 'g1' || s.tuning.gc === 'parallel' || s.tuning.gc === 'auto')
</script>

<header class="head">
  <h1 class="shine">{t('Performance')}</h1>
  <p class="muted">{t('Memory, Java and launcher tuning in one place.')}</p>
</header>

<div class="groups">
  <section class="group auto">
    <h2>{t('Automatic optimizer')}</h2>
    <p class="muted small intro">{t('Picks memory and Java settings that fit your computer, so you do not have to.')}</p>

    {#if rec}
      <div class="card" data-tier={rec.tier}>
        <PixelField level={level} />
        <div class="body">
          <div class="eyebrow">{t('Detected')}</div>
          <div class="tier">{tierName}</div>
          <div class="facts">
            <div><span class="muted small">{t('Memory')}</span><b>{gb(rec.memoryMaxMb)}</b></div>
            <div><span class="muted small">{t('Java profile')}</span><b>{profiles().find((p) => p.id === rec?.flagProfile)?.label}</b></div>
            <div><span class="muted small">{t('Threads')}</span><b>{cores}</b></div>
          </div>
          {#each rec.reasons as r}<p class="why">{t(r)}</p>{/each}
          <div class="actions">
            <button class="primary" disabled={isSaved} onclick={applyRecommendation}>{isSaved ? t('Saved') : t('Save')}</button>
            <span class="muted small">{t('Saves these values as the defaults for every instance.')}</span>
          </div>
        </div>
      </div>
    {/if}

    <div class="item">
      <div class="txt">
        <div class="n">{t('Optimize every launch automatically')}</div>
        <div class="d">{t('Before each launch it checks your free memory and the mods in the instance, then sets memory and Java flags. You can still choose the garbage collector yourself.')}</div>
        <div class="d">{t('It is a safe starting point, not always better than settings you tune yourself. If you know what your computer needs, manual values can do better.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={s.autoOptimize} onchange={saveSettings} aria-label={t('Optimize every launch automatically')} />
        <span class="track"></span>
      </label>
    </div>
    {#if s.autoOptimize}<p class="warn small">{t('The optimizer is on: at each launch it can raise memory and picks the Java flags. The values below are the minimum it starts from.')}</p>{/if}
  </section>
  <section class="group">
    <h2>{t('Default memory and profile')}</h2>
    <p class="muted small intro">
      {#if sys}{t('Your computer has {ram} of RAM and {cores} threads.', { ram: gb(totalMb), cores })}{/if}
      {t('Used by every instance unless it has its own custom values.')}
    </p>

    <div class="item col">
      <div class="row2">
        <div class="n">{t('Max RAM (MB)')}</div>
        <div class="val">
          <input class="num" type="number" min="512" max={ceiling} step="256" value={s.defaultMemoryMaxMb} onchange={(e) => setMax(Number(e.currentTarget.value))} aria-label={t('Max RAM (MB)')} />
          <span class="muted small">{gb(s.defaultMemoryMaxMb)}</span>
        </div>
      </div>
      <Slider min={512} max={ceiling} step={256} value={s.defaultMemoryMaxMb} label={t('Max RAM (MB)')} oninput={(v) => (s.defaultMemoryMaxMb = v)} onchange={setMax} />
      <div class="presets">
        {#each presets as m}
          <button class="ghost" class:on={s.defaultMemoryMaxMb === m} onclick={() => setMax(m)}>{gb(m)}</button>
        {/each}
      </div>
      {#if tooMuch}<p class="warn small">{t('That is most of your RAM. Leave some for the system, or the game may slow down.')}</p>{/if}
    </div>

    <div class="item col">
      <div class="row2">
        <div class="txt">
          <div class="n">{t('Initial RAM (MB)')}</div>
          <div class="d">{t('Memory reserved at startup. Setting it equal to the maximum avoids resizing pauses.')}</div>
        </div>
        <div class="val">
          <input class="num" type="number" min="128" max={s.defaultMemoryMaxMb} step="128" value={s.defaultMemoryMinMb} onchange={(e) => setMin(Number(e.currentTarget.value))} aria-label={t('Initial RAM (MB)')} />
          <span class="muted small">{gb(s.defaultMemoryMinMb)}</span>
        </div>
      </div>
      <Slider min={128} max={s.defaultMemoryMaxMb} step={128} value={s.defaultMemoryMinMb} label={t('Initial RAM (MB)')} oninput={(v) => (s.defaultMemoryMinMb = v)} onchange={setMin} />
      <div class="presets">
        <button class="ghost" onclick={() => setMin(s.defaultMemoryMaxMb)}>{t('Same as maximum')}</button>
        <button class="ghost" onclick={() => setMin(Math.round(s.defaultMemoryMaxMb / 2 / 128) * 128)}>{t('Half of maximum')}</button>
      </div>
    </div>
  </section>

  <section class="group">
    <h2>{t('JVM')}</h2>
    <p class="muted small intro">{t('Applies to every launch. Each instance can still pick its own profile and arguments in its settings.')}</p>

    <div class="item">
      <div class="txt">
        <div class="n">{t('JVM profile')}</div>
        <div class="d">{profiles().find((p) => p.id === s.defaultFlagProfile)?.hint} {t('Used by new instances.')}</div>
      </div>
      <Select bind:value={s.defaultFlagProfile} width="190px" label={t('JVM profile')} onchange={saveSettings} options={profiles().map((p) => ({ value: p.id, label: p.label }))} />
    </div>

    <div class="item">
      <div class="txt">
        <div class="n">{t('Garbage collector')}</div>
        <div class="d">{collectorHint}</div>
      </div>
      <Select bind:value={s.tuning.gc} width="190px" label={t('Garbage collector')} onchange={saveSettings} options={collectors} />
    </div>

    {#if pauseCapable}
      <div class="item">
        <div class="txt">
          <div class="n">{t('Pause target (ms)')}</div>
          <div class="d">{t('Maximum pause the collector aims for. 0 keeps the default.')}</div>
        </div>
        <input class="num" type="number" min="0" max="2000" step="10" bind:value={s.tuning.pauseMs} onchange={saveSettings} aria-label={t('Pause target (ms)')} />
      </div>
    {/if}

    <div class="item">
      <div class="txt">
        <div class="n">{t('Collector threads')}</div>
        <div class="d">{t('0 lets Java decide. Using about half of your threads is usually enough.')}</div>
      </div>
      <input class="num" type="number" min="0" max={Math.min(64, cores)} step="1" bind:value={s.tuning.gcThreads} onchange={saveSettings} aria-label={t('Collector threads')} />
    </div>

    <div class="item">
      <div class="txt">
        <div class="n">{t('Pre-allocate memory')}</div>
        <div class="d">{t('Reserves the whole heap at startup. Smoother gameplay, but a slower start and more RAM used from the beginning.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={s.tuning.preTouch} onchange={saveSettings} aria-label={t('Pre-allocate memory')} />
        <span class="track"></span>
      </label>
    </div>

    <div class="item">
      <div class="txt">
        <div class="n">{t('Ignore forced cleanups')}</div>
        <div class="d">{t('Stops mods from forcing a full garbage collection, which causes freezes.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={s.tuning.disableExplicitGc} onchange={saveSettings} aria-label={t('Ignore forced cleanups')} />
        <span class="track"></span>
      </label>
    </div>

    <div class="item">
      <div class="txt">
        <div class="n">{t('Deduplicate strings')}</div>
        <div class="d">{t('Saves RAM in heavy modpacks. Works with the G1 collector only.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={s.tuning.stringDedup} onchange={saveSettings} aria-label={t('Deduplicate strings')} />
        <span class="track"></span>
      </label>
    </div>

    <div class="item col">
      <div class="txt">
        <div class="n">{t('Extra JVM arguments')}</div>
        <div class="d">{t('Added to new instances. Separate arguments with spaces.')}</div>
      </div>
      <input class="wide" bind:value={s.defaultExtraJvmArgs} onchange={saveSettings} placeholder="-Dprop=value" aria-label={t('Extra JVM arguments')} />
    </div>
  </section>

  <section class="group">
    <h2>{t('Launcher')}</h2>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Smooth scrolling')}</div>
        <div class="d">{t('Eases the mouse wheel for a softer scroll.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={s.smoothScroll} onchange={saveSettings} aria-label={t('Smooth scrolling')} />
        <span class="track"></span>
      </label>
    </div>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Reduce visual effects')}</div>
        <div class="d">{t('Turns off animations and transitions. Helps on slower computers.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={s.reduceEffects} onchange={saveSettings} aria-label={t('Reduce visual effects')} />
        <span class="track"></span>
      </label>
    </div>
  </section>
</div>

<style>
  .head { display: grid; gap: 6px; }
  h1.shine { width: fit-content; color: transparent; -webkit-background-clip: text; background-clip: text; background-image: linear-gradient(100deg, var(--text) 38%, color-mix(in srgb, var(--text) 35%, #9b7bff) 50%, var(--text) 62%); background-size: 300% 100%; background-position: 100% 0; animation: shine 5s ease-in-out 1s infinite; }
  @keyframes shine { 0%, 60% { background-position: 100% 0; } 100% { background-position: 0% 0; } }
  .groups { display: grid; gap: 34px; max-width: 820px; }
  .group h2 { margin: 0 0 4px; color: var(--text); font-size: 16px; font-weight: 600; letter-spacing: -0.02em; }
  .intro { margin-bottom: 4px; }
  .item { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 16px 0; border-bottom: 1px solid var(--line); }
  .item.col { flex-direction: column; align-items: stretch; gap: 12px; }
  .row2 { display: flex; align-items: center; justify-content: space-between; gap: 24px; }
  .val { display: flex; align-items: center; gap: 12px; flex: none; }
  .txt { min-width: 0; }
  .n { font-weight: 500; }
  .d { margin-top: 2px; color: var(--muted); font-size: 13px; line-height: 1.45; max-width: 520px; }
  .num { width: 130px; }
  .wide { width: 100%; }
  .presets { display: flex; flex-wrap: wrap; gap: 6px; }
  .presets .on { background: #7c4dff; color: #fff; border-color: #7c4dff; box-shadow: 0 0 14px rgba(124, 77, 255, 0.55); }
  .card { position: relative; overflow: hidden; margin: 14px 0 4px; border-radius: 20px; border: 1px solid var(--line-strong); background: var(--surface-2); isolation: isolate; }
  .body { position: relative; display: grid; gap: 12px; padding: 24px 26px; }
  .eyebrow { color: var(--muted); font-size: 11px; font-weight: 600; letter-spacing: 0.12em; text-transform: uppercase; }
  .tier { font-size: 30px; font-weight: 700; letter-spacing: -0.035em; line-height: 1.05; }
  .facts { display: flex; flex-wrap: wrap; gap: 14px 40px; margin-top: 4px; }
  .facts div { display: grid; gap: 2px; }
  .facts b { font-size: 17px; font-weight: 600; letter-spacing: -0.02em; }
  .why { margin: 0; max-width: 460px; color: var(--muted); font-size: 13px; line-height: 1.5; }
  .actions { display: flex; align-items: center; flex-wrap: wrap; gap: 14px; margin-top: 6px; }
  .warn { color: var(--text); background: var(--surface-2); border-radius: 12px; padding: 10px 14px; margin: 0; }
</style>
