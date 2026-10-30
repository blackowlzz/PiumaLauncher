<script lang="ts" module>
  import type { ComponentProps } from 'svelte'
  import Icon from './Icon.svelte'

  export type SectionCard = {
    id: string
    icon: ComponentProps<typeof Icon>['name']
    title: string
    sub: string
    tag?: string
    danger?: boolean
    span?: 1 | 2 | 3
    field?: number
  }
</script>

<script lang="ts">
  import PixelField from './PixelField.svelte'
  import { reveal } from './actions'

  let { cards, onselect }: { cards: SectionCard[]; onselect: (id: string) => void } = $props()
</script>

<div class="cards">
  {#each cards as c, n (c.id)}
    <button class="scard" class:danger={c.danger} class:s2={c.span === 2} class:s3={c.span === 3} class:art={c.field !== undefined} use:reveal={n} onclick={() => onselect(c.id)}>
      {#if c.field !== undefined}<span class="field"><PixelField level={c.field} /></span>{/if}
      <span class="si"><Icon name={c.icon} size={26} /></span>
      <span class="go"><Icon name="chevron" size={16} /></span>
      <span class="st">{c.title}{#if c.tag}<span class="tag">{c.tag}</span>{/if}</span>
      <span class="ss">{c.sub}</span>
    </button>
  {/each}
</div>

<style>
  .cards { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); grid-auto-flow: dense; gap: 16px; }
  .scard { position: relative; display: flex; flex-direction: column; align-items: flex-start; gap: 8px; min-height: 172px; padding: 24px 52px 24px 24px; text-align: left; border-radius: 20px; border: 1px solid var(--line); background: var(--surface); overflow: hidden; isolation: isolate; transition: border-color var(--ease), transform 300ms var(--spring); }
  .scard:hover:not(:disabled) { border-color: var(--line-strong); background: var(--surface); transform: translateY(-2px); }
  .s2 { grid-column: span 2; }
  .s3 { grid-column: span 3; }
  .art .ss { max-width: 52%; }
  .field { position: absolute; inset: 0; z-index: -1; }
  .si { display: inline-grid; color: var(--text); margin-bottom: 14px; }
  .st { display: inline-flex; align-items: center; flex-wrap: wrap; gap: 10px; font-size: 17px; font-weight: 600; letter-spacing: -0.02em; color: var(--text); }
  .st .tag { font-size: 11px; font-weight: 500; letter-spacing: 0; color: var(--inv-text); background: var(--inv-bg); border-color: var(--inv-bg); }
  .ss { font-size: 13.5px; line-height: 1.5; color: var(--muted); display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
  .go { position: absolute; right: 22px; top: 26px; color: var(--faint); display: inline-grid; transition: transform var(--ease), color var(--ease); }
  .scard:hover .go { color: var(--text); transform: translateX(3px); }
  .scard.danger .si { color: #e5484d; }
  @media (max-width: 1000px) { .cards { grid-template-columns: repeat(2, minmax(0, 1fr)); } .s3 { grid-column: span 2; } }
  @media (max-width: 640px) { .cards { grid-template-columns: minmax(0, 1fr); } .s2, .s3 { grid-column: auto; } }
</style>
