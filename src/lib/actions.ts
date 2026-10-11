export function reveal(node: HTMLElement, delay = 0) {
  node.classList.add('rv')
  node.style.setProperty('--d', String(Math.min(delay, 12)))
  const io = new IntersectionObserver(
    (entries) => {
      if (entries.some((e) => e.isIntersecting)) {
        node.classList.add('in')
        io.disconnect()
      }
    },
    { threshold: 0.05 },
  )
  io.observe(node)
  return { destroy: () => io.disconnect() }
}

export function spotlight(node: HTMLElement) {
  node.classList.add('spot')
  let rect: DOMRect | null = null
  let raf = 0
  let x = 0
  let y = 0
  const enter = () => (rect = node.getBoundingClientRect())
  const leave = () => (rect = null)
  const move = (e: PointerEvent) => {
    rect ??= node.getBoundingClientRect()
    x = e.clientX - rect.left
    y = e.clientY - rect.top
    if (!raf)
      raf = requestAnimationFrame(() => {
        raf = 0
        node.style.setProperty('--mx', `${x}px`)
        node.style.setProperty('--my', `${y}px`)
      })
  }
  node.addEventListener('pointerenter', enter)
  node.addEventListener('pointerleave', leave)
  node.addEventListener('pointermove', move)
  return {
    destroy: () => {
      cancelAnimationFrame(raf)
      node.removeEventListener('pointerenter', enter)
      node.removeEventListener('pointerleave', leave)
      node.removeEventListener('pointermove', move)
    },
  }
}

export function portal(node: HTMLElement) {
  document.body.appendChild(node)
  return { destroy: () => node.remove() }
}
