import type { Instance, ProjectHit } from './api'

const base: Omit<Instance, 'id' | 'name' | 'mcVersion' | 'loader' | 'loaderVersion'> = {
  launchVersion: null,
  memoryMaxMb: 2048,
  memoryMinMb: 512,
  flagProfile: 'low',
  javaPath: null,
  extraJvmArgs: '',
  iconUrl: null,
  bannerUrl: null,
}

const instances: Instance[] = [
  { ...base, id: 'survival-a1b2', name: 'Cobblemon', mcVersion: '1.21.1', loader: 'fabric', loaderVersion: '0.16.9', memoryMaxMb: 4096, iconUrl: 'https://cdn.modrinth.com/data/5FFgwNNP/e7f9ee2e9d361623847853fe2ddce42f519ee64f.png', bannerUrl: 'https://cdn.modrinth.com/data/5FFgwNNP/images/43765359395bb8216a238d18c02a045106d7bad4_350.webp' },
  { ...base, id: 'fabulously-c3d4', name: 'Fabulously Optimized', mcVersion: '26.2', loader: 'fabric', loaderVersion: '0.19.5', flagProfile: 'balanced', memoryMaxMb: 3072, iconUrl: 'https://cdn.modrinth.com/data/1KVo5zza/d8152911f8fd5d7e9a8c499fe89045af81fe816e_96.webp', bannerUrl: 'https://cdn.modrinth.com/data/1KVo5zza/images/38f70f307f1a0b17d8ed4526d1636317e8c12d1d_350.webp' },
  { ...base, id: 'vanilla-e5f6', name: 'Vanilla', mcVersion: '1.20.6', loader: 'vanilla', loaderVersion: null },
  { ...base, id: 'redstone-0789', name: 'Redstone Lab', mcVersion: '1.21.4', loader: 'quilt', loaderVersion: '0.27.1' },
]

async function search(a: Record<string, unknown>) {
  const facets: string[][] = [[`project_type:${a.projectType}`]]
  if (a.mcVersion) facets.push([`versions:${a.mcVersion}`])
  const loaders = (a.loaders as string[]) ?? []
  if (loaders.length) facets.push(loaders.map((l) => `categories:${l}`))
  for (const c of (a.categories as string[]) ?? []) facets.push([`categories:${c}`])
  const url = new URL('https://api.modrinth.com/v2/search')
  url.searchParams.set('query', String(a.query ?? ''))
  url.searchParams.set('facets', JSON.stringify(facets))
  url.searchParams.set('index', String(a.sort ?? 'downloads'))
  url.searchParams.set('limit', '20')
  url.searchParams.set('offset', String(a.offset ?? 0))
  const raw = await (await fetch(url)).json()
  const hits: ProjectHit[] = raw.hits.map((h: Record<string, any>) => ({
    projectId: h.project_id,
    slug: h.slug,
    title: h.title,
    description: h.description,
    iconUrl: h.icon_url ?? null,
    bannerUrl: h.featured_gallery ?? h.gallery?.[0] ?? null,
    downloads: h.downloads,
    author: h.author,
    categories: h.categories,
    latestMc: h.versions?.at(-1) ?? null,
  }))
  return { hits, total: raw.total_hits, pageSize: 20 }
}

const empty = typeof location !== 'undefined' && new URLSearchParams(location.search).has('empty')
const fresh = typeof location !== 'undefined' && new URLSearchParams(location.search).has('fresh')
const accounts: { id: string; name: string; uuid: string; kind: string }[] = fresh ? [] : [{ id: 'a1', name: 'Steve', uuid: '8667ba71b85a4004af54457a9734eed7', kind: 'offline' }]
let activeAccount: string | null = accounts[0]?.id ?? null

