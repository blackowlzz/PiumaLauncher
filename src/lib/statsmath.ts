export interface SessionLike {
  instanceId: string
  startMs: number
  secs: number
}

export function dayStart(ms: number): number {
  const d = new Date(ms)
  d.setHours(0, 0, 0, 0)
  return d.getTime()
}

function shiftDays(ms: number, days: number): number {
  const d = new Date(ms)
  d.setDate(d.getDate() + days)
  return dayStart(d.getTime())
}

export function perDay(sessions: SessionLike[], days: number, now: number): { day: number; secs: number }[] {
  const today = dayStart(now)
  const out = Array.from({ length: days }, (_, i) => ({ day: shiftDays(today, i - days + 1), secs: 0 }))
  const index = new Map(out.map((d, i) => [d.day, i]))
  for (const s of sessions) {
    const i = index.get(dayStart(s.startMs))
    if (i !== undefined) out[i].secs += s.secs
  }
  return out
}

export function streaks(sessions: SessionLike[], now: number): { current: number; longest: number } {
  const days = new Set(sessions.filter((s) => s.secs >= 60).map((s) => dayStart(s.startMs)))
  if (!days.size) return { current: 0, longest: 0 }
  const sorted = [...days].sort((a, b) => a - b)
  let longest = 1
  let run = 1
  for (let i = 1; i < sorted.length; i++) {
    run = shiftDays(sorted[i - 1], 1) === sorted[i] ? run + 1 : 1
    longest = Math.max(longest, run)
  }
  const today = dayStart(now)
  let cursor = days.has(today) ? today : shiftDays(today, -1)
  let current = 0
  while (days.has(cursor)) {
    current++
    cursor = shiftDays(cursor, -1)
  }
  return { current, longest }
}

export function totals(sessions: SessionLike[], now: number) {
  const week = shiftDays(dayStart(now), -6)
  const month = shiftDays(dayStart(now), -29)
  let all = 0
  let last7 = 0
  let last30 = 0
  for (const s of sessions) {
    all += s.secs
    if (s.startMs >= week) last7 += s.secs
    if (s.startMs >= month) last30 += s.secs
  }
  return { all, week: last7, month: last30, count: sessions.length, average: sessions.length ? all / sessions.length : 0 }
}

export function perInstance(sessions: SessionLike[]): Map<string, { secs: number; count: number; last: number }> {
  const out = new Map<string, { secs: number; count: number; last: number }>()
  for (const s of sessions) {
    const cur = out.get(s.instanceId) ?? { secs: 0, count: 0, last: 0 }
    cur.secs += s.secs
    cur.count++
    cur.last = Math.max(cur.last, s.startMs)
    out.set(s.instanceId, cur)
  }
  return out
}

export function byWeekday(sessions: SessionLike[]): number[] {
  const out = Array(7).fill(0)
  for (const s of sessions) out[(new Date(s.startMs).getDay() + 6) % 7] += s.secs
  return out
}

export function byHour(sessions: SessionLike[]): number[] {
  const out = Array(24).fill(0)
  for (const s of sessions) {
    let at = s.startMs
    const end = s.startMs + Math.min(s.secs, 86_400) * 1000
    while (at < end) {
      const d = new Date(at)
      const next = new Date(d.getFullYear(), d.getMonth(), d.getDate(), d.getHours() + 1).getTime()
      const until = Math.min(next, end)
      out[d.getHours()] += (until - at) / 1000
      at = until
    }
  }
  return out
}

export function weekCompare(sessions: SessionLike[], now: number): { current: number; previous: number; change: number | null } {
  const thisStart = shiftDays(dayStart(now), -6)
  const prevStart = shiftDays(dayStart(now), -13)
  let current = 0
  let previous = 0
  for (const s of sessions) {
    if (s.startMs >= thisStart) current += s.secs
    else if (s.startMs >= prevStart) previous += s.secs
  }
  return { current, previous, change: previous > 0 ? Math.round(((current - previous) / previous) * 100) : null }
}
