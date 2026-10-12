export type Palette = { bg: string; surface: string; text: string; accent: string }
export type ColorKey = keyof Palette

export const COLOR_KEYS: ColorKey[] = ['bg', 'surface', 'text', 'accent']

export const PRESETS: { id: string; name: string; palette: Palette | null }[] = [
  { id: 'mono', name: 'Monochrome', palette: null },
  { id: 'paper', name: 'Paper', palette: { bg: '#fcfcfc', surface: '#ffffff', text: '#0a0a0a', accent: '#0a0a0a' } },
  { id: 'midnight', name: 'Midnight', palette: { bg: '#05070f', surface: '#0c1020', text: '#e8ecff', accent: '#7c8cff' } },
  { id: 'ocean', name: 'Ocean', palette: { bg: '#04100f', surface: '#0a1a19', text: '#e6fbf8', accent: '#2dd4bf' } },
  { id: 'forest', name: 'Forest', palette: { bg: '#060d07', surface: '#0e1a10', text: '#eaf7ea', accent: '#4ade80' } },
  { id: 'crimson', name: 'Crimson', palette: { bg: '#0b0506', surface: '#170b0d', text: '#fdecee', accent: '#ef4452' } },
  { id: 'amber', name: 'Amber', palette: { bg: '#0b0803', surface: '#171108', text: '#fdf3e0', accent: '#f59e0b' } },
  { id: 'violet', name: 'Violet', palette: { bg: '#09050f', surface: '#140d1f', text: '#f3eaff', accent: '#a855f7' } },
]

const KEY = 'piuma-theme'

type Saved = { preset: string; custom: Palette | null }

const load = (): Saved => {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? 'null')
    if (raw && typeof raw.preset === 'string') return { preset: raw.preset, custom: raw.custom && COLOR_KEYS.every((k) => /^#[0-9a-f]{6}$/i.test(raw.custom[k])) ? raw.custom : null }
  } catch {}
  return { preset: 'mono', custom: null }
}

export const themes = $state<Saved>(load())

const rgb = (hex: string): [number, number, number] => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) as [number, number, number]
const hex = (c: number[]) => '#' + c.map((v) => Math.round(Math.min(255, Math.max(0, v))).toString(16).padStart(2, '0')).join('')
const mix = (a: string, b: string, w: number) => {
  const x = rgb(a)
  const y = rgb(b)
  return hex(x.map((v, i) => v + (y[i] - v) * w))
}
const luma = (h: string) => {
  const [r, g, b] = rgb(h)
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255
}

export const onAccent = (h: string) => (luma(h) > 0.58 ? '#000000' : '#ffffff')

export function activePalette(): Palette | null {
  if (themes.custom) return themes.custom
  return PRESETS.find((p) => p.id === themes.preset)?.palette ?? null
}

const VARS = ['--bg', '--surface', '--surface-2', '--line', '--line-strong', '--text', '--muted', '--faint', '--inv-bg', '--inv-text', '--glow']

export function applyTheme() {
  const root = document.documentElement
  const p = activePalette()
  if (!p) {
    for (const v of VARS) root.style.removeProperty(v)
    root.removeAttribute('data-custom')
    return
  }
  const set: Record<string, string> = {
    '--bg': p.bg,
    '--surface': p.surface,
    '--surface-2': mix(p.surface, p.text, 0.07),
    '--line': mix(p.bg, p.text, 0.11),
    '--line-strong': mix(p.bg, p.text, 0.2),
    '--text': p.text,
    '--muted': mix(p.bg, p.text, 0.62),
    '--faint': mix(p.bg, p.text, 0.36),
    '--inv-bg': p.accent,
    '--inv-text': onAccent(p.accent),
    '--glow': rgb(p.text).join(', '),
  }
  for (const [k, v] of Object.entries(set)) root.style.setProperty(k, v)
  root.setAttribute('data-custom', luma(p.bg) > 0.5 ? 'light' : 'dark')
}

function persist() {
  try {
    localStorage.setItem(KEY, JSON.stringify($state.snapshot(themes)))
  } catch {}
  applyTheme()
}

export function choosePreset(id: string) {
  themes.preset = id
  themes.custom = null
  persist()
}

export function setColor(key: ColorKey, value: string) {
  const base = activePalette() ?? { bg: '#000000', surface: '#0a0a0a', text: '#fafafa', accent: '#fafafa' }
  themes.custom = { ...base, [key]: value }
  themes.preset = 'custom'
  persist()
}

export function resetTheme() {
  choosePreset('mono')
}

export const isCustomized = () => themes.preset !== 'mono'
