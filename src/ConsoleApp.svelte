<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { api, defaultSettings, inTauri, type GameStateEvent } from './lib/api'
  import { i18n, loadLanguage, t } from './lib/i18n.svelte'
  import { applyTheme } from './lib/palette.svelte'

  let { instanceId }: { instanceId: string } = $props()

  type Level = 'trace' | 'debug' | 'info' | 'warn' | 'error'
  type Row = { seq: number; text: string; level: Level }

  const LEVELS: Exclude<Level, 'trace'>[] = ['debug', 'info', 'warn', 'error']
  const MAX_ROWS = 10_000
  const SHOWN = 4_000

  let rows: Row[] = []
  let all = $state.raw<Row[]>([])
  let name = $state('')
  let phase = $state<string>('')
  let query = $state('')
  let enabled = $state<Record<Level, boolean>>({ trace: true, debug: true, info: true, warn: true, error: true })
  let follow = $state(true)
  let viewport: HTMLDivElement | undefined = $state()
  let lastLevel: Level = 'info'
  let lastSeq = -1
  let frame = 0
  let copied = $state(false)

  function levelOf(text: string): Level {
    const m = text.match(/level="(\w+)"/) ?? text.match(/\/(TRACE|DEBUG|INFO|WARN|ERROR|FATAL)\]/) ?? text.match(/\b(TRACE|DEBUG|INFO|WARN|WARNING|ERROR|FATAL|SEVERE)\b/)
    if (!m) return lastLevel
    const v = m[1].toLowerCase()
    if (v === 'fatal' || v === 'severe') return 'error'
    if (v === 'warning') return 'warn'
    return v as Level
  }

  function add(seq: number, text: string) {
    if (seq <= lastSeq) return
    lastSeq = seq
    const level = levelOf(text)
    lastLevel = level
    rows.push({ seq, text, level })
    if (rows.length > MAX_ROWS) rows.splice(0, rows.length - MAX_ROWS)
    schedule()
  }

  function schedule() {
    if (frame) return
    frame = requestAnimationFrame(() => {
      frame = 0
      all = rows.slice()
    })
  }

  const visible = $derived.by(() => {
    const q = query.trim().toLowerCase()
    const out = all.filter((r) => enabled[r.level === 'trace' ? 'debug' : r.level] && (!q || r.text.toLowerCase().includes(q)))
    return out.length > SHOWN ? out.slice(out.length - SHOWN) : out
  })

  const counts = $derived.by(() => {
    const c = { debug: 0, info: 0, warn: 0, error: 0 }
    for (const r of all) c[r.level === 'trace' ? 'debug' : r.level]++
    return c
  })

  $effect(() => {
    void visible
    if (follow && viewport) {
      tick().then(() => viewport && (viewport.scrollTop = viewport.scrollHeight))
    }
  })

  function onScroll() {
    if (!viewport) return
    const atEnd = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight < 24
    if (atEnd !== follow) follow = atEnd
  }

  function jumpToEnd() {
    follow = true
    if (viewport) viewport.scrollTop = viewport.scrollHeight
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(visible.map((r) => r.text).join('\n'))
      copied = true
      setTimeout(() => (copied = false), 1400)
    } catch {}
  }

  function clear() {
    rows = []
    all = []
  }

  const running = $derived(phase === 'running' || phase === 'preparing')

  onMount(() => {
    const offs: (() => void)[] = []
    void (async () => {
      const [, settings] = await Promise.all([loadLanguage(), api.getSettings().catch(() => defaultSettings())])
      applyTheme()
      if (settings.theme !== 'system') document.documentElement.dataset.theme = settings.theme
      api.listInstances().then((l) => (name = l.find((i) => i.id === instanceId)?.name ?? instanceId)).catch(() => {})
      if (!inTauri) return
      offs.push(
        await listen<{ instanceId: string; seq: number; text: string }>('console-line', (e) => {
          if (e.payload.instanceId === instanceId) add(e.payload.seq, e.payload.text)
        }),
        await listen<GameStateEvent>('game-state', (e) => {
          if (e.payload.instanceId === instanceId) phase = e.payload.state
        }),
      )
      for (const l of await api.consoleHistory(instanceId).catch(() => [])) add(l.seq, l.text)
      const live = await api.running().catch(() => [] as string[])
      if (live.includes(instanceId) && !phase) phase = 'running'
    })()
    return () => {
      offs.forEach((o) => o())
      if (frame) cancelAnimationFrame(frame)
    }
  })

  $effect(() => {
    document.title = `${name || instanceId} — ${t('Console')}`
  })
