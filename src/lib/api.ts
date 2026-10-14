import { invoke } from '@tauri-apps/api/core'

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (inTauri) return invoke<T>(cmd, args)
  if (import.meta.env.DEV) {
    const { mock } = await import('./mock')
    return (await mock(cmd, args ?? {})) as T
  }
  throw new Error('Backend non disponibile: apri PiumaLauncher come app')
}

export type Loader = 'vanilla' | 'fabric' | 'quilt' | 'forge' | 'neoforge'

export type FlagProfile = 'low' | 'balanced' | 'performance' | 'none'
export type Gc = 'auto' | 'g1' | 'zgc' | 'parallel' | 'serial'

export interface Tuning {
  gc: Gc
  pauseMs: number
  gcThreads: number
  preTouch: boolean
  disableExplicitGc: boolean
  stringDedup: boolean
}

export interface SystemInfo {
  totalMemoryMb: number
  cpuCores: number
}

export interface Recommendation {
  tier: 'weak' | 'modest' | 'capable' | 'strong'
  memoryMaxMb: number
  memoryMinMb: number
  flagProfile: FlagProfile
  tuning: Tuning
  reasons: string[]
}

export interface Instance {
  id: string
  name: string
  mcVersion: string
  loader: Loader
  loaderVersion: string | null
  launchVersion: string | null
  memoryMaxMb: number
  memoryMinMb: number
  flagProfile: FlagProfile
  javaPath: string | null
  extraJvmArgs: string
  iconUrl: string | null
  bannerUrl: string | null
}

export interface Account {
  id: string
  name: string
  uuid: string
  kind: 'offline' | 'microsoft'
}

export interface DeviceLogin {
  userCode: string
  verificationUri: string
  expiresIn: number
}

export interface UpdateInfo {
  version: string
  url: string
}

export interface AccountStore {
  accounts: Account[]
  active: string | null
}

export interface VersionEntry {
  id: string
  kind: 'release' | 'snapshot' | 'old_beta' | 'old_alpha'
  releaseTime: string
}

export interface JavaInstall {
  path: string
  major: number
  version: string
  isGraal: boolean
}

export interface GameStateEvent {
  instanceId: string
  state: 'preparing' | 'running' | 'exited' | 'error'
  stage: string | null
  done: number
  total: number
  message: string | null
  code: number | null
}

export type CrashFix =
  | { kind: 'disableMods'; files: string[] }
  | { kind: 'installMods'; projects: string[] }
  | { kind: 'memory'; mb: number }
  | { kind: 'autoJava' }
  | { kind: 'clearJvmArgs' }

export type CrashCause =
  | 'outOfMemory'
  | 'heapTooLarge'
  | 'javaVersion'
  | 'jvmArgs'
  | 'missingDeps'
  | 'incompatibleMods'
  | 'duplicateMods'
  | 'mixin'
  | 'suspectedMod'
  | 'graphics'
  | 'nativeCrash'
  | 'unknown'

export interface CrashReport {
  instanceId: string
  cause: CrashCause
  subject: string | null
  fix: CrashFix | null
  excerpt: string[]
  hasReport: boolean
  code: number | null
}

export type QuickPlay = { kind: 'server'; address: string } | { kind: 'world'; folder: string }

export interface SavedServer {
  name: string
  address: string
  icon: string | null
  instances: string[]
}

export interface ServerStatus {
  motd: string
  version: string
  protocol: number
  online: number
  max: number
  latencyMs: number
  favicon: string | null
  sample: string[]
}

export interface CliRequest {
  instance: string | null
  server: string | null
  world: string | null
  pack: string | null
}

export interface ExportOptions {
  resourcepacks: boolean
  shaderpacks: boolean
  config: boolean
  options: boolean
  servers: boolean
}

export interface Exported {
  path: string
  linked: number
  bundled: number
}

export interface ShareCode {
  code: string
  included: number
  excluded: string[]
}

