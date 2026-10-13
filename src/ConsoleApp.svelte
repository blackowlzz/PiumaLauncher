<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import type { Window } from '@tauri-apps/api/window'
  import { api, defaultSettings, inTauri, type GameStateEvent } from './lib/api'
  import { i18n, loadLanguage, t } from './lib/i18n.svelte'
  import { applyTheme } from './lib/palette.svelte'
  import Icon from './lib/Icon.svelte'
  import mark from './assets/piuma.png'

  let { instanceId }: { instanceId: string } = $props()

  type Level = 'debug' | 'info' | 'warn' | 'error'
  type Row = { seq: number; time: string; level: Level; source: string; text: string }
  type Pending = { time: string; level: Level; source: string; msg: string[]; thr: string[]; mode: 'none' | 'msg' | 'thr' }

  const LEVELS: Level[] = ['debug', 'info', 'warn', 'error']
  const MAX_ROWS = 10_000
  const SHOWN = 4_000
  const MSG_OPEN = '<log4j:Message><![CDATA['
  const THR_OPEN = '<log4j:Throwable><![CDATA['

  let rows: Row[] = []
  let all = $state.raw<Row[]>([])
  let name = $state('')
  let phase = $state('')
  let query = $state('')
  let enabled = $state<Record<Level, boolean>>({ debug: true, info: true, warn: true, error: true })
  let follow = $state(true)
  let viewport: HTMLDivElement | undefined = $state()
  let win: Window | null = $state(null)
  let maximized = $state(false)
  let copied = $state(false)
  let lastLevel: Level = 'info'
  let lastSeq = -1
  let pending: Pending | null = null
  let frame = 0

  const clock = (ms: number) => new Date(ms).toLocaleTimeString([], { hour12: false })

  function level(raw: string | undefined): Level {
    switch ((raw ?? '').toLowerCase()) {
      case 'trace':
      case 'debug':
        return 'debug'
      case 'warn':
      case 'warning':
        return 'warn'
      case 'error':
      case 'fatal':
      case 'severe':
        return 'error'
      case 'info':
        return 'info'
      default:
        return lastLevel
    }
  }

  const shortSource = (s: string) => s.split('.').pop() ?? s

  function commit(row: Omit<Row, 'seq'>, seq: number) {
    lastLevel = row.level
    rows.push({ ...row, seq })
    if (rows.length > MAX_ROWS) rows.splice(0, rows.length - MAX_ROWS)
    schedule()
  }

  function flush(seq: number) {
    if (!pending) return
    const p = pending
    pending = null
    const text = [...p.msg, ...p.thr].join('\n').replace(/\s+$/, '')
    commit({ time: p.time, level: p.level, source: p.source, text }, seq)
  }

  function capture(p: Pending, mode: 'msg' | 'thr', rest: string) {
    const end = rest.indexOf(']]>')
    const target = mode === 'msg' ? p.msg : p.thr
    if (end >= 0) {
      target.push(rest.slice(0, end))
      p.mode = 'none'
    } else {
      target.push(rest)
      p.mode = mode
    }
  }

  function add(seq: number, text: string) {
    if (seq <= lastSeq) return
    lastSeq = seq

    if (pending) {
      const p = pending
      if (p.mode !== 'none') {
        const target = p.mode === 'msg' ? p.msg : p.thr
        const end = text.indexOf(']]>')
        if (end >= 0) {
          target.push(text.slice(0, end))
          p.mode = 'none'
        } else {
          target.push(text)
        }
        return
      }
      if (text.includes(MSG_OPEN)) return capture(p, 'msg', text.slice(text.indexOf(MSG_OPEN) + MSG_OPEN.length))
      if (text.includes(THR_OPEN)) return capture(p, 'thr', text.slice(text.indexOf(THR_OPEN) + THR_OPEN.length))
      if (text.includes('</log4j:Event>')) return flush(seq)
      if (!text.trimStart().startsWith('<log4j:Event')) return
      flush(seq)
    }

    if (text.trimStart().startsWith('<log4j:Event')) {
      const attr = (k: string) => text.match(new RegExp(`${k}="([^"]*)"`))?.[1]
      const stamp = Number(attr('timestamp'))
      pending = {
        time: stamp ? clock(stamp) : '',
        level: level(attr('level')),
        source: shortSource(attr('logger') ?? ''),
        msg: [],
        thr: [],
        mode: 'none',
      }
      return
    }

    const m = text.match(/^\[(\d\d:\d\d:\d\d)\] \[[^\]/]*\/(\w+)\](?: \[([^\]]*)\])?:? ?(.*)$/)
    if (m) commit({ time: m[1], level: level(m[2]), source: shortSource(m[3] ?? ''), text: m[4] }, seq)
    else commit({ time: '', level: lastLevel, source: '', text }, seq)
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
    const out = all.filter((r) => enabled[r.level] && (!q || `${r.source} ${r.text}`.toLowerCase().includes(q)))
    return out.length > SHOWN ? out.slice(out.length - SHOWN) : out
  })

  const counts = $derived.by(() => {
    const c = { debug: 0, info: 0, warn: 0, error: 0 }
    for (const r of all) c[r.level]++
    return c
  })

  $effect(() => {
    void visible
    if (follow && viewport) tick().then(() => viewport && (viewport.scrollTop = viewport.scrollHeight))
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
    const text = visible.map((r) => `${r.time ? `[${r.time}] ` : ''}[${r.level.toUpperCase()}]${r.source ? ` [${r.source}]` : ''} ${r.text}`).join('\n')
    try {
      await navigator.clipboard.writeText(text)
      copied = true
      setTimeout(() => (copied = false), 1400)
    } catch {}
  }

  function clear() {
    rows = []
    all = []
    pending = null
  }

  const running = $derived(phase === 'running' || phase === 'preparing')

  const edges = [
    ['North', 'n'], ['South', 's'], ['East', 'e'], ['West', 'w'],
    ['NorthEast', 'ne'], ['NorthWest', 'nw'], ['SouthEast', 'se'], ['SouthWest', 'sw'],
  ] as const

  onMount(() => {
    const offs: (() => void)[] = []
    void (async () => {
      const [, settings] = await Promise.all([loadLanguage(), api.getSettings().catch(() => defaultSettings())])
      applyTheme()
      if (settings.theme !== 'system') document.documentElement.dataset.theme = settings.theme
      api.listInstances().then((l) => (name = l.find((i) => i.id === instanceId)?.name ?? instanceId)).catch(() => {})
      if (!inTauri) return
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      win = getCurrentWindow()
      maximized = await win.isMaximized()
      offs.push(await win.onResized(async () => (maximized = await win!.isMaximized())))
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
  <header class="bar" data-tauri-drag-region>
    <div class="id" data-tauri-drag-region>
      <span class="mark" style:--m="url({mark})" aria-hidden="true"></span>
      <span class="nm">{t('Console')}</span>
      <span class="inst">{name || instanceId}</span>
      <span class="state" class:on={running}><span class="dot"></span>{running ? t('Running') : phase === 'exited' ? t('Closed') : ''}</span>
    </div>
    {#if inTauri}
      <div class="ctl">
        <button onclick={() => win?.minimize()} aria-label={t('Minimize')} title={t('Minimize')}><Icon name="minimize" size={16} /></button>
        <button onclick={() => win?.toggleMaximize()} aria-label={maximized ? t('Restore') : t('Maximize')} title={maximized ? t('Restore') : t('Maximize')}>
          <Icon name={maximized ? 'restore' : 'maximize'} size={15} />
        </button>
        <button class="close" onclick={() => win?.close()} aria-label={t('Close')} title={t('Close')}><Icon name="x" size={17} /></button>
      </div>
    {/if}
  </header>

  <div class="tools">
    <label class="find">
      <Icon name="search" size={15} />
      <input type="text" bind:value={query} placeholder={t('Filter lines…')} aria-label={t('Filter lines…')} />
      {#if query}<button class="clr" onclick={() => (query = '')} aria-label={t('Clear')}><Icon name="x" size={14} /></button>{/if}
    </label>
    <div class="seg" role="group" aria-label={t('Levels')}>
      {#each LEVELS as l}
        <button aria-pressed={enabled[l]} onclick={() => (enabled[l] = !enabled[l])}>
          {l}<span class="cnt">{counts[l]}</span>
        </button>
      {/each}
    </div>
    <div class="acts">
      <button class="ghost" onclick={copy}><Icon name={copied ? 'check' : 'file'} size={15} />{copied ? t('Copied') : t('Copy')}</button>
      <button class="ghost" onclick={clear}><Icon name="trash" size={15} />{t('Clear')}</button>
      {#if running}<button onclick={() => api.stop(instanceId).catch(() => {})}>{t('Stop')}</button>{/if}
    </div>
  </div>

  <div class="view" bind:this={viewport} onscroll={onScroll}>
    {#if !visible.length}
      <p class="empty">{all.length ? t('No lines match the filters.') : t('Waiting for the game to write something…')}</p>
    {/if}
    {#each visible as r (r.seq)}
      <div class="row {r.level}">
        <span class="tm">{r.time}</span>
        <span class="lv">{r.level}</span>
        <span class="src" title={r.source}>{r.source}</span>
        <span class="msg">{r.text}</span>
      </div>
    {/each}
  </div>

  {#if !follow}
    <button class="primary end" onclick={jumpToEnd}>{t('Jump to the latest')} ↓</button>
  {/if}
  <footer>{t('{n} lines', { n: all.length })}{visible.length !== all.length ? ` · ${visible.length}` : ''}</footer>
</div>

{#if inTauri && !maximized}
  {#each edges as [dir, cls]}
    <div class="rz {cls}" role="presentation" onpointerdown={(e) => { e.preventDefault(); void win?.startResizeDragging(dir) }}></div>
  {/each}
{/if}

<style>
  .console { height: 100vh; display: flex; flex-direction: column; background: var(--bg); color: var(--text); position: relative; }

  .bar { display: flex; align-items: center; gap: 14px; height: 44px; padding-left: 16px; border-bottom: 1px solid var(--line); user-select: none; -webkit-user-select: none; flex: none; }
  .id { display: flex; align-items: center; gap: 10px; flex: 1; height: 100%; min-width: 0; }
  .id > * { pointer-events: none; }
  .mark { width: 19px; height: 19px; background: var(--text); -webkit-mask: var(--m) center / contain no-repeat; mask: var(--m) center / contain no-repeat; flex: none; }
  .nm { font-size: 14px; font-weight: 600; letter-spacing: -0.03em; }
  .inst { font-size: 13px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .state { display: inline-flex; align-items: center; gap: 7px; font-size: 11px; color: var(--muted); border: 1px solid var(--line-strong); border-radius: 999px; padding: 2px 10px; }
  .state:not(.on):empty { display: none; }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--faint); }
  .state.on { color: var(--text); }
  .state.on .dot { background: var(--text); animation: pulse 1.4s infinite; }
  @keyframes pulse { 50% { opacity: 0.25; } }
  .ctl { display: flex; height: 100%; }
  .ctl button { display: grid; place-items: center; width: 48px; height: 100%; padding: 0; border: 0; border-radius: 0; color: var(--muted); background: transparent; }
  .ctl button:hover:not(:disabled) { background: var(--surface-2); color: var(--text); border: 0; }
  .ctl .close:hover:not(:disabled) { background: #e5484d; color: #fff; }

  .tools { display: flex; align-items: center; gap: 12px; padding: 12px 16px; flex-wrap: wrap; flex: none; }
  .find { display: flex; align-items: center; gap: 10px; flex: 1; min-width: 180px; max-width: 360px; height: 34px; padding: 0 12px 0 14px; border: 1px solid var(--line); border-radius: 10px; background: var(--surface); color: var(--faint); }
  .find:focus-within { border-color: var(--muted); color: var(--muted); }
  .find input { border: 0; background: transparent; padding: 0; height: 100%; font-size: 13px; border-radius: 0; }
  .find input:focus { background: transparent; }
  .clr { display: grid; place-items: center; width: 22px; height: 22px; padding: 0; border: 0; color: var(--muted); }
  .seg > button { display: inline-flex; align-items: center; gap: 7px; padding: 5px 12px; font-size: 12.5px; text-transform: capitalize; }
  .seg > button[aria-pressed='false'] { opacity: 0.5; }
  .cnt { font-size: 11px; color: var(--faint); }
  .seg > button[aria-pressed="true"] .cnt { color: var(--muted); }
  .acts { display: flex; gap: 4px; margin-left: auto; align-items: center; }
  .acts button { display: inline-flex; align-items: center; gap: 7px; padding: 7px 14px; font-size: 13px; }

  .view { flex: 1; overflow: auto; margin: 0 16px; border: 1px solid var(--line); border-radius: var(--radius); background: var(--surface); padding: 6px 0; font-family: ui-monospace, 'JetBrains Mono', 'Cascadia Code', Menlo, Consolas, monospace; font-size: 12px; line-height: 1.6; user-select: text; -webkit-user-select: text; }
  .row { display: grid; grid-template-columns: 62px 44px 128px minmax(0, 1fr); gap: 0 12px; padding: 1px 14px 1px 12px; border-left: 2px solid transparent; }
  .row:hover { background: var(--surface-2); }
  .tm { color: var(--faint); }
  .lv { color: var(--faint); font-size: 10px; text-transform: uppercase; letter-spacing: 0.06em; padding-top: 1px; }
  .src { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .msg { white-space: pre-wrap; overflow-wrap: anywhere; color: var(--text); }
  .row.debug .msg, .row.debug .src { color: var(--faint); }
  .row.warn .lv { color: var(--text); }
  .row.warn .lv::before { content: '! '; }
  .row.error { border-left-color: var(--text); background: color-mix(in srgb, var(--text) 6%, transparent); }
  .row.error .lv { color: var(--text); font-weight: 700; }
  .row.error .msg { font-weight: 500; }
  .empty { margin: 40px 0; text-align: center; color: var(--muted); font-family: 'Inter Tight Variable', system-ui, sans-serif; font-size: 13px; }

  .end { position: absolute; right: 32px; bottom: 44px; box-shadow: 0 8px 30px rgba(0, 0, 0, 0.35); }
  footer { padding: 9px 18px 11px; font-size: 12px; color: var(--muted); flex: none; }

  .rz { position: fixed; z-index: 200; }
  .rz.n { top: 0; left: 10px; right: 10px; height: 4px; cursor: ns-resize; }
  .rz.s { bottom: 0; left: 10px; right: 10px; height: 4px; cursor: ns-resize; }
  .rz.e { right: 0; top: 10px; bottom: 10px; width: 4px; cursor: ew-resize; }
  .rz.w { left: 0; top: 10px; bottom: 10px; width: 4px; cursor: ew-resize; }
  .rz.ne { top: 0; right: 0; width: 10px; height: 10px; cursor: nesw-resize; }
  .rz.nw { top: 0; left: 0; width: 10px; height: 10px; cursor: nwse-resize; }
  .rz.se { bottom: 0; right: 0; width: 10px; height: 10px; cursor: nwse-resize; }
  .rz.sw { bottom: 0; left: 0; width: 10px; height: 10px; cursor: nesw-resize; }
</style>
