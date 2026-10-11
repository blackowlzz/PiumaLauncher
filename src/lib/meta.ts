import type { FlagProfile, Loader } from './api'
import { locale, t } from './i18n.svelte'

export const profiles = (): { id: FlagProfile; label: string; hint: string }[] => [
  { id: 'low', label: t('Low usage'), hint: t('Least RAM possible; gives memory back to the system while the game is idle.') },
  { id: 'balanced', label: t('Balanced'), hint: t("G1 with Aikar's flags: smooth and stable.") },
  { id: 'performance', label: t('Performance'), hint: t('Low-pause ZGC; uses more RAM (Java 17+).') },
  { id: 'none', label: t('None'), hint: t('No extra flags.') },
]

export const loaderNames: Record<Loader, string> = { vanilla: 'Vanilla', fabric: 'Fabric', quilt: 'Quilt' }

export const profileName = (id: FlagProfile) => profiles().find((p) => p.id === id)?.label ?? id

export function formatBytes(n: number): string {
  if (n >= 1 << 30) return `${(n / (1 << 30)).toFixed(1)} GB`
  if (n >= 1 << 20) return `${(n / (1 << 20)).toFixed(1)} MB`
  return `${Math.max(1, Math.round(n / 1024))} KB`
}

export function formatDuration(secs: number): string {
  if (secs < 60) return '0 min'
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  return h ? `${h} h${m ? ` ${m} min` : ''}` : `${m} min`
}

export function relativeTime(ms: number): string {
  const min = Math.round((Date.now() - ms) / 60000)
  if (min < 1) return t('now')
  if (min < 60) return t('{n} min ago', { n: min })
  const h = Math.round(min / 60)
  if (h < 24) return t('{n} h ago', { n: h })
  const d = Math.round(h / 24)
  return d < 30 ? t('{n} d ago', { n: d }) : new Date(ms).toLocaleDateString(locale(), { day: 'numeric', month: 'short' })
}