export interface Imported {
  instance: Instance
  skipped: number
}

export interface RecentWorld {
  instanceId: string
  folder: string
  name: string
  modifiedMs: number
  mode: number | null
  hardcore: boolean
  icon: string | null
}

export const api = {
  listVersions: () => call<VersionEntry[]>('list_mc_versions'),
  listInstances: () => call<Instance[]>('list_instances'),
  createInstance: (name: string, mcVersion: string, loader: Loader) =>
    call<Instance>('create_instance', { name, mcVersion, loader }),
  updateInstance: (instance: Instance) => call<void>('update_instance', { instance }),
  deleteInstance: (id: string) => call<void>('delete_instance', { id }),
  listAccounts: () => call<AccountStore>('list_accounts'),
  addOfflineAccount: (name: string) => call<Account>('add_offline_account', { name }),
  microsoftLoginStart: () => call<DeviceLogin>('microsoft_login_start'),
  microsoftLoginWait: () => call<Account>('microsoft_login_wait'),
  microsoftLoginCancel: () => call<void>('microsoft_login_cancel'),
  removeAccount: (id: string) => call<void>('remove_account', { id }),
  setActiveAccount: (id: string) => call<void>('set_active_account', { id }),
  systemInfo: () => call<SystemInfo>('system_info'),
  effectiveLaunch: (instanceId: string) => call<EffectiveLaunch>('effective_launch', { instanceId }),
  recommendPerformance: () => call<Recommendation>('recommend_performance'),
  detectJava: () => call<JavaInstall[]>('detect_java'),
  listRuntimes: () => call<RuntimeEntry[]>('list_runtimes'),
  installRuntime: (flavor: string, major: number) => call<JavaInstall>('install_runtime', { flavor, major }),
  launch: (id: string, quickPlay?: QuickPlay) => call<void>('launch_instance', { id, quickPlay: quickPlay ?? null }),
  listServers: (instanceId?: string) => call<SavedServer[]>('list_servers', { instanceId: instanceId ?? null }),
  addServer: (instanceIds: string[], name: string, address: string) => call<void>('add_server', { instanceIds, name, address }),
  removeServer: (instanceIds: string[], address: string) => call<void>('remove_server', { instanceIds, address }),
  pingServer: (address: string) => call<ServerStatus>('ping_server', { address }),
  recentWorlds: (limit: number) => call<RecentWorld[]>('recent_worlds', { limit }),
  takeCliRequest: () => call<CliRequest | null>('take_cli_request'),
  createShortcut: (instanceId: string) => call<void>('create_shortcut', { instanceId }),
  exportMrpack: (instanceId: string, options: ExportOptions) => call<Exported>('export_mrpack', { instanceId, options }),
  shareCode: (instanceId: string) => call<ShareCode>('share_code', { instanceId }),
  importShareCode: (code: string) => call<Imported>('import_share_code', { code }),
  importModrinthLink: (link: string) => call<Imported>('import_modrinth_link', { link }),
  importMrpackFile: (path: string) => call<Imported>('import_mrpack_file', { path }),
  importMrpackBytes: (bytes: Uint8Array) => (inTauri ? invoke<Imported>('import_mrpack_bytes', bytes) : call<Imported>('import_mrpack_bytes')),
  revealFile: (path: string) => call<void>('reveal_file', { path }),
  stop: (id: string) => call<void>('stop_instance', { id }),
  running: () => call<string[]>('running_instances'),
  consoleHistory: (instanceId: string) => call<{ seq: number; text: string }[]>('console_history', { instanceId }),
  openConsole: (instanceId: string) => call<void>('open_console', { instanceId }),
  applyCrashFix: (instanceId: string) => call<void>('apply_crash_fix', { instanceId }),
  openCrashReport: (instanceId: string) => call<void>('open_crash_report', { instanceId }),
  checkUpdate: () => call<UpdateInfo | null>('check_update'),
  thumbnail: (url: string, width: number) => call<string>('thumbnail', { url, width }),
  fetchSkin: (name: string) => call<SkinInfo | null>('fetch_skin', { name }),
  wardrobeLoad: (accountId: string) => call<Wardrobe>('wardrobe_load', { accountId }),
  wardrobeSetSkin: (accountId: string, png: string, slim: boolean) => call<void>('wardrobe_set_skin', { accountId, png, slim }),
  wardrobeSetCape: (accountId: string, capeId: string | null) => call<void>('wardrobe_set_cape', { accountId, capeId }),
  getSettings: () => call<Settings>('get_settings'),
  saveSettings: (settings: Settings) => call<void>('save_settings', { settings }),
  storageInfo: () => call<StorageInfo>('storage_info'),
  deleteRuntime: (name: string) => call<void>('delete_runtime', { name }),
  openDataDir: () => call<void>('open_data_dir'),
  resetData: (scope: string[]) => call<void>('reset_data', { scope }),
  restartApp: () => call<void>('restart_app'),
  benchInfo: () => call<string | null>('bench_info'),
  benchDone: (report: string) => call<void>('bench_done', { report }),
  versionImages: () => call<Record<string, string>>('version_images'),
  setLanguage: (code: string) => call<void>('set_language', { code }),
  listWorlds: (instanceId: string) => call<World[]>('list_worlds', { instanceId }),
  openFolder: (instanceId: string, which: 'game' | 'mods' | 'saves') =>
    call<void>('open_folder', { instanceId, which }),
  openUrl: (url: string) => call<void>('open_url', { url }),
  duplicateInstance: (instanceId: string) => call<Instance>('duplicate_instance', { instanceId }),
}

