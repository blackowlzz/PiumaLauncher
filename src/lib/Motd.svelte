<script lang="ts">
  let { text }: { text: string } = $props()

  const palette: Record<string, string> = {
    '0': '#000000', '1': '#0000aa', '2': '#00aa00', '3': '#00aaaa', '4': '#aa0000', '5': '#aa00aa', '6': '#ffaa00', '7': '#aaaaaa',
    '8': '#555555', '9': '#5555ff', a: '#55ff55', b: '#55ffff', c: '#ff5555', d: '#ff55ff', e: '#ffff55', f: '#ffffff',
  }
  type Seg = { text: string; color: string | null; cls: string }

  function parse(line: string): Seg[] {
    const out: Seg[] = []
    let color: string | null = null
    let flags = new Set<string>()
    let buf = ''
    const flush = () => {
      if (buf) out.push({ text: buf, color, cls: [...flags].join(' ') })
      buf = ''
    }
    for (let i = 0; i < line.length; i++) {
      if (line[i] !== '§' || i + 1 >= line.length) {
        buf += line[i]
        continue
      }
      flush()
      const code = line[i + 1].toLowerCase()
      if (code === '#' && /^[0-9a-f]{6}$/i.test(line.slice(i + 2, i + 8))) {
        color = `#${line.slice(i + 2, i + 8)}`
        flags = new Set()
        i += 7
        continue
      }
      i++
      if (palette[code]) {
        color = palette[code]
        flags = new Set()
      } else if (code === 'r') {
        color = null
        flags = new Set()
      } else if ('klmno'.includes(code)) {
        flags.add(`f${code}`)
      }
    }
    flush()
    while (out.length && !out[0].text.trim()) out.shift()
    if (out.length) out[0].text = out[0].text.trimStart()
    return out
  }

  const lines = $derived(text.split('\n').slice(0, 2).map(parse))
</script>

<div class="motd">
  {#each lines as segs, n}
    <div class="ln" style:--n={n}>{#each segs as s}<span class={s.cls} style:color={s.color}>{s.text}</span>{/each}</div>
  {/each}
</div>

<style>
  .motd { display: grid; gap: 1px; padding: 8px 12px; border-radius: 10px; background: #0b0b0b; border: 1px solid #1f1f1f; color: #aaaaaa; font: 12.5px/1.45 ui-monospace, 'JetBrains Mono', monospace; letter-spacing: 0; min-width: 0; }
  .ln { white-space: pre; overflow: hidden; text-overflow: ellipsis; animation: swapin 420ms var(--spring) calc(var(--n) * 90ms) backwards; }
  .fl { font-weight: 700; }
  .fo { font-style: italic; }
  .fn { text-decoration: underline; }
  .fm { text-decoration: line-through; }
  .fn.fm { text-decoration: underline line-through; }
  .fk { filter: blur(2.5px); animation: scramble 0.35s steps(2) infinite; }
  @keyframes scramble { 50% { filter: blur(1.5px); opacity: 0.6; } }
</style>
