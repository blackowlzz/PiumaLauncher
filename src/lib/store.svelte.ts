import { listen } from '@tauri-apps/api/event'
import { loadLanguage, t, translateError } from './i18n.svelte'
import type { MenuItem } from './contextmenu'
import { ping, serverKey } from './serverstatus.svelte'
import { contentKindOf, formatDuration } from './meta'
import { breakStep, secondsToday } from './playtime'
import {
  api,
  inTauri,
  modrinth,
  stats,
  type EffectiveLaunch,
  type NewsItem,
  defaultSettings,
  type PackUpdate,
  type StartupReport,
  type SponsoredServer,
  type PackVersion,
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

export type Page = 'home' | 'library' | 'servers' | 'explore' | 'accounts' | 'instance' | 'settings' | 'themes' | 'performance' | 'wardrobe' | 'worlds' | 'stats'

const MAX_STAT_SAMPLES = 90

export const app = $state({
  page: 'home' as Page,
  detailId: null as string | null,
  wardrobeId: null as string | null,
  instances: [] as Instance[],
  accounts: { accounts: [], active: null } as AccountStore,
  versions: [] as VersionEntry[],
  selectedId: null as string | null,
  games: {} as Record<string, GameStateEvent>,
  toast: null as string | null,
  confirm: null as { message: string; confirmLabel: string; danger: boolean; resolve: (ok: boolean) => void } | null,
  loaded: false,
  settings: defaultSettings() as Settings,
  versionImages: {} as Record<string, string>,
  install: null as InstallProgress | null,
  runtimes: [] as RuntimeEntry[],
  javaBusy: null as string | null,
  stats: {} as Record<string, number[]>,
  effective: {} as Record<string, EffectiveLaunch>,
  featured: [] as ProjectHit[],
  sessions: [] as Session[],
  installedVersions: [] as string[],
  news: [] as NewsItem[],
  installingId: null as string | null,
  cancellable: false,
  packUpdates: {} as Record<string, PackUpdate>,
  gpu: null as { name: string } | null,
  sponsored: null as SponsoredServer[] | null,
  startup: {} as Record<string, StartupReport>,
  cancelling: false,
  detail: null as { hit: ProjectHit; type: ProjectType } | null,
  picker: null as { hit: ProjectHit; type: ProjectType; version?: PackVersion } | null,
  contentRev: 0,
  exploreIntent: null as { kind: ProjectType; targetId?: string } | null,
  update: null as UpdateInfo | null,
  updateDismissed: false,
  crash: null as CrashReport | null,
  joining: null as SavedServer | null,
  ctx: null as { x: number; y: number; items: MenuItem[] } | null,
  sharingId: null as string | null,
  inviting: null as { instanceId: string; name: string; address: string } | null,
})

let toastTimer: ReturnType<typeof setTimeout> | undefined

export function notify(message: unknown) {
  app.toast = translateError(message)
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (app.toast = null), 6000)
}

export function askConfirm(message: string, confirmLabel = t('Delete'), danger = true): Promise<boolean> {
  app.confirm?.resolve(false)
  return new Promise((resolve) => {
    app.confirm = {
      message,
      confirmLabel,
      danger,
      resolve: (ok) => {
        app.confirm = null
        resolve(ok)
      },
    }
  })
}

export async function refreshEffective(id: string) {
  try {
    app.effective[id] = await api.effectiveLaunch(id)
  } catch {
    delete app.effective[id]
  }
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
  api.gpuInfo().then((g) => (app.gpu = g)).catch(() => {})
  if (inTauri) await listenToBackend()
  await Promise.all([reloadInstances(), reloadAccounts()])
  const remembered = app.settings.lastInstance
  if (remembered && app.instances.some((i) => i.id === remembered)) app.selectedId = remembered
  app.loaded = true
  const initial = app.selectedId
  reloadStats().then(() => {
    if (remembered || app.selectedId !== initial) return
    const recent = lastPlayed()
    if (recent) app.selectedId = recent
  })
  const checkUpdate = () =>
    api
      .checkUpdate()
      .then((u) => {
        if (u && u.version !== app.update?.version) app.updateDismissed = false
        if (u) app.update = u
      })
      .catch(() => {})
  setTimeout(checkUpdate, 4000)
  setTimeout(refreshPackUpdates, 2500)
  setInterval(checkPlaytime, 30_000)
  setInterval(checkUpdate, 30 * 60 * 1000)
  stats.news().then((n) => (app.news = n)).catch(() => {})
  api.listVersions().then((v) => (app.versions = v)).catch((e) => notify(t('Version list unavailable: {error}', { error: e })))
  modrinth
    .search({ query: '', projectType: 'modpack', loaders: [], categories: [], sort: 'downloads', offset: 0 })
    .then((r) => (app.featured = r.hits.slice(0, 10)))
    .catch(() => {})
}