export async function mock(cmd: string, args: Record<string, unknown>): Promise<unknown> {
  switch (cmd) {
    case 'list_instances':
      return empty ? [] : instances
    case 'list_accounts':
      return { accounts, active: activeAccount }
    case 'add_offline_account': {
      const a = { id: `m${accounts.length + 1}`, name: String(args.name), uuid: '0'.repeat(32), kind: 'offline' }
      accounts.push(a)
      activeAccount ??= a.id
      return a
    }
    case 'microsoft_login_start':
      return { userCode: 'AB12CD34', verificationUri: 'https://www.microsoft.com/link', expiresIn: 900 }
    case 'microsoft_login_wait': {
      await new Promise((r) => setTimeout(r, 4000))
      const a = { id: `m${accounts.length + 1}`, name: 'Alex', uuid: '6ab4317889fd490597f60f67d9d76fd9', kind: 'microsoft' }
      accounts.push(a)
      activeAccount = a.id
      return a
    }
    case 'microsoft_login_cancel':
      return null
    case 'check_update':
      return new URLSearchParams(location.search).has('update') ? { version: '9.9.9', url: 'https://github.com/blackowlzz/PiumaLauncher/releases' } : null
    case 'thumbnail':
      return String(args.url)
    case 'list_mc_versions':
      return ['1.21.4', '1.21.1', '1.20.6', '1.20.1'].map((id) => ({ id, kind: 'release', releaseTime: '' }))
    case 'modrinth_search':
      return search(args)
    case 'list_content':
      return args.kind === 'mods'
        ? [
            { fileName: 'sodium-fabric-0.6.13.jar', size: 1_200_000, enabled: true },
            { fileName: 'lithium-fabric-0.14.3.jar', size: 840_000, enabled: true },
            { fileName: 'iris-1.8.8.jar.disabled', size: 2_300_000, enabled: false },
          ]
        : [{ fileName: 'Faithful-64x.zip', size: 18_000_000, enabled: true }]
    case 'content_details': {
      const files = (await mock('list_content', args)) as { fileName: string; size: number; enabled: boolean }[]
      if (args.kind !== 'mods') return files.map((f) => ({ ...f, project: null, version: null, update: null }))
      const ids = ['AANobbMI', 'gvQqBUqZ', 'YL57xq9U']
      let projects: Record<string, any>[] = []
      try {
        projects = await (await fetch(`https://api.modrinth.com/v2/projects?ids=${encodeURIComponent(JSON.stringify(ids))}`)).json()
      } catch {  }
      return files.map((f, i) => {
        const p = projects.find((x) => x.id === ids[i])
        return {
          ...f,
          project: p ? { id: p.id, slug: p.slug, title: p.title, description: p.description, iconUrl: p.icon_url } : null,
          version: p ? { id: `v${i}`, number: ['mc1.21.1-0.6.13-fabric', '0.14.3+mc1.21.1', '1.8.8+1.21'][i] } : null,
          update: p && i === 1 ? { id: 'new', number: '0.15.0+mc1.21.1' } : null,
        }
      })
    }
    case 'modpack_details': {
      const ids = ['AANobbMI', 'gvQqBUqZ', 'YL57xq9U', 'P7dR8mSH', 'mOgUt4GM', 'hvFnDODi']
      let projects: Record<string, any>[] = []
      try {
        projects = await (await fetch(`https://api.modrinth.com/v2/projects?ids=${encodeURIComponent(JSON.stringify(ids))}`)).json()
      } catch {  }
      await new Promise((r) => setTimeout(r, 700))
      const file = (p: any, i: number, kind: string) => ({
        path: `${kind === 'mod' ? 'mods' : 'resourcepacks'}/${p.slug}.jar`, fileName: `${p.slug}-${i + 1}.0.${i}.jar`, kind, size: 400_000 * (i + 2), optional: i === 3,
        project: { id: p.id, slug: p.slug, title: p.title, description: p.description, iconUrl: p.icon_url }, version: { id: `v${i}`, number: `0.${i + 4}.${i}+1.21` },
      })
      return {
        version: '14.1.1', mcVersion: '1.21.1', loader: 'fabric', overrides: 57,
        files: [
          ...Array.from({ length: 6 }, (_, r) => projects.map((p, i) => ({ ...file(p, i + r, 'mod'), path: `mods/${p.slug}-${r}.jar` }))).flat(),
          { path: 'mods/custom-thing.jar', fileName: 'custom-thing.jar', kind: 'mod', size: 90_000, optional: false, project: null, version: null },
          { path: 'resourcepacks/Faithful.zip', fileName: 'Faithful.zip', kind: 'resourcepack', size: 18_000_000, optional: false, project: null, version: null },
        ],
      }
    }
    case 'project_page': {
      let p: any = {}
      try {
        p = await (await fetch(`https://api.modrinth.com/v2/project/${encodeURIComponent(String(args.projectId))}`)).json()
      } catch {  }
      return {
        body: p.body ?? '## About\n\nA **modpack** with [a link](https://modrinth.com) and a list:\n\n- one\n- two',
        gallery: (p.gallery ?? []).map((g: any) => ({ url: g.url, title: g.title })).slice(0, 6),
        versions: [['14.1.1', 'release', '1.21.1', 'fabric', 2], ['14.1.0', 'release', '1.21.1', 'fabric', 9], ['14.2.0-beta.1', 'beta', '1.21.4', 'fabric', 20], ['13.9.2', 'release', '1.20.1', 'fabric', 60]].map(([number, kind, mc, loader, d], i) => ({
          id: `ver${i}`, number, name: `Version ${number}`, kind, mcVersions: [mc], loaders: [loader], date: new Date(Date.now() - (d as number) * 86_400_000).toISOString(), downloads: 120000 - i * 20000,
        })),
      }
    }
    case 'open_url':
      window.open(String(args.url), '_blank')
      return null
    case 'update_content':
      return String(args.fileName)
    case 'modrinth_categories': {
      const all = await (await fetch('https://api.modrinth.com/v2/tag/category')).json()
      return all.filter((c: any) => c.project_type === args.projectType).map((c: any) => c.name).sort()
    }
    case 'sessions': {
      const day = 86_400_000
      return [[0, 5400, 'survival-a1b2'], [1, 3600, 'fabulously-c3d4'], [2, 9000, 'survival-a1b2'], [4, 2400, 'vanilla-e5f6'], [5, 7200, 'survival-a1b2'], [6, 1200, 'redstone-0789']].map(
        ([d, secs, id]) => ({ instanceId: id, startMs: Date.now() - (d as number) * day - 3_600_000, secs }),
      )
    }
    case 'installed_versions':
      return ['1.21.1', '26.2']
    case 'fetch_skin': {
      try {
        const blob = await (await fetch(`https://minotar.net/skin/${args.name}.png`)).blob()
        const png = await new Promise<string>((res) => {
          const r = new FileReader()
          r.onload = () => res(String(r.result))
          r.readAsDataURL(blob)
        })
        return { png, slim: false }
      } catch {
        return null
      }
    }
    case 'wardrobe_load': {
      const { makeSkin } = await import('./skin')
      const cape = (a: string, b: string) => {
        const cv = document.createElement('canvas')
        cv.width = 64
        cv.height = 32
        const c = cv.getContext('2d')!
        c.fillStyle = a
        c.fillRect(0, 0, 64, 32)
        c.fillStyle = b
        c.fillRect(1, 1, 10, 16)
        c.fillRect(12, 1, 10, 16)
        return cv.toDataURL('image/png')
      }
      return {
        skin: { png: makeSkin('Piuma'), slim: false },
        capes: [
          { id: 'a1', name: 'Migrator', active: true, png: cape('#7a1f1f', '#c0392b') },
          { id: 'b2', name: 'Vanilla', active: false, png: cape('#1f3a7a', '#3a7bd5') },
        ],
      }
    }
    case 'wardrobe_set_skin':
    case 'wardrobe_set_cape':
      return null
    case 'get_settings': {
      const forced = new URLSearchParams(location.search).get('lang')
      let saved: Record<string, unknown> = {}
      let lang: string | null = null
      try { saved = JSON.parse(localStorage.getItem('mock-settings') ?? '{}'); lang = localStorage.getItem('mock-lang') } catch {  }
      return {
        theme: 'system', uiScale: 'auto', minimizeOnLaunch: false, checkUpdates: true, defaultMemoryMaxMb: 2048,
        defaultMemoryMinMb: 512, defaultFlagProfile: 'balanced', lookupSkins: true, reduceEffects: false, smoothScroll: true, discordPresence: true, defaultExtraJvmArgs: '', tuning: { gc: 'auto', pauseMs: 0, gcThreads: 0, preTouch: false, disableExplicitGc: false, stringDedup: false }, ...saved,
        ...{ language: forced ?? (fresh ? null : lang) },
      }
    }
    case 'console_history':
      return []
    case 'open_console':
    case 'apply_crash_fix':
    case 'open_crash_report':
      return null
    case 'save_settings':
      try { localStorage.setItem('mock-settings', JSON.stringify(args.settings)) } catch {  }
      return null
    case 'storage_info':
      return {
        root: '/home/utente/.local/share/PiumaLauncher',
        items: [
          { key: 'instances', bytes: 1_420_000_000 }, { key: 'versions', bytes: 61_000_000 }, { key: 'libraries', bytes: 124_000_000 },
          { key: 'assets', bytes: 640_000_000 }, { key: 'runtimes', bytes: 710_000_000 }, { key: 'skins', bytes: 38_000 }, { key: 'other', bytes: 120_000 },
        ],
        runtimes: [{ name: 'graalvm-21', bytes: 340_000_000 }, { name: 'graalvm-25', bytes: 370_000_000 }],
      }
    case 'list_runtimes': {
      let have: string[] = []
      try { have = JSON.parse(localStorage.getItem('mock-runtimes') ?? '["graalvm-21"]') } catch {  }
      return [['temurin', 'Temurin', 8], ['temurin', 'Temurin', 17], ['temurin', 'Temurin', 21], ['temurin', 'Temurin', 25], ['graalvm', 'GraalVM', 17], ['graalvm', 'GraalVM', 21], ['graalvm', 'GraalVM', 25]].map(([flavor, label, major]) => {
        const id = `${flavor}-${major}`
        const installed = have.includes(id)
        return { id, flavor, label, major, installed, version: installed ? `${major}.0.1` : null, path: installed ? `/runtimes/${id}/bin/java` : null }
      })
    }
    case 'install_runtime': {
      let have: string[] = []
      try { have = JSON.parse(localStorage.getItem('mock-runtimes') ?? '["graalvm-21"]') } catch {  }
      await new Promise((r) => setTimeout(r, 900))
      try { localStorage.setItem('mock-runtimes', JSON.stringify([...have, `${args.flavor}-${args.major}`])) } catch {  }
      return { path: '', major: args.major, version: `${args.major}.0.1`, isGraal: args.flavor === 'graalvm' }
    }
    case 'delete_runtime': {
      try { localStorage.setItem('mock-runtimes', JSON.stringify((JSON.parse(localStorage.getItem('mock-runtimes') ?? '["graalvm-21"]') as string[]).filter((x) => x !== args.name))) } catch {  }
      return null
    }
    case 'reset_data':
    case 'open_data_dir':
      return null
    case 'restart_app':
      location.reload()
      return null
    case 'version_images': {
      const feed = await (await fetch('https://launchercontent.mojang.com/v2/javaPatchNotes.json')).json()
      return Object.fromEntries(
        feed.entries.filter((e: Record<string, any>) => e.version && e.image?.url?.startsWith('/')).map((e: Record<string, any>) => [e.version, `https://launchercontent.mojang.com${e.image.url}`]),
      )
    }
    case 'set_language':
      try { localStorage.setItem('mock-lang', String(args.code)) } catch {  }
      return null
    case 'minecraft_news': {
      const feed = await (await fetch('https://launchercontent.mojang.com/v2/javaPatchNotes.json')).json()
      return feed.entries.slice(0, 8).map((e: Record<string, any>) => ({
        title: e.title, version: e.version, kind: e.type, date: e.date, summary: e.shortText,
        imageUrl: e.image?.url?.startsWith('/') ? `https://launchercontent.mojang.com${e.image.url}` : null,
      }))
    }
    case 'minecraft_news_static':
      return [
        { title: 'Minecraft 26.4 Snapshot 3', version: '26.4-snapshot-3', kind: 'snapshot', date: '2026-10-06T12:54:26Z', summary: 'Brr! Does it feel colder all of a sudden? Ice caves filled with glittering icicles.', imageUrl: null },
        { title: 'Minecraft 26.3', version: '26.3', kind: 'release', date: '2026-09-23T10:00:00Z', summary: 'The latest game drop is here.', imageUrl: null },
        { title: 'Minecraft 26.3 Release Candidate 1', version: '26.3-rc-1', kind: 'snapshot', date: '2026-09-18T10:00:00Z', summary: 'Final fixes before release.', imageUrl: null },
      ]
    case 'list_worlds':
      return [{ folder: 'Nuovo mondo', name: 'Nuovo mondo', size: 48_000_000, modifiedMs: Date.now() - 3_600_000 }]
    case 'list_servers':
      return [
        { name: 'Hypixel', address: 'mc.hypixel.net', icon: null, instances: ['survival-a1b2', 'vanilla-e5f6'] },
        { name: 'Server di casa', address: '192.168.1.20', icon: null, instances: ['survival-a1b2'] },
      ].filter((s) => !args.instanceId || s.instances.includes(String(args.instanceId)))
    case 'take_cli_request':
      return null
    case 'export_mrpack':
      await new Promise((r) => setTimeout(r, 1400))
      return { path: '/home/steve/Scaricati/Cobblemon.mrpack', linked: 142, bundled: 3 }
    case 'share_code':
      await new Promise((r) => setTimeout(r, 1100))
      return { code: 'PIUMA1-DckxCoAwEAXRq8jWGoyFoJ2llYVeIJqELGQVVrER755fvpmPThppTU5DtYX7oZoEwZquNRbIQHS78tHky_mgaC8abm8GIAKycPIzy0R_AQ', included: 142, excluded: ['homemade-1.0.jar', 'secret-tweaks.jar'] }
    case 'import_share_code':
    case 'import_modrinth_link':
    case 'import_mrpack_file':
    case 'import_mrpack_bytes':
      await new Promise((r) => setTimeout(r, 1500))
      return { instance: instances[0], skipped: 0 }
    case 'reveal_file':
      return null
    case 'create_shortcut':
      await new Promise((r) => setTimeout(r, 500))
      return null
    case 'add_server':
    case 'remove_server':
      return null
    case 'ping_server':
      await new Promise((r) => setTimeout(r, 400 + Math.random() * 900))
      if (String(args.address).startsWith('192.')) throw 'The server did not answer'
      return { motd: '§r§a§lHypixel Network §c[1.8-1.21]\n§r§6§lHALLOWEEN EVENT', version: 'Requires MC 1.8 / 1.21', protocol: 47, online: 31_204, max: 200_000, latencyMs: 48, favicon: null, sample: [] }
    case 'recent_worlds':
      return [
        { instanceId: 'survival-a1b2', folder: 'Nuovo mondo', name: 'Nuovo mondo', modifiedMs: Date.now() - 3_600_000, mode: 0, hardcore: false, icon: null },
        { instanceId: 'redstone-0789', folder: 'Lab', name: 'Laboratorio', modifiedMs: Date.now() - 90_000_000, mode: 1, hardcore: false, icon: null },
      ]
    case 'detect_java':
      return [{ path: '/usr/lib/jvm/graalvm/bin/java', major: 25, version: '25.0.4', isGraal: true }]
    case 'transfer_content':
      return { done: (args.files as string[]).length, skipped: 0 }
    case 'system_info':
      return { totalMemoryMb: 16384, cpuCores: 8 }
    case 'recommend_performance':
      return { tier: 'strong', memoryMaxMb: 6144, memoryMinMb: 6144, flagProfile: 'performance', tuning: { gc: 'auto', pauseMs: 0, gcThreads: 4, preTouch: false, disableExplicitGc: true, stringDedup: false }, reasons: ['A powerful computer: a large heap and a low-pause collector for the smoothest play.'] }
    case 'effective_launch':
      return { memoryMaxMb: 6144, memoryMinMb: 6144, flagProfile: 'performance' }
    case 'running_instances':
      return []
    default:
      return null
  }
}
