import { listen } from '@tauri-apps/api/event'
import { loadLanguage, t } from './i18n.svelte'
import { ping, serverKey } from './quickplay.svelte'
import {
  api,
  inTauri,
  modrinth,
  stats,
  type NewsItem,
  defaultSettings,
  type ProjectHit,
  type Settings,
  type ProjectType,
  type Session,
  type AccountStore,
  type UpdateInfo,
  type GameStateEvent,
  type CrashReport,
  type QuickPlay,
  type SavedServer,
  type Imported,
  type InstallProgress,
  type RuntimeEntry,
  type JavaInstall,
  type Instance,
  type VersionEntry,
} from './api'

export type Page = 'home' | 'library' | 'quickplay' | 'explore' | 'accounts' | 'instance' | 'settings' | 'themes' | 'performance'

const MAX_STAT_SAMPLES = 90

export const app = $state({
  page: 'home' as Page,
  detailId: null as string | null,
  instances: [] as Instance[],
  accounts: { accounts: [], active: null } as AccountStore,
  versions: [] as VersionEntry[],
  selectedId: null as string | null,
  games: {} as Record<string, GameStateEvent>,
  toast: null as string | null,
  confirm: null as { message: string; confirmLabel: string; resolve: (ok: boolean) => void } | null,
  loaded: false,
  settings: defaultSettings() as Settings,
  versionImages: {} as Record<string, string>,
  install: null as InstallProgress | null,
  runtimes: [] as RuntimeEntry[],
  javaBusy: null as string | null,
  stats: {} as Record<string, number[]>,
  featured: [] as ProjectHit[],
  sessions: [] as Session[],
  installedVersions: [] as string[],
  news: [] as NewsItem[],
  installingId: null as string | null,
  detail: null as ProjectHit | null,
  exploreIntent: null as { kind: ProjectType; targetId?: string } | null,
  update: null as UpdateInfo | null,
  updateDismissed: false,
  crash: null as CrashReport | null,
  joining: null as SavedServer | null,
})

let toastTimer: ReturnType<typeof setTimeout> | undefined

export function notify(message: unknown) {
  app.toast = String(message)
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (app.toast = null), 6000)
}

export function askConfirm(message: string, confirmLabel = t('Delete')): Promise<boolean> {
  app.confirm?.resolve(false)
  return new Promise((resolve) => {
    app.confirm = {
      message,
      confirmLabel,
      resolve: (ok) => {
        app.confirm = null
        resolve(ok)
      },
    }
  })
}

export async function reloadInstances() {
  app.instances = await api.listInstances()
  if (!app.instances.some((i) => i.id === app.selectedId)) {
    app.selectedId = app.instances[0]?.id ?? null
  }
}

export async function reloadAccounts() {
  app.accounts = await api.listAccounts()
}

export const selectedInstance = () => app.instances.find((i) => i.id === app.selectedId) ?? null
export const activeAccount = () =>
  app.accounts.accounts.find((a) => a.id === app.accounts.active) ?? null

export async function init() {
  const settings = await api.getSettings().catch(() => defaultSettings())
  await loadLanguage(settings.language)
  app.settings = { ...defaultSettings(), ...settings }
  api.versionImages().then((m) => (app.versionImages = m)).catch(() => {})
  if (inTauri) await listenToBackend()
  await Promise.all([reloadInstances(), reloadAccounts()])
  app.loaded = true
  reloadStats()
  const checkUpdate = () =>
    api
      .checkUpdate()
      .then((u) => {
        if (u && u.version !== app.update?.version) app.updateDismissed = false
        if (u) app.update = u
      })
      .catch(() => {})
  setTimeout(checkUpdate, 4000)
  setInterval(checkUpdate, 30 * 60 * 1000)
  stats.news().then((n) => (app.news = n)).catch(() => {})
  api.listVersions().then((v) => (app.versions = v)).catch((e) => notify(t('Version list unavailable: {error}', { error: e })))
  modrinth
    .search({ query: '', projectType: 'modpack', loaders: [], categories: [], sort: 'downloads', offset: 0 })
    .then((r) => (app.featured = r.hits.slice(0, 10)))
    .catch(() => {})
}

async function listenToBackend() {
  await listen<GameStateEvent>('game-state', (e) => {
    const ev = e.payload
    app.games[ev.instanceId] = ev
    if (ev.state === 'preparing') {
      app.stats[ev.instanceId] = []
    }
    if (ev.state === 'error') notify(ev.message ?? t('Launch failed'))
    if (ev.state === 'exited' || ev.state === 'running') reloadStats()
  })
  await listen<{ instanceId: string; rssMb: number }>('game-stats', (e) => {
    const samples = (app.stats[e.payload.instanceId] ??= [])
    samples.push(e.payload.rssMb)
    if (samples.length > MAX_STAT_SAMPLES) samples.shift()
  })
  await listen<InstallProgress>('install-progress', (e) => (app.install = e.payload))
  await listen<CrashReport>('game-crash', (e) => (app.crash = e.payload))
  await listen('cli-request', () => handleCli())
}

export function openProject(hit: ProjectHit) {
  app.detail = hit
}

export function openInstance(id: string) {
  app.selectedId = id
  app.detailId = id
  app.page = 'instance'
}

export async function play(id: string, quickPlay?: QuickPlay) {
  app.selectedId = id
  try {
    await api.launch(id, quickPlay)
  } catch (e) {
    notify(e)
  }
}

