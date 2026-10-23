import { api } from './api'
import { errorKey } from './errors'

export const LANGUAGES = [
  { code: 'en', name: 'English' },
  { code: 'it', name: 'Italiano' },
  { code: 'es', name: 'Español' },
  { code: 'fr', name: 'Français' },
  { code: 'de', name: 'Deutsch' },
  { code: 'pt', name: 'Português' },
  { code: 'sq', name: 'Shqip' },
  { code: 'hu', name: 'Magyar' },
  { code: 'ru', name: 'Русский' },
  { code: 'bg', name: 'Български' },
  { code: 'uk', name: 'Українська' },
  { code: 'pl', name: 'Polski' },
  { code: 'ro', name: 'Română' },
  { code: 'sr', name: 'Srpski' },
  { code: 'el', name: 'Ελληνικά' },
  { code: 'tr', name: 'Türkçe' },
] as const

type Dict = Record<string, string>
const sources = import.meta.glob<Record<string, Dict>>('./lang/*.ts')
const dicts: Record<string, Dict> = { en: {} }

async function ensureDict(code: string): Promise<boolean> {
  if (dicts[code]) return true
  const load = sources[`./lang/${code}.ts`]
  if (!load) return false
  const dict = (await load())[code]
  if (!dict) return false
  dicts[code] = dict
  return true
}

export const i18n = $state({ lang: 'en', ready: false, chosen: false })

export const locale = () => i18n.lang

export function t(text: string, params?: Record<string, string | number>): string {
  const raw = dicts[i18n.lang]?.[text] ?? text
  return params ? raw.replace(/\{(\w+)\}/g, (_, k) => String(params[k] ?? `{${k}}`)) : raw
}

export function translateError(message: unknown): string {
  const text = String(message)
  if (dicts[i18n.lang]?.[text]) return t(text)
  const { key, params } = errorKey(text)
  return t(key, params)
}

export function stageText(stage: string): string {
  if (dicts[i18n.lang]?.[stage]) return t(stage)
  const m = stage.match(/^(Downloading|Installing) (.+)$/)
  return m ? t(`${m[1]} {name}`, { name: m[2] }) : stage
}

export async function loadLanguage(language?: string | null) {
  try {
    const code = language === undefined ? (await api.getSettings()).language : language
    if (code && (await ensureDict(code))) {
      i18n.lang = code
      i18n.chosen = true
    }
  } finally {
    i18n.ready = true
  }
}

export async function setLanguage(code: string) {
  if (!(await ensureDict(code).catch(() => false))) return
  i18n.lang = code
  i18n.chosen = true
  document.documentElement.lang = code
  await api.setLanguage(code).catch(() => {})
}

export const suggestedLanguage = () => {
  const sys = (typeof navigator !== 'undefined' ? navigator.language : 'en').slice(0, 2)
  return LANGUAGES.some((l) => l.code === sys) ? sys : 'en'
}
