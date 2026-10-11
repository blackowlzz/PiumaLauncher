export function smoothScroll(node: HTMLElement, enabled = true) {
  let on = enabled
  let target = node.scrollTop
  let raf = 0
  let last = 0

  const max = () => node.scrollHeight - node.clientHeight

  function innerCanScroll(el: EventTarget | null, dy: number) {
    for (let n = el as HTMLElement | null; n && n !== node; n = n.parentElement) {
      const oy = getComputedStyle(n).overflowY
      if ((oy === 'auto' || oy === 'scroll') && n.scrollHeight > n.clientHeight + 1) {
        if (dy < 0 ? n.scrollTop > 0 : n.scrollTop + n.clientHeight < n.scrollHeight - 1) return true
      }
    }
    return false
  }

  function step(now: number) {
    const dt = Math.min(48, now - (last || now))
    last = now
    const diff = target - node.scrollTop
    if (Math.abs(diff) < 0.6) {
      node.scrollTop = target
      raf = 0
      last = 0
      return
    }
    node.scrollTop += diff * (1 - Math.exp(-dt / 95))
    raf = requestAnimationFrame(step)
  }

  function onWheel(e: WheelEvent) {
    if (!on || e.ctrlKey || e.shiftKey || e.defaultPrevented || e.deltaY === 0) return
    if (innerCanScroll(e.target, e.deltaY)) return
    const dy = e.deltaMode === 1 ? e.deltaY * 40 : e.deltaMode === 2 ? e.deltaY * node.clientHeight : e.deltaY
    if (!raf) target = node.scrollTop
    const next = Math.max(0, Math.min(max(), target + dy))
    if (next === target && node.scrollTop === target) return
    e.preventDefault()
    target = next
    if (!raf) raf = requestAnimationFrame(step)
  }

  const onScroll = () => {
    if (!raf) target = node.scrollTop
  }

  node.addEventListener('wheel', onWheel, { passive: false })
  node.addEventListener('scroll', onScroll, { passive: true })
  return {
    update: (v: boolean) => (on = v),
    destroy: () => {
      cancelAnimationFrame(raf)
      node.removeEventListener('wheel', onWheel)
      node.removeEventListener('scroll', onScroll)
    },
  }
}
