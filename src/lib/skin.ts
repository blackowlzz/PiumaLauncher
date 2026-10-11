
const SKIN = ['#f1c9a5', '#dba87b', '#b9814f', '#8a5a36', '#fbdcc4']
const HAIR = ['#2b1d12', '#6b4423', '#c9a24b', '#8f2f1c', '#161616', '#d9d9d9']
const EYES = ['#2c3e8f', '#2f7d4f', '#5a3b1e', '#3a3a3a']
const SHIRT = ['#3a7bd5', '#c0392b', '#2e8b57', '#8e44ad', '#e1a100', '#34495e', '#e0607e', '#16a3a3']
const PANTS = ['#2d3a8c', '#3b3b3b', '#5b3a1e', '#1f6f5f', '#6b2d5c', '#4b5563']
const SHOES = ['#242424', '#4a4a4a', '#5b3a1e']

function hashes(name: string) {
  let h = 2166136261
  for (const c of name || '?') {
    h ^= c.charCodeAt(0)
    h = Math.imul(h, 16777619)
  }
  const h1 = h >>> 0
  const h2 = Math.imul(h1 ^ (h1 >>> 15), 2246822519) >>> 0
  return { h1, h2 }
}

export function palette(name: string) {
  const { h1, h2 } = hashes(name)
  const pick = <T,>(a: T[], h: number, shift: number) => a[((h >>> shift) & 0xff) % a.length]
  return {
    skin: pick(SKIN, h1, 0),
    hair: pick(HAIR, h1, 8),
    eye: pick(EYES, h1, 16),
    shirt: pick(SHIRT, h2, 0),
    pants: pick(PANTS, h2, 8),
    shoes: pick(SHOES, h2, 16),
    fringe: (h1 >>> 24) % 3,
    beard: ((h1 >>> 5) & 3) === 0,
  }
}

export function headCells(name: string): string[][] {
  const p = palette(name)
  const shade = 'rgba(0,0,0,0.16)'
  const g: string[][] = Array.from({ length: 8 }, () => Array(8).fill(p.skin))
  for (let x = 0; x < 8; x++) g[0][x] = g[1][x] = p.hair
  for (const x of [[0, 1, 6, 7], [0, 1, 2, 7], [0, 7]][p.fringe]) g[2][x] = p.hair
  g[3][0] = g[3][7] = p.hair
  g[4][1] = p.eye
  g[4][2] = '#fff'
  g[4][5] = '#fff'
  g[4][6] = p.eye
  g[5][3] = g[5][4] = shade
  for (const x of [2, 3, 4, 5]) g[6][x] = p.beard ? p.hair : shade
  if (p.beard) for (let x = 1; x <= 6; x++) g[7][x] = p.hair
  return g
}

export function makeSkin(name: string): string {
  const p = palette(name)
  const cv = document.createElement('canvas')
  cv.width = cv.height = 64
  const ctx = cv.getContext('2d')!
  const rect = (x: number, y: number, w: number, h: number, c: string) => {
    ctx.fillStyle = c
    ctx.fillRect(x, y, w, h)
  }

  rect(0, 8, 32, 8, p.skin)
  rect(8, 0, 8, 8, p.hair)
  rect(16, 0, 8, 8, p.skin)
  rect(24, 8, 8, 8, p.hair)
  rect(0, 8, 8, 3, p.hair); rect(0, 8, 2, 8, p.hair)
  rect(16, 8, 8, 3, p.hair); rect(22, 8, 2, 8, p.hair)
  headCells(name).forEach((row, y) => row.forEach((c, x) => {
    ctx.clearRect(8 + x, 8 + y, 1, 1)
    rect(8 + x, 8 + y, 1, 1, c.startsWith('rgba') ? p.skin : c)
    if (c.startsWith('rgba')) rect(8 + x, 8 + y, 1, 1, c)
  }))

  rect(16, 20, 16, 12, p.shirt)
  rect(32, 20, 8, 12, p.shirt)
  rect(20, 16, 8, 4, p.shirt)
  rect(28, 16, 8, 4, p.pants)
  rect(22, 20, 4, 2, p.skin)

  for (const [ox, oy] of [[40, 16], [32, 48]] as const) {
    rect(ox + 4, oy, 4, 4, p.shirt)
    rect(ox + 8, oy, 4, 4, p.skin)
    rect(ox, oy + 4, 16, 12, p.skin)
    rect(ox, oy + 4, 16, 5, p.shirt)
  }
  for (const [ox, oy] of [[0, 16], [16, 48]] as const) {
    rect(ox + 4, oy, 4, 4, p.pants)
    rect(ox + 8, oy, 4, 4, p.shoes)
    rect(ox, oy + 4, 16, 12, p.pants)
    rect(ox, oy + 14, 16, 2, p.shoes)
  }

  const { h1 } = hashes(name)
  let seed = h1
  const img = ctx.getImageData(0, 0, 64, 64)
  for (let i = 0; i < img.data.length; i += 4) {
    if (!img.data[i + 3]) continue
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0
    const d = ((seed >>> 24) % 9) - 4
    img.data[i] = Math.max(0, Math.min(255, img.data[i] + d))
    img.data[i + 1] = Math.max(0, Math.min(255, img.data[i + 1] + d))
    img.data[i + 2] = Math.max(0, Math.min(255, img.data[i + 2] + d))
  }
  ctx.putImageData(img, 0, 0)
  return cv.toDataURL('image/png')
}
