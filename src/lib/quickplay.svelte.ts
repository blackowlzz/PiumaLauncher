import { api, type Instance, type ServerStatus } from './api'

export type Ping =
  | { state: 'pending' }
  | { state: 'online'; status: ServerStatus; at: number }
  | { state: 'offline'; at: number }

const FRESH_MS = 60_000

export const pings = $state<Record<string, Ping>>({})

export const serverKey = (address: string) => address.trim().toLowerCase().replace(/:25565$/, '')

export function ping(address: string, force = false) {
  const k = serverKey(address)
  const cur = pings[k]
  if (cur && (cur.state === 'pending' || (!force && Date.now() - cur.at < FRESH_MS))) return
  pings[k] = { state: 'pending' }
  api
    .pingServer(address)
    .then((status) => (pings[k] = { state: 'online', status, at: Date.now() }))
    .catch(() => (pings[k] = { state: 'offline', at: Date.now() }))
}

export const pingOf = (address: string): Ping | undefined => pings[serverKey(address)]

export function signal(ms: number | null): number {
  if (ms === null) return 0
  return ms < 80 ? 4 : ms < 150 ? 3 : ms < 300 ? 2 : 1
}

type Parts = number[]
const parse = (v: string): Parts => v.split('.').map((n) => Number(n))
function compare(a: Parts, b: Parts): number {
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    const d = (a[i] ?? 0) - (b[i] ?? 0)
    if (d) return d
  }
  return 0
}

export function versionMatch(mc: string, server: string | undefined): 'exact' | 'supported' | null {
  if (!server || !/^\d+\.\d+(\.\d+)?$/.test(mc)) return null
  const found: string[] = server.match(/\d+\.\d+(?:\.\d+)?/g) ?? []
  if (!found.length) return null
  if (found.includes(mc)) return 'exact'
  const family = mc.split('.').slice(0, 2).join('.')
  if (found.some((v) => v.split('.').slice(0, 2).join('.') === family)) return 'supported'
  if (found.length > 1) {
    const sorted = found.map(parse).sort(compare)
    const v = parse(mc)
    if (compare(v, sorted[0]) >= 0 && compare(v, sorted[sorted.length - 1]) <= 0) return 'supported'
  }
  return null
}

export function opensWorlds(mc: string): boolean {
  if (/^(rd-|inf-|[abc]\d)/.test(mc)) return false
  const release = mc.match(/^1\.(\d+)/)
  if (release) return Number(release[1]) >= 20
  const snapshot = mc.match(/^(\d\d)w(\d\d)/)
  if (snapshot) return Number(snapshot[1]) > 23 || (Number(snapshot[1]) === 23 && Number(snapshot[2]) >= 14)
  const yearly = mc.match(/^(\d\d)\.\d/)
  if (yearly) return Number(yearly[1]) >= 24
  return true
}

export function rankInstances(instances: Instance[], saved: string[], server: string | undefined) {
  const order = { exact: 0, supported: 1 } as const
  return instances
    .map((inst, i) => {
      const match = versionMatch(inst.mcVersion, server)
      const isSaved = saved.includes(inst.id)
      const score = (match ? order[match] : 2) * 2 + (isSaved ? 0 : 1)
      return { inst, match, saved: isSaved, score, i }
    })
    .sort((a, b) => a.score - b.score || a.i - b.i)
}