</script>

<div class="console" lang={i18n.lang}>
  <header>
    <div class="who">
      <span class="dot" class:on={running}></span>
      <strong>{name || instanceId}</strong>
      <span class="muted small">{running ? t('Running') : phase === 'exited' ? t('Closed') : ''}</span>
    </div>
    <input type="search" bind:value={query} placeholder={t('Filter lines…')} aria-label={t('Filter lines…')} />
    <div class="levels" role="group" aria-label={t('Levels')}>
      {#each LEVELS as l}
        <button class="lv {l}" aria-pressed={enabled[l]} onclick={() => (enabled[l] = !enabled[l])}>
          {l}<span>{counts[l]}</span>
        </button>
      {/each}
    </div>
    <div class="acts">
      <button onclick={copy}>{copied ? t('Copied') : t('Copy')}</button>
      <button onclick={clear}>{t('Clear')}</button>
      {#if running}<button class="stop" onclick={() => api.stop(instanceId).catch(() => {})}>{t('Stop')}</button>{/if}
    </div>
  </header>

  <div class="view" bind:this={viewport} onscroll={onScroll}>
    {#if !visible.length}
      <p class="empty muted">{all.length ? t('No lines match the filters.') : t('Waiting for the game to write something…')}</p>
    {/if}
    {#each visible as r (r.seq)}
      <div class="row {r.level}">{r.text}</div>
    {/each}
  </div>

  {#if !follow}
    <button class="end" onclick={jumpToEnd}>{t('Jump to the latest')} ↓</button>
  {/if}
  <footer class="muted small">{t('{n} lines', { n: all.length })}{visible.length !== all.length ? ` · ${visible.length}` : ''}</footer>
</div>

<style>
  .console { height: 100vh; display: flex; flex-direction: column; background: var(--bg); color: var(--text); position: relative; }
  header { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; padding: 10px 14px; border-bottom: 1px solid var(--line); }
  .who { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--muted); flex: none; }
  .dot.on { background: #4ade80; animation: pulse 1.4s infinite; }
  @keyframes pulse { 50% { opacity: 0.3; } }
  input[type='search'] { flex: 1; min-width: 160px; max-width: 320px; }
  .levels { display: flex; gap: 6px; }
  .lv { display: inline-flex; gap: 6px; align-items: center; padding: 4px 10px; font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; opacity: 0.45; }
  .lv[aria-pressed='true'] { opacity: 1; }
  .lv span { color: var(--muted); font-variant-numeric: tabular-nums; }
  .lv.warn[aria-pressed='true'] { color: #f5b942; }
  .lv.error[aria-pressed='true'] { color: #ff6b6b; }
  .acts { display: flex; gap: 6px; margin-left: auto; }
  .stop { color: #ff6b6b; }
  .view { flex: 1; overflow: auto; padding: 8px 14px; font-family: ui-monospace, 'JetBrains Mono', 'Cascadia Code', Menlo, Consolas, monospace; font-size: 12px; line-height: 1.55; user-select: text; }
  .row { white-space: pre-wrap; word-break: break-all; color: var(--muted); }
  .row.info { color: var(--text); }
  .row.debug, .row.trace { color: var(--muted); opacity: 0.75; }
  .row.warn { color: #f5b942; }
  .row.error { color: #ff6b6b; }
  .empty { margin: 24px 0; text-align: center; font-family: inherit; }
  .end { position: absolute; right: 22px; bottom: 40px; box-shadow: 0 6px 24px rgba(0, 0, 0, 0.35); }
  footer { padding: 6px 14px; border-top: 1px solid var(--line); }
</style>