export async function importPack(run: () => Promise<Imported>): Promise<boolean> {
  if (app.installingId) return false
  app.installingId = 'import'
  app.install = null
  try {
    const r = await run()
    await reloadInstances()
    app.selectedId = r.instance.id
    notify(
      r.skipped
        ? t('"{name}" imported, {n} files are no longer available', { name: r.instance.name, n: r.skipped })
        : t('"{name}" imported', { name: r.instance.name }),
    )
    return true
  } catch (e) {
    notify(e)
    return false
  } finally {
    app.installingId = null
    app.install = null
  }
}

export async function handleCli() {
  const r = await api.takeCliRequest().catch(() => null)
  if (!r) return
  if (r.pack) {
    const path = r.pack
    app.page = 'library'
    await importPack(() => api.importMrpackFile(path))
    return
  }
  const quick: QuickPlay | undefined = r.server ? { kind: 'server', address: r.server } : r.world ? { kind: 'world', folder: r.world } : undefined
  if (r.instance) {
    const wanted = r.instance.toLowerCase()
    const named = app.instances.filter((i) => i.name.toLowerCase() === wanted)
    const inst = app.instances.find((i) => i.id === r.instance) ?? (named.length === 1 ? named[0] : null)
    if (!inst) {
      notify(named.length > 1 ? t('More than one instance is called "{name}"', { name: r.instance }) : t('No instance called "{name}"', { name: r.instance }))
      return
    }
    await play(inst.id, quick)
    return
  }
  if (r.server) {
    const address = r.server
    const saved = await api.listServers().catch(() => [] as SavedServer[])
    ping(address)
    app.joining = saved.find((s) => serverKey(s.address) === serverKey(address)) ?? { name: address, address, icon: null, instances: [] }
    return
  }
  if (r.world) notify(t('Add --instance to say which instance the world belongs to'))
}

export async function createShortcut(id: string): Promise<boolean> {
  try {
    await api.createShortcut(id)
    return true
  } catch (e) {
    notify(e)
    return false
  }
}

export const stop = (id: string) => api.stop(id).catch(notify)

export const isBusy = (id: string | null) => {
  const state = id ? app.games[id]?.state : undefined
  return state === 'preparing' || state === 'running'
}

export async function installModpack(projectId: string, versionId?: string, mcVersion?: string | null): Promise<boolean> {
  if (app.installingId) return false
  app.installingId = projectId
  app.install = null
  try {
    const inst = await modrinth.installModpack(projectId, versionId, mcVersion ?? undefined)
    await reloadInstances()
    app.selectedId = inst.id
    notify(t('"{name}" installed', { name: inst.name }))
    return true
  } catch (e) {
    notify(e)
    return false
  } finally {
    app.installingId = null
    app.install = null
  }
}

export async function reloadStats() {
  const [sessions, installed] = await Promise.all([
    stats.sessions().catch(() => [] as Session[]),
    stats.installedVersions().catch(() => [] as string[]),
  ])
  app.sessions = sessions
  app.installedVersions = installed
}

export function playInfo(id: string) {
  let secs = 0
  let last = 0
  for (const s of app.sessions) {
    if (s.instanceId !== id) continue
    secs += s.secs
    last = Math.max(last, s.startMs)
  }
  return { secs, last }
}

export function weeklyPlaytime() {
  const day = new Date()
  day.setHours(0, 0, 0, 0)
  const start = day.getTime() - 6 * 86_400_000
  const days = Array.from({ length: 7 }, (_, i) => ({ date: new Date(start + i * 86_400_000), secs: 0 }))
  for (const s of app.sessions) {
    const d = new Date(s.startMs)
    d.setHours(0, 0, 0, 0)
    const i = Math.round((d.getTime() - start) / 86_400_000)
    if (i >= 0 && i < 7) days[i].secs += s.secs
  }
  return days
}

export const isInstalled = (mcVersion: string) => app.installedVersions.includes(mcVersion)

export const saveSettings = () => api.saveSettings($state.snapshot(app.settings)).catch(notify)

export function photoFor(version: string | undefined, seed: string): string | null {
  const map = app.versionImages
  const keys = Object.keys(map)
  if (!keys.length) return null
  if (version && map[version]) return map[version]
  if (version) {
    const series = version.split('.').slice(0, 2).join('.')
    const near = keys.filter((k) => k === series || k.startsWith(`${series}.`) || k.startsWith(`${series}-`)).sort()
    if (near.length) return map[near[near.length - 1]]
  }
  let h = 0
  for (const c of seed) h = (h * 31 + c.charCodeAt(0)) >>> 0
  return map[keys[h % keys.length]]
}

export async function reloadRuntimes() {
  app.runtimes = await api.listRuntimes().catch(() => [] as RuntimeEntry[])
}

export async function installRuntime(r: RuntimeEntry): Promise<boolean> {
  if (app.javaBusy) return false
  app.javaBusy = r.id
  app.install = null
  try {
    await api.installRuntime(r.flavor, r.major)
    await reloadRuntimes()
    return true
  } catch (e) {
    notify(e)
    return false
  } finally {
    app.javaBusy = null
    app.install = null
  }
}

export async function installedJavas(): Promise<JavaInstall[]> {
  const [found] = await Promise.all([api.detectJava().catch(() => [] as JavaInstall[]), reloadRuntimes()])
  const seen = new Set(found.map((j) => j.path))
  const managed = app.runtimes
    .filter((r) => r.installed && r.path && !seen.has(r.path))
    .map((r) => ({ path: r.path!, major: r.major, version: r.version ?? '', isGraal: r.flavor === 'graalvm' }))
  return [...managed, ...found].sort((a, b) => b.major - a.major)
}
