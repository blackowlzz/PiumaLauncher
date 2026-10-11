import { api } from './api'
import { de } from './lang/de'
import { es } from './lang/es'
import { fr } from './lang/fr'
import { it } from './lang/it'
import { pt } from './lang/pt'

export const LANGUAGES = [
  { code: 'en', name: 'English' },
  { code: 'it', name: 'Italiano' },
  { code: 'es', name: 'Español' },
  { code: 'fr', name: 'Français' },
  { code: 'de', name: 'Deutsch' },
  { code: 'pt', name: 'Português' },
] as const

type Dict = Record<string, string>
const dicts: Record<string, Dict> = { en: {}, it, es, fr, de, pt }

export const i18n = $state({ lang: 'en', ready: false, chosen: false })

export const locale = () => i18n.lang

export function t(text: string, params?: Record<string, string | number>): string {
  const raw = dicts[i18n.lang]?.[text] ?? text
  return params ? raw.replace(/\{(\w+)\}/g, (_, k) => String(params[k] ?? `{${k}}`)) : raw
}

export function stageText(stage: string): string {
  if (dicts[i18n.lang]?.[stage]) return t(stage)
  const m = stage.match(/^(Downloading|Installing) (.+)$/)
  return m ? t(`${m[1]} {name}`, { name: m[2] }) : stage
}

export async function loadLanguage() {
  try {
    const { language } = await api.getSettings()
    if (language && dicts[language]) {
      i18n.lang = language
      i18n.chosen = true
    }
  } finally {
    i18n.ready = true
  }
}

export async function setLanguage(code: string) {
  if (!dicts[code]) return
  i18n.lang = code
  i18n.chosen = true
  document.documentElement.lang = code
  await api.setLanguage(code).catch(() => {})
}

export const suggestedLanguage = () => {
  const sys = (typeof navigator !== 'undefined' ? navigator.language : 'en').slice(0, 2)
  return LANGUAGES.some((l) => l.code === sys) ? sys : 'en'
}
