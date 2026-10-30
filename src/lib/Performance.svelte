<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type Gc, type Recommendation, type SystemInfo } from './api'
  import { t } from './i18n.svelte'
  import Select from './Select.svelte'
  import PixelField from './PixelField.svelte'
  import Slider from './Slider.svelte'
  import { profileName, profiles } from './meta'
  import { app, saveSettings } from './store.svelte'
  import JavaRuntimes from './JavaRuntimes.svelte'
  import SectionCards, { type SectionCard } from './SectionCards.svelte'
  import SectionHead from './SectionHead.svelte'

  let sys = $state<SystemInfo | null>(null)
  let rec = $state<Recommendation | null>(null)
  let power = $state<{ active: string } | null>(null)
  onMount(() => {
    api.systemInfo().then((s) => (sys = s)).catch(() => {})
    api.recommendPerformance().then((r) => (rec = r)).catch(() => {})
    api.powerInfo().then((p) => (power = p)).catch(() => {})
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

  type Section = 'optimizer' | 'java' | 'gpu' | 'power' | 'aot' | 'memory' | 'jvm' | 'launch' | 'launcher'
  let open = $state<Section | null>((app.section as Section | null) ?? null)
  app.section = null

  function show(id: Section | null) {
    open = id
    document.querySelector('main')?.scrollTo({ top: 0 })
  }

  const forced = $derived.by(() => {
    const id = s.preferredJava
    if (!id) return null
    const r = app.runtimes.find((x) => x.id === id)
    if (!r || r.flavor === 'graalvm') return null
    return `${r.label} ${r.major}`
  })

  function useOptimized() {
    s.preferredJava = null
    saveSettings()
  }

  const cards = $derived.by(() => {
    const extras = Number(!!app.gpu) + Number(!!power)
    const list: (SectionCard & { id: Section })[] = [
      { id: 'optimizer', icon: 'cpu', title: t('Automatic optimizer'), sub: rec ? `${tierName} · ${gb(rec.memoryMaxMb)} · ${profileName(rec.flagProfile)}` : t('Picks memory and Java settings that fit your computer, so you do not have to.'), span: (3 - extras) as 1 | 2 | 3, field: level },
      { id: 'java', icon: 'package', title: t('Java'), sub: forced ? t('Every instance is forced to use {name}.', { name: forced }) : t('GraalVM for modern versions'), tag: t('Recommended') },
    ]
    if (app.gpu) list.push({ id: 'gpu', icon: 'layout', title: t('Graphics card'), sub: app.gpu.name })
    if (power) list.push({ id: 'power', icon: 'flame', title: t('Power profile'), sub: t('Performance mode while playing') })
    list.push(
      { id: 'aot', icon: 'clock', title: t('Faster launches'), sub: s.aotCache ? t('Launch cache') : `${t('Launch cache')} · ${t('Off')}` },
      { id: 'memory', icon: 'box', title: t('Default memory and profile'), sub: `${gb(s.defaultMemoryMaxMb)} · ${profileName(s.defaultFlagProfile)}` },
      { id: 'jvm', icon: 'wrench', title: t('JVM'), sub: [t('Garbage collector'), t('Collector threads'), t('Extra JVM arguments')].join(' · ') },
      { id: 'launch', icon: 'maximize', title: t('Default launch options'), sub: [t('Start in fullscreen'), t('Window size'), t('Launch wrapper')].join(' · ') },
      { id: 'launcher', icon: 'feather', title: t('Launcher'), sub: [t('Smooth scrolling'), t('Reduce visual effects')].join(' · ') },
    )
    return list
  })
  const current = $derived(cards.find((c) => c.id === open) ?? null)

  const pauseCapable = $derived(s.tuning.gc === 'g1' || s.tuning.gc === 'parallel' || s.tuning.gc === 'auto')
</script>

{#if current === null}
  <header class="head">
    <h1 class="shine">{t('Performance')}</h1>
    <p class="muted">{t('Memory, Java and launcher tuning in one place.')}</p>
  </header>

  <SectionCards {cards} onselect={(id) => show(id as Section)} />
{:else}
  <SectionHead title={current.title} back={t('Performance')} onback={() => show(null)} />

<div class="groups">
  {#if open === 'java'}
  <section class="group">
    <div class="jbanner" class:forced={!!forced}>
      <div class="n">{forced ? t('Every instance is forced to use {name}.', { name: forced }) : t('Optimized Java is on')}</div>
      <p class="d">{t('From Minecraft 1.13 on, the launcher uses GraalVM, which compiles the busiest game code more aggressively for higher, steadier FPS. Versions up to 1.12, and Forge up to 1.16, keep Temurin 8, the Java they need.')}</p>
      {#if forced}<div><button class="primary" onclick={useOptimized}>{t('Use optimized Java')}</button></div>{/if}
    </div>
    <JavaRuntimes />
  </section>
  {/if}

  {#if open === 'optimizer'}
  <section class="group auto">
    <p class="muted small intro">{t('Picks memory and Java settings that fit your computer, so you do not have to.')}</p>

    {#if rec}
      <div class="card" data-tier={rec.tier}>
        <div class="field"><PixelField level={level} /></div>
        <div class="body">
          <div class="top">
            <div class="tier">{tierName}</div>
            <button class="primary" disabled={isSaved} onclick={applyRecommendation}>{isSaved ? t('Saved') : t('Save')}</button>
          </div>
          {#each rec.reasons as r}<p class="why">{t(r)}</p>{/each}
          <p class="muted small why">{t('Saves these values as the defaults for every instance.')}</p>
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
  {/if}
  {#if app.gpu}
    {#if open === 'gpu'}
    <section class="group">
      <div class="item">
        <div class="txt">
          <div class="n">{t('Play on the {name} graphics card', { name: app.gpu.name })}</div>
          <div class="d">{t('Your laptop has two graphics cards and games start on the integrated one, which is slower. Turn this on for more FPS; the battery drains faster.')}</div>
        </div>
        <label class="switch">
          <input type="checkbox" bind:checked={s.dedicatedGpu} onchange={saveSettings} aria-label={t('Play on the {name} graphics card', { name: app.gpu.name })} />
          <span class="track"></span>
        </label>
      </div>
    </section>
    {/if}
  {/if}
  {#if power}
    {#if open === 'power'}
    <section class="group">
      <div class="item">
        <div class="txt">
          <div class="n">{t('Performance mode while playing')}</div>
          <div class="d">{t('Switches the computer to its performance power profile while Minecraft runs, then back when you close it.')}</div>
        </div>
        <label class="switch">
          <input type="checkbox" bind:checked={s.performancePower} onchange={saveSettings} aria-label={t('Performance mode while playing')} />
          <span class="track"></span>
        </label>
      </div>
      {#if power.active === 'power-saver'}<p class="warn small">{t('Your computer is in power saver mode, which limits the launcher to 30 FPS. Pick the balanced profile in your system settings for smoother animations.')}</p>{/if}
    </section>
    {/if}
  {/if}

  {#if open === 'aot'}
  <section class="group">
    <div class="item">
      <div class="txt">
        <div class="n">{t('Launch cache')}</div>
        <div class="d">{t('Minecraft 26 and newer reach the menu about a second sooner. The first launch prepares the cache and takes a few extra seconds to close. It uses about 175 MB of disk per instance.')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={s.aotCache} onchange={saveSettings} aria-label={t('Launch cache')} />
        <span class="track"></span>
      </label>
    </div>
  </section>
  {/if}

  {#if open === 'memory'}
  <section class="group">
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
  {/if}

  {#if open === 'jvm'}
  <section class="group">
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
  {/if}

  {#if open === 'launch'}
  <section class="group">
    <p class="d intro">{t('Used by every instance that leaves these empty. An instance can also ignore them from its own settings.')}</p>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Start in fullscreen')}</div>
      </div>
      <label class="switch">
        <input type="checkbox" bind:checked={s.defaultLaunch.fullscreen} onchange={saveSettings} aria-label={t('Start in fullscreen')} />
        <span class="track"></span>
      </label>
    </div>
    <div class="item">
      <div class="txt">
        <div class="n">{t('Window size')}</div>
        <div class="d">{t('Leave empty to use the game default.')}</div>
      </div>
      <div class="val">
        <input class="num" type="number" min="320" max="16384" step="1" placeholder="854" bind:value={s.defaultLaunch.width} onchange={saveSettings} disabled={s.defaultLaunch.fullscreen} aria-label={t('Window width')} />
        <span class="muted">×</span>
        <input class="num" type="number" min="320" max="16384" step="1" placeholder="480" bind:value={s.defaultLaunch.height} onchange={saveSettings} disabled={s.defaultLaunch.fullscreen} aria-label={t('Window height')} />
      </div>
    </div>
    <div class="item col">
      <div class="txt">
        <div class="n">{t('Launch wrapper')}</div>
        <div class="d">{t('A command that starts the game for you, such as gamemoderun, mangohud or prime-run. Leave it empty to start Java directly.')}</div>
      </div>
      <input class="wide" bind:value={s.defaultLaunch.wrapper} onchange={saveSettings} placeholder="gamemoderun mangohud" spellcheck="false" aria-label={t('Launch wrapper')} />
    </div>
    <div class="item col">
      <div class="txt">
        <div class="n">{t('Environment variables')}</div>
        <div class="d">{t('One per line, as NAME=value.')}</div>
      </div>
      <textarea class="wide mono" rows="3" bind:value={s.defaultLaunch.env} onchange={saveSettings} placeholder="MESA_GL_VERSION_OVERRIDE=4.6" spellcheck="false" aria-label={t('Environment variables')}></textarea>
    </div>
  </section>
  {/if}

  {#if open === 'launcher'}
  <section class="group">
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
  {/if}
</div>
{/if}

<style>
  .head { display: grid; gap: 6px; }
  h1.shine { width: fit-content; color: transparent; -webkit-background-clip: text; background-clip: text; background-image: linear-gradient(100deg, var(--text) 38%, color-mix(in srgb, var(--text) 35%, #9b7bff) 50%, var(--text) 62%); background-size: 300% 100%; background-position: 100% 0; animation: shine 5s ease-in-out 1s infinite; }
  @keyframes shine { 0%, 60% { background-position: 100% 0; } 100% { background-position: 0% 0; } }
  .groups { display: grid; gap: 34px; max-width: 820px; }
  .jbanner { display: grid; gap: 8px; padding: 20px 22px; margin-bottom: 8px; border-radius: 16px; border: 1px solid var(--line); background: var(--surface); }
  .jbanner.forced { border-color: var(--line-strong); }
  .jbanner .d { margin: 0; max-width: 620px; }
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
  .mono { resize: vertical; font: 12.5px/1.5 ui-monospace, 'JetBrains Mono', monospace; letter-spacing: 0; }
  .presets { display: flex; flex-wrap: wrap; gap: 6px; }
  .presets .on { background: var(--inv-bg); color: var(--inv-text); border-color: var(--inv-bg); }
  .card { position: relative; overflow: hidden; margin: 12px 0 4px; border-radius: 12px; border: 1px solid var(--line); background: var(--surface); isolation: isolate; }
  .field { position: absolute; inset: 0; }
  .body { position: relative; display: grid; gap: 6px; padding: 16px 18px; }
  .body .why { max-width: 46%; }
  .top { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
  .tier { font-size: 15px; font-weight: 600; letter-spacing: -0.01em; }
  .why { margin: 0; max-width: 60%; color: var(--muted); font-size: 13px; line-height: 1.5; }
  .warn { color: var(--text); background: var(--surface-2); border-radius: 12px; padding: 10px 14px; margin: 0; }
</style>
