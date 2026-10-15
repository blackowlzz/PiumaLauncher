import { api, inTauri } from './api'

const BLANK = 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7'
const pending = new Map<string, Promise<string>>()

function fetchThumb(src: string, w: number): Promise<string> {
  const key = `${w}|${src}`
  let p = pending.get(key)
  if (!p) {
    p = api.thumbnail(src, w).finally(() => pending.delete(key))
    pending.set(key, p)
  }
  return p
}

export function thumb(node: HTMLImageElement, opts: { src: string; w: number }) {
  let current = opts
  let started = ''
  let io: IntersectionObserver | undefined

  const onLoad = () => {
    if (node.src !== BLANK) delete node.dataset.ph
  }
  const onError = () => {
    node.dataset.ph = ''
    if (node.src !== BLANK) node.src = BLANK
  }
  node.addEventListener('load', onLoad)
  node.addEventListener('error', onError)

  async function load() {
    const { src, w } = current
    const id = `${w}|${src}`
    if (started === id) return
    started = id
    try {
      const url = await fetchThumb(src, w)
      if (started === id) node.src = url
    } catch {
      if (started === id) node.src = src
    }
  }

  function watch() {
    io?.disconnect()
    if (!current.src) return
    node.dataset.ph = ''
    node.src = BLANK
    if (!inTauri || typeof IntersectionObserver === 'undefined') {
      node.src = current.src
      return
    }
    io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          io?.disconnect()
          void load()
        }
      },
      { rootMargin: '300px' },
    )
    io.observe(node)
  }

  watch()
  return {
    update(next: { src: string; w: number }) {
      if (next.src === current.src && next.w === current.w) return
      current = next
      started = ''
      delete node.dataset.ph
      node.src = BLANK
      watch()
    },
    destroy() {
      io?.disconnect()
      node.removeEventListener('load', onLoad)
      node.removeEventListener('error', onError)
    },
  }
}