export interface ProjectHit {
  projectId: string
  slug: string
  title: string
  description: string
  iconUrl: string | null
  bannerUrl: string | null
  downloads: number
  author: string
  categories: string[]
  latestMc: string | null
}

export interface SearchResult {
  hits: ProjectHit[]
  total: number
  pageSize: number
}

export interface ModFile {
  fileName: string
  size: number
  enabled: boolean
}

export interface ProjectMeta {
  id: string
  slug: string
  title: string
  description: string
  iconUrl: string | null
}

export interface VersionMeta {
  id: string
  number: string
}

export interface ContentEntry extends ModFile {
  project: ProjectMeta | null
  version: VersionMeta | null
  update: VersionMeta | null
}

export interface PackFile {
  path: string
  fileName: string
  kind: 'mod' | 'resourcepack' | 'shader' | 'other'
  size: number
  optional: boolean
  project: ProjectMeta | null
  version: VersionMeta | null
}

export interface PackDetails {
  version: string
  mcVersion: string | null
  loader: string | null
  files: PackFile[]
  overrides: number
}

export interface PackVersion {
  id: string
  number: string
  name: string
  kind: 'release' | 'beta' | 'alpha'
  mcVersions: string[]
  loaders: string[]
  date: string
  downloads: number
}

export interface ProjectPage {
  body: string
  gallery: { url: string; title: string | null }[]
  versions: PackVersion[]
}

export interface World {
  folder: string
  name: string | null
  size: number
  modifiedMs: number
}

export interface InstallProgress {
  stage: string
  done: number
  total: number
}

export type ContentKind = 'mods' | 'resourcepacks' | 'shaderpacks'
export type ProjectType = 'modpack' | 'mod' | 'resourcepack' | 'shader'

export interface Session {
  instanceId: string
  startMs: number
  secs: number
}

export interface NewsItem {
  title: string
  version: string
  kind: string
  date: string
  summary: string
  imageUrl: string | null
}

