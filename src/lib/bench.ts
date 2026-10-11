import { api } from './api'
import { app } from './store.svelte'

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

function stats(frames: number[]) {
  let t = 0
  const slowAt: number[] = []
  frames.forEach((f) => {
    t += f
    if (f > 20) slowAt.push(Math.round(t))
  })
  const sorted = [...frames].sort((a, b) => a - b)
  const avg = frames.reduce((n, f) => n + f, 0) / Math.max(1, frames.length)
  return {
    frames: frames.length,
    avgMs: +avg.toFixed(2),
    fps: +(1000 / avg).toFixed(1),
    p95Ms: +(sorted[Math.floor(sorted.length * 0.95)] ?? 0).toFixed(2),
    maxMs: +(sorted[sorted.length - 1] ?? 0).toFixed(2),
    slowFrames: frames.filter((f) => f > 20).length,
    slowAt,
  }
}

function measure(ms: number, step: (p: number) => void) {
  return new Promise<ReturnType<typeof stats>>((resolve) => {
    const frames: number[] = []
    const t0 = performance.now()
    let last = t0
    const tick = (now: number) => {
      frames.push(now - last)
      last = now
      step(Math.min(1, (now - t0) / ms))
      if (now - t0 < ms) requestAnimationFrame(tick)
      else resolve(stats(frames.slice(2)))
    }
    requestAnimationFrame(tick)
  })
}

export async function maybeBench() {
  const out = await api.benchInfo().catch(() => null)
  if (!out) return
  await sleep(5000)
  const main = document.querySelector('main') as HTMLElement
  const idle = await measure(3000, () => {})
  const range = Math.max(0, main.scrollHeight - main.clientHeight)
  const scroll = await measure(5000, (p) => {
    main.scrollTop = (p < 0.5 ? p * 2 : (1 - p) * 2) * range
  })
  const tab = [...document.querySelectorAll('nav .tab')].find((b) => /explore|esplora/i.test(b.textContent ?? '')) as HTMLElement | undefined
  tab?.click()
  await sleep(4500)
  const exploreMain = document.querySelector('main') as HTMLElement
  const exploreRange = Math.max(0, exploreMain.scrollHeight - exploreMain.clientHeight)
  const explore = await measure(5000, (p) => {
    exploreMain.scrollTop = (p < 0.5 ? p * 2 : (1 - p) * 2) * exploreRange
  })
  const paneInfo = () => {
    const pane = document.querySelector('.modal .pane') as HTMLElement | null
    const table = document.querySelector('.modal .table') as HTMLElement | null
    if (!pane) return null
    pane.scrollTop = 120
    return { scrollHeight: pane.scrollHeight, clientHeight: pane.clientHeight, scrolledTo: pane.scrollTop, tableHeight: table?.getBoundingClientRect().height ?? 0, rows: document.querySelectorAll('.modal .tr').length }
  }
  const detail: Record<string, unknown> = {}
  const hit = app.featured[0]
  if (hit) {
    app.detail = hit
    await sleep(9000)
    detail.contents = paneInfo()
    ;(document.querySelectorAll('.modal [role=tab]')[2] as HTMLElement | undefined)?.click()
    await sleep(800)
    detail.versions = paneInfo()
    app.detail = null
  }
  const zoom = (document.documentElement.style as CSSStyleDeclaration & { zoom?: string }).zoom || '1'
  await api.benchDone(JSON.stringify({ detail, diag: { home: [main.scrollHeight, main.clientHeight], explore: [exploreMain.scrollHeight, exploreMain.clientHeight], cards: document.querySelectorAll('.card').length, welcome: !!document.querySelector('.welcome') }, idle, scroll, explore, exploreRange, scrollRange: range, viewport: [innerWidth, innerHeight], dpr: devicePixelRatio, cssZoom: zoom }, null, 2))
}
