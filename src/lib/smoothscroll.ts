export function smoothScroll(node: HTMLElement, enabled = true) {
  let on = enabled
  let target = node.scrollTop
  let pos = target
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

  function stop() {
    cancelAnimationFrame(raf)
    raf = 0
    last = 0
  }

  function step(now: number) {
    const dt = Math.min(48, now - (last || now))
    last = now
    const diff = target - pos
    if (Math.abs(diff) < 0.5) {
      pos = target
      node.scrollTop = target
      stop()
      return
    }
    pos += diff * (1 - Math.exp(-dt / 95))
    node.scrollTop = pos
    raf = requestAnimationFrame(step)
  }

  function onWheel(e: WheelEvent) {
    if (!on || e.ctrlKey || e.shiftKey || e.defaultPrevented || e.deltaY === 0) return
    if (innerCanScroll(e.target, e.deltaY)) return
    const dy = e.deltaMode === 1 ? e.deltaY * 40 : e.deltaMode === 2 ? e.deltaY * node.clientHeight : e.deltaY
    if (!raf) target = pos = node.scrollTop
    const next = Math.max(0, Math.min(max(), target + dy))
    if (next === target && node.scrollTop === target) return
    e.preventDefault()
    target = next
    if (!raf) raf = requestAnimationFrame(step)
  }

  const onScroll = () => {
    if (raf && Math.abs(node.scrollTop - pos) <= 2) return
    stop()
    target = pos = node.scrollTop
  }

  node.addEventListener('wheel', onWheel, { passive: false })
  node.addEventListener('scroll', onScroll, { passive: true })
  return {
    update: (v: boolean) => (on = v),
    destroy: () => {
      stop()
      node.removeEventListener('wheel', onWheel)
      node.removeEventListener('scroll', onScroll)
    },
  }
}
