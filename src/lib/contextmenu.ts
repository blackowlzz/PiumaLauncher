import type { ComponentProps } from 'svelte'
import type Icon from './Icon.svelte'
import { app } from './store.svelte'

export type MenuEntry = {
  label: string
  icon?: ComponentProps<typeof Icon>['name']
  onclick?: () => void
  danger?: boolean
  disabled?: boolean
  children?: MenuEntry[]
}
export type MenuItem = MenuEntry | 'sep'

export function ctx(node: HTMLElement, items: () => MenuItem[]) {
  let get = items
  const open = (e: MouseEvent) => {
    e.preventDefault()
    e.stopPropagation()
    const list = get().filter((i, n, all) => i !== 'sep' || (n > 0 && n < all.length - 1 && all[n - 1] !== 'sep'))
    if (list.length) app.ctx = { x: e.clientX, y: e.clientY, items: list }
  }
  node.addEventListener('contextmenu', open)
  return {
    update(next: () => MenuItem[]) {
      get = next
    },
    destroy: () => node.removeEventListener('contextmenu', open),
  }
}

export async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    return false
  }
}