async function listenToBackend() {
  await listen<{ instanceId: string }>('startup-measured', (e) => void refreshStartup(e.payload.instanceId))
  await listen<GameStateEvent>('game-state', (e) => {
    const ev = e.payload
    app.games[ev.instanceId] = ev
    if (ev.state === 'preparing') {
      app.stats[ev.instanceId] = []
    }
    if (ev.state === 'error') notify(ev.message ?? t('Launch failed'))
    if (ev.state === 'running' && ev.message) notify(t(ev.message))
    if (ev.state === 'running') runningSince[ev.instanceId] ??= Date.now()
    if (ev.state === 'exited' || ev.state === 'error') {
      delete runningSince[ev.instanceId]
      delete breaksShown[ev.instanceId]
    }
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

export function openProject(hit: ProjectHit, type: ProjectType = 'modpack') {
  app.detail = { hit, type }
}

export function openInstance(id: string) {
  app.selectedId = id
  app.detailId = id
  app.page = 'instance'
}

const runningSince: Record<string, number> = {}
const breaksShown: Record<string, number> = {}
let limitShownOn = ''

export function playedTodaySecs(): number {
  return secondsToday(app.sessions, Object.values(runningSince), Date.now())
}

async function osNotify(title: string, body: string) {
  notify(`${title} ${body}`)
  if (!inTauri) return
  try {
    const n = await import('@tauri-apps/plugin-notification')
    if ((await n.isPermissionGranted()) || (await n.requestPermission()) === 'granted') n.sendNotification({ title, body })
  } catch {}
}

function checkPlaytime() {
  const { breakReminderMinutes: every, dailyLimitMinutes: limit } = app.settings
  const now = Date.now()
  if (every > 0) {
    for (const [id, since] of Object.entries(runningSince)) {
      const step = breakStep(since, now, every)
      if (step >= 1 && step > (breaksShown[id] ?? 0)) {
        breaksShown[id] = step
        void osNotify(t('Time for a break?'), t('You have been playing for {time}.', { time: formatDuration(step * every * 60) }))
      }
    }
  }
  const today = new Date().toDateString()
  if (limit > 0 && Object.keys(runningSince).length && limitShownOn !== today && playedTodaySecs() >= limit * 60) {
    limitShownOn = today
    void osNotify(t('Daily limit reached'), t('You have played {time} today, your limit is {limit}.', { time: formatDuration(playedTodaySecs()), limit: formatDuration(limit * 60) }))
  }
}

export async function play(id: string, quickPlay?: QuickPlay) {
  app.selectedId = id
  if (app.settings.lastInstance !== id) {
    app.settings.lastInstance = id
    saveSettings()
  }
  const limit = app.settings.dailyLimitMinutes
  if (limit > 0 && playedTodaySecs() >= limit * 60) {
    const msg = t('You have played {time} today, your limit is {limit}. Play anyway?', { time: formatDuration(playedTodaySecs()), limit: formatDuration(limit * 60) })
    if (!(await askConfirm(msg, t('Play anyway'), false))) return
  }
  try {
    const leftovers = await api.leftoverProcesses(id).catch(() => 0)
    if (leftovers > 0 && !(await askConfirm(t('This instance is still open in the background, probably from a previous session that did not close properly. Close it and play?'), t('Close and play')))) return
    await api.launch(id, quickPlay, leftovers > 0)
  } catch (e) {
    notify(e)
  }
}

const wasCancelled = (e: unknown) => String(e) === 'Cancelled'

function reportFailure(e: unknown) {
  notify(wasCancelled(e) ? t('Cancelled') : e)
}

function beginInstall(id: string) {
  app.installingId = id
  app.install = null
  app.cancellable = true
  app.cancelling = false
}

function endInstall() {
  app.installingId = null
  app.install = null
  app.cancellable = false
  app.cancelling = false
}

export async function cancelInstall() {
  if (!app.cancellable || app.cancelling) return
  app.cancelling = true
  if (!(await modrinth.cancelInstall().catch(() => false))) app.cancelling = false
}

export async function importPack(run: () => Promise<Imported>): Promise<boolean> {
  if (app.installingId) return false
  beginInstall('import')
  try {
    const r = await run()
    await reloadInstances()
    app.selectedId = r.instance.id
    notify(
      r.skipped
        ? t('"{name}" imported, {n} files are no longer available', { name: r.instance.name, n: r.skipped })
        : t('"{name}" imported', { name: r.instance.name }),
    )
    if (r.server) void joinInvite(r.instance.id, r.server)
    return true
  } catch (e) {
    reportFailure(e)
    return false
  } finally {
    endInstall()
  }
}

async function joinInvite(instanceId: string, address: string) {
  await api.addServer([instanceId], address, address).catch(() => {})
  if (await askConfirm(t('Join {server} now?', { server: address }), t('Join'), false)) await play(instanceId, { kind: 'server', address })
}

export function invite(server: SavedServer, instanceId?: string) {
  const id =
    instanceId ??
    [...server.instances].sort((a, b) => playInfo(b).last - playInfo(a).last).find((i) => app.instances.some((x) => x.id === i))
  if (id) app.inviting = { instanceId: id, name: server.name, address: server.address }
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
  return state === 'preparing' || state === 'running' || (id !== null && app.installingId === id)
}

export function installProject(hit: ProjectHit, type: ProjectType, version?: PackVersion): Promise<boolean> {
  if (type === 'modpack') return installModpack(hit.projectId, version?.id, hit.latestMc)
  app.picker = { hit, type, version }
  return Promise.resolve(false)
}

export async function installContentInto(instanceIds: string[], hit: ProjectHit, type: ProjectType, versionId?: string): Promise<boolean> {
  if (app.installingId || !instanceIds.length) return false
  beginInstall(hit.projectId)
  const added: string[] = []
  let failed: unknown = null
  for (const id of instanceIds) {
    if (app.cancelling) break
    try {
      added.push(...(await modrinth.installContent(id, hit.projectId, contentKindOf(type), versionId)))
    } catch (e) {
      failed ??= e
      if (wasCancelled(e)) break
    }
  }
  app.contentRev++
  endInstall()
  if (added.length) notify(t('Installed: {files}', { files: [...new Set(added)].join(', ') }))
  if (failed) reportFailure(failed)
  return !failed
}

export async function updateInstalled(instanceId: string, type: ProjectType, fileName: string, projectId: string): Promise<boolean> {
  if (app.installingId) return false
  beginInstall(projectId)
  try {
    const name = await modrinth.updateContent(instanceId, contentKindOf(type), fileName)
    app.contentRev++
    notify(t('Installed: {files}', { files: name }))
    return true
  } catch (e) {
    reportFailure(e)
    return false
  } finally {
    endInstall()
  }
}

export async function installModpack(projectId: string, versionId?: string, mcVersion?: string | null): Promise<boolean> {
  if (app.installingId) return false
  beginInstall(projectId)
  try {
    const inst = await modrinth.installModpack(projectId, versionId, mcVersion ?? undefined)
    await reloadInstances()
    app.selectedId = inst.id
    notify(t('"{name}" installed', { name: inst.name }))
    return true
  } catch (e) {
    reportFailure(e)
    return false
  } finally {
    endInstall()
  }
}

export async function refreshStartup(id: string) {
  const r = await api.startupReport(id).catch(() => null)
  if (r) app.startup[id] = r
}

export const seconds = (ms: number) => `${Math.max(1, Math.round(ms / 1000))} s`

export async function refreshPackUpdates() {
  app.packUpdates = await modrinth.modpackUpdates().catch(() => app.packUpdates)
}

export async function refreshPackUpdate(id: string) {
  const u = await modrinth.modpackUpdate(id).catch(() => undefined)
  if (u) app.packUpdates[id] = u
  else if (u === null) delete app.packUpdates[id]
}

export async function updateModpack(id: string, versionId?: string): Promise<boolean> {
  if (app.installingId) return false
  beginInstall(id)
  try {
    const inst = await modrinth.updateModpack(id, versionId)
    delete app.packUpdates[id]
    await reloadInstances()
    notify(t('"{name}" updated to {version}', { name: inst.name, version: inst.pack?.versionNumber ?? '' }))
    return true
  } catch (e) {
    reportFailure(e)
    return false
  } finally {
    endInstall()
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

function lastPlayed(): string | null {
  let best: { id: string; at: number } | null = null
  for (const s of app.sessions) {
    if (app.instances.some((i) => i.id === s.instanceId) && (!best || s.startMs > best.at)) best = { id: s.instanceId, at: s.startMs }
  }
  return best?.id ?? null
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
