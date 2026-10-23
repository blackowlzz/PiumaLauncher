export function startOfDay(now: number): number {
  const d = new Date(now)
  d.setHours(0, 0, 0, 0)
  return d.getTime()
}

export function secondsToday(sessions: { startMs: number; secs: number }[], running: number[], now: number): number {
  const from = startOfDay(now)
  let secs = 0
  for (const s of sessions) {
    const end = s.startMs + s.secs * 1000
    if (end > from) secs += (end - Math.max(s.startMs, from)) / 1000
  }
  for (const since of running) secs += Math.max(0, now - Math.max(since, from)) / 1000
  return secs
}

export function breakStep(since: number, now: number, everyMinutes: number): number {
  return everyMinutes > 0 ? Math.floor((now - since) / 60000 / everyMinutes) : 0
}
