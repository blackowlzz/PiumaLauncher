type Kind = 'line' | 'clamp' | 'scroll'

const kinds = new Map<HTMLElement, Kind>()
const seen = new WeakSet<Element>()

function solidBg(el: HTMLElement | null): string {
  for (; el; el = el.parentElement) {
    const c = getComputedStyle(el).backgroundColor
    if (c && c !== 'transparent' && !/rgba\(.*,\s*0\)$/.test(c)) return c
  }
  return 'var(--bg)'
}

function oneColumn(cs: CSSStyleDeclaration) {
  if (cs.display === 'block') return true
  if (cs.display === 'flex') return cs.flexDirection === 'column'
  if (cs.display === 'grid') return cs.gridTemplateColumns.trim().split(/\s+/).length === 1
  return false
}

function paintScroll(el: HTMLElement) {
  const cs = getComputedStyle(el)
  el.style.setProperty('--fade-bg', solidBg(el))
  el.style.setProperty('--fade-gap', cs.rowGap === 'normal' ? '0px' : cs.rowGap)
  el.style.setProperty('--fade-pt', cs.paddingTop)
  el.style.setProperty('--fade-pb', cs.paddingBottom)
}

function classify(el: HTMLElement) {
  if (seen.has(el)) return
  seen.add(el)
  const cs = getComputedStyle(el)
  if ((cs.overflowY === 'auto' || cs.overflowY === 'scroll') && el.tagName !== 'MAIN' && el.tagName !== 'NAV' && oneColumn(cs)) {
    kinds.set(el, 'scroll')
    paintScroll(el)
  } else if (cs.textOverflow === 'ellipsis') {
    kinds.set(el, 'line')
  } else if (cs.webkitLineClamp && cs.webkitLineClamp !== 'none') {
    kinds.set(el, 'clamp')
    el.style.setProperty('--lines', cs.webkitLineClamp)
  }
}

function classifyTree(root: Element) {
  if (root instanceof HTMLElement) classify(root)
  for (const el of root.querySelectorAll<HTMLElement>('*')) if (el instanceof HTMLElement) classify(el)
}

function edges(el: HTMLElement) {
  el.toggleAttribute('data-up', el.scrollTop > 2)
  el.toggleAttribute('data-down', el.scrollTop + el.clientHeight < el.scrollHeight - 2)
}

function update() {
  for (const [el, kind] of kinds) {
    if (!el.isConnected) {
      kinds.delete(el)
      continue
    }
    if (kind === 'scroll') {
      const on = el.scrollHeight > el.clientHeight + 1
      el.toggleAttribute('data-sfade', on)
      if (on) edges(el)
    } else if (kind === 'line') {
      if (el.scrollWidth > el.clientWidth + 1) el.dataset.fade = 'line'
      else delete el.dataset.fade
    } else {
      if (el.scrollHeight > el.clientHeight + 1) el.dataset.fade = 'clamp'
      else delete el.dataset.fade
    }
  }
}

export function watchFades() {
  let frame = 0
  const later = () => {
    if (!frame) frame = requestAnimationFrame(() => ((frame = 0), update()))
  }
  const onMutate = (records: MutationRecord[]) => {
    for (const r of records) for (const n of r.addedNodes) if (n instanceof Element) classifyTree(n)
    later()
  }
  const onScroll = (e: Event) => {
    const el = e.target
    if (el instanceof HTMLElement && el.hasAttribute('data-sfade')) edges(el)
  }
  const onTheme = () => {
    for (const [el, kind] of kinds) if (kind === 'scroll') paintScroll(el)
  }
  const scheme = matchMedia('(prefers-color-scheme: light)')
  const body = new MutationObserver(onMutate)
  const theme = new MutationObserver(() => requestAnimationFrame(onTheme))
  body.observe(document.body, { childList: true, subtree: true, characterData: true })
  theme.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'style', 'data-custom'] })
  scheme.addEventListener('change', onTheme)
  addEventListener('resize', later)
  document.addEventListener('scroll', onScroll, { capture: true, passive: true })
  classifyTree(document.body)
  later()
  return () => {
    cancelAnimationFrame(frame)
    body.disconnect()
    theme.disconnect()
    scheme.removeEventListener('change', onTheme)
    removeEventListener('resize', later)
    document.removeEventListener('scroll', onScroll, { capture: true })
  }
}
