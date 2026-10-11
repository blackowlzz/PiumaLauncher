import { api, type SkinInfo } from './api'
import { app } from './store.svelte'

type Entry = { state: 'loading' } | { state: 'none' } | ({ state: 'ready' } & SkinInfo)
const cache: Record<string, Entry> = $state({})

export const skinOf = (name: string): Entry | undefined => cache[name.toLowerCase()]

export async function ensureSkin(name: string) {
  const key = name.toLowerCase()
  if (cache[key]) return
  if (!app.settings.lookupSkins) {
    cache[key] = { state: 'none' }
    return
  }
  cache[key] = { state: 'loading' }
  try {
    const info = await api.fetchSkin(name)
    cache[key] = info ? { state: 'ready', ...info } : { state: 'none' }
  } catch {
    cache[key] = { state: 'none' }
  }
}
