
const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

function inline(raw: string): string {
  let s = esc(raw)
  const codes: string[] = []
  s = s.replace(/`([^`]+)`/g, (_, c) => `\u0000${codes.push(`<code>${c}</code>`) - 1}\u0000`)
  s = s.replace(/!\[([^\]]*)\]\((https:\/\/cdn\.modrinth\.com\/[^\s)]+)[^)]*\)/g, (_, alt, url) => `<img src="${url}" alt="${alt}" loading="lazy" decoding="async">`)
  s = s.replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1')
  s = s.replace(/\[\s*\]\([^)]*\)/g, '')
  s = s.replace(/\[([^\]]+)\]\((https:\/\/[^\s)]+)[^)]*\)/g, (_, label, url) => `<a href="${url}" data-ext>${label}</a>`)
  s = s.replace(/\*\*([^*]+)\*\*|__([^_]+)__/g, (_, a, b) => `<strong>${a ?? b}</strong>`)
  s = s.replace(/(^|[\s(])\*([^*\s][^*]*)\*(?=$|[\s).,!?])/g, '$1<em>$2</em>')
  s = s.replace(/~~([^~]+)~~/g, '<s>$1</s>')
  return s.replace(/\u0000(\d+)\u0000/g, (_, i) => codes[+i])
}

export function renderMarkdown(src: string): string {
  const defs = new Map<string, string>()
  const text = src
    .replace(/\r/g, '')
    .replace(/<[^>]*>/g, ' ')
    .replace(/^\s*\[([^\]]+)\]:\s*(\S+).*$/gm, (_, id: string, url: string) => (defs.set(id.toLowerCase(), url), ''))
    .replace(/\[([^\]]+)\]\[([^\]]*)\]/g, (m, label: string, id: string) => {
      const url = defs.get((id || label).toLowerCase())
      return url ? `[${label}](${url})` : label
    })
  const lines = text.split('\n')
  const out: string[] = []
  let list: 'ul' | 'ol' | null = null
  let para: string[] = []
  let code: string[] | null = null
  const flushPara = () => {
    if (para.length) out.push(`<p>${inline(para.join(' '))}</p>`)
    para = []
  }
  const closeList = () => {
    if (list) out.push(`</${list}>`)
    list = null
  }
  for (const line of lines) {
    if (line.trimStart().startsWith('```')) {
      if (code) {
        out.push(`<pre><code>${esc(code.join('\n'))}</code></pre>`)
        code = null
      } else {
        flushPara()
        closeList()
        code = []
      }
      continue
    }
    if (code) {
      code.push(line)
      continue
    }
    const h = /^(#{1,6})\s+(.*)$/.exec(line)
    const li = /^\s*(?:[-*+]|(\d+)[.)])\s+(.*)$/.exec(line)
    if (!line.trim()) {
      flushPara()
      closeList()
    } else if (h) {
      flushPara()
      closeList()
      const n = Math.min(4, h[1].length + 1)
      out.push(`<h${n}>${inline(h[2])}</h${n}>`)
    } else if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) {
      flushPara()
      closeList()
      out.push('<hr>')
    } else if (li) {
      flushPara()
      const kind = li[1] ? 'ol' : 'ul'
      if (list !== kind) {
        closeList()
        out.push(`<${kind}>`)
        list = kind
      }
      out.push(`<li>${inline(li[2])}</li>`)
    } else if (line.startsWith('>')) {
      flushPara()
      closeList()
      out.push(`<blockquote>${inline(line.replace(/^>\s?/, ''))}</blockquote>`)
    } else {
      closeList()
      para.push(line.trim())
    }
  }
  if (code) out.push(`<pre><code>${esc(code.join('\n'))}</code></pre>`)
  flushPara()
  closeList()
  return out.join('\n')
}