export const modrinth = {
  search: (args: {
    query: string
    projectType: ProjectType
    mcVersion?: string
    loaders: string[]
    categories: string[]
    sort: string
    offset: number
  }) => call<SearchResult>('modrinth_search', args),
  categories: (projectType: ProjectType) => call<string[]>('modrinth_categories', { projectType }),
  modpackDetails: (projectId: string, versionId?: string) => call<PackDetails>('modpack_details', { projectId, versionId }),
  projectPage: (projectId: string) => call<ProjectPage>('project_page', { projectId }),
  installModpack: (projectId: string, versionId?: string, mcVersion?: string) =>
    call<Instance>('install_modpack', { projectId, versionId, mcVersion }),
  installContent: (instanceId: string, projectId: string, kind: ContentKind) =>
    call<string[]>('install_content', { instanceId, projectId, kind }),
  listContent: (instanceId: string, kind: ContentKind) =>
    call<ModFile[]>('list_content', { instanceId, kind }),
  removeContent: (instanceId: string, kind: ContentKind, fileName: string) =>
    call<void>('remove_content', { instanceId, kind, fileName }),
  contentDetails: (instanceId: string, kind: ContentKind) =>
    call<ContentEntry[]>('content_details', { instanceId, kind }),
  updateContent: (instanceId: string, kind: ContentKind, fileName: string) =>
    call<string>('update_content', { instanceId, kind, fileName }),
  transferContent: (fromId: string, toId: string, kind: ContentKind, files: string[], removeSource: boolean) =>
    call<{ done: number; skipped: number }>('transfer_content', { fromId, toId, kind, files, removeSource }),
  importContent: (instanceId: string, kind: ContentKind, files: string[]) =>
    call<{ done: number; skipped: number }>('import_content', { instanceId, kind, files }),
  setModEnabled: (instanceId: string, fileName: string, enabled: boolean) =>
    call<void>('set_mod_enabled', { instanceId, fileName, enabled }),
}

export interface EffectiveLaunch {
  memoryMaxMb: number
  memoryMinMb: number
  flagProfile: FlagProfile
}

export interface SkinInfo {
  png: string
  slim: boolean
}

export interface Cape {
  id: string
  name: string
  active: boolean
  png: string | null
}

export interface Wardrobe {
  skin: SkinInfo | null
  capes: Cape[]
}

export interface Settings {
  language: string | null
  theme: 'system' | 'light' | 'dark'
  uiScale: 'auto' | '100' | '125' | '150'
  minimizeOnLaunch: boolean
  defaultMemoryMaxMb: number
  defaultMemoryMinMb: number
  defaultFlagProfile: FlagProfile
  lookupSkins: boolean
  reduceEffects: boolean
  smoothScroll: boolean
  checkUpdates: boolean
  discordPresence: boolean
  preferredJava: string | null
  defaultExtraJvmArgs: string
  tuning: Tuning
  autoOptimize: boolean
  liveConsole: boolean
}

export const defaultSettings = (): Settings => ({
  language: null,
  theme: 'system',
  uiScale: 'auto',
  minimizeOnLaunch: false,
  defaultMemoryMaxMb: 2048,
  defaultMemoryMinMb: 512,
  defaultFlagProfile: 'balanced',
  lookupSkins: true,
  reduceEffects: false,
  smoothScroll: true,
  checkUpdates: true,
  discordPresence: true,
  preferredJava: null,
  defaultExtraJvmArgs: '',
  tuning: { gc: 'auto', pauseMs: 0, gcThreads: 0, preTouch: false, disableExplicitGc: false, stringDedup: false },
  autoOptimize: false,
  liveConsole: false,
})

export interface RuntimeEntry {
  id: string
  flavor: string
  label: string
  major: number
  installed: boolean
  version: string | null
  path: string | null
}

export interface StorageInfo {
  root: string
  items: { key: string; bytes: number }[]
  runtimes: { name: string; bytes: number }[]
}

export const stats = {
  sessions: () => call<Session[]>('sessions'),
  installedVersions: () => call<string[]>('installed_versions'),
  news: () => call<NewsItem[]>('minecraft_news'),
}
