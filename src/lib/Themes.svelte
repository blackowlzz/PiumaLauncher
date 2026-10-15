<script lang="ts">
  import Icon from './Icon.svelte'
  import { t } from './i18n.svelte'
  import { COLOR_KEYS, PRESETS, activePalette, choosePreset, onAccent, isCustomized, resetTheme, setColor, themes, type ColorKey, type Palette } from './palette.svelte'

  const labels: Record<ColorKey, () => string> = {
    bg: () => t('Background'),
    surface: () => t('Surfaces'),
    text: () => t('Text'),
    accent: () => t('Accent'),
  }
  const hints: Record<ColorKey, () => string> = {
    bg: () => t('Window and sidebar'),
    surface: () => t('Cards and panels'),
    text: () => t('Titles and icons'),
    accent: () => t('Play button and highlights'),
  }

  const fallback: Palette = { bg: '#000000', surface: '#0a0a0a', text: '#fafafa', accent: '#fafafa' }
  const current = $derived(activePalette() ?? fallback)

  let drafts = $state<Partial<Record<ColorKey, string>>>({})
  function commit(key: ColorKey, value: string) {
    const v = value.trim().startsWith('#') ? value.trim() : `#${value.trim()}`
    drafts[key] = v
    if (/^#[0-9a-f]{6}$/i.test(v)) setColor(key, v.toLowerCase())
  }
  const shown = (key: ColorKey) => drafts[key] ?? current[key]
  $effect(() => {
    void current
    drafts = {}
  })
</script>

<header class="head">
  <h1>{t('Themes')}</h1>
  <p class="muted">{t('Pick a preset or tune every color yourself.')}</p>
</header>

<div class="cols">
  <section class="group">
    <h2>{t('Presets')}</h2>
    <div class="presets">
      {#each PRESETS as p}
        {@const c = p.palette ?? fallback}
        <button class="preset" class:on={themes.preset === p.id} aria-pressed={themes.preset === p.id} onclick={() => choosePreset(p.id)}>
          <span class="mini" style:background={c.bg} style:border-color={c.text + '33'}>
            <span class="side" style:background={c.surface}></span>
            <span class="body">
              <span class="ln" style:background={c.text}></span>
              <span class="ln short" style:background={c.text + '66'}></span>
              <span class="btn" style:background={c.accent}></span>
            </span>
          </span>
          <span class="pn">{t(p.name)}</span>
        </button>
      {/each}
    </div>
  </section>

  <section class="group">
    <div class="gh">
      <h2>{t('Colors')}</h2>
      {#if isCustomized()}<button class="ghost" onclick={resetTheme}><Icon name="refresh" size={15} /> {t('Reset')}</button>{/if}
    </div>
    <div class="rows">
      {#each COLOR_KEYS as k}
        <div class="crow">
          <label class="swatch" style:background={current[k]}>
            <input type="color" value={current[k]} oninput={(e) => commit(k, e.currentTarget.value)} aria-label={labels[k]()} />
          </label>
          <div class="grow">
            <div class="n">{labels[k]()}</div>
            <div class="muted small">{hints[k]()}</div>
          </div>
          <input class="hex" type="text" maxlength="7" spellcheck="false" value={shown(k)} oninput={(e) => commit(k, e.currentTarget.value)} aria-label={labels[k]()} />
        </div>
      {/each}
    </div>

    <div class="preview" style:background={current.bg} style:border-color={current.text + '26'}>
      <div class="pcard" style:background={current.surface} style:border-color={current.text + '1c'}>
        <div class="pt" style:color={current.text}>{t('Preview')}</div>
        <div class="ps" style:color={current.text + '99'}>PiumaLauncher</div>
        <span class="pb" style:background={current.accent} style:color={onAccent(current.accent)}>{t('Play')}</span>
      </div>
    </div>
  </section>
</div>

<style>
  .head { display: grid; gap: 6px; }
  .cols { display: grid; grid-template-columns: minmax(0, 1.3fr) minmax(0, 1fr); gap: 28px; align-items: start; }
  .group { display: grid; gap: 14px; }
  .group h2 { margin: 0; font-size: 16px; font-weight: 600; color: var(--text); }
  .gh { display: flex; align-items: center; justify-content: space-between; }
  .gh button { display: inline-flex; align-items: center; gap: 6px; padding: 6px 12px; }
  .presets { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 12px; }
  .preset { display: grid; gap: 10px; padding: 10px; border-radius: 16px; background: var(--surface); border-color: var(--line); text-align: left; }
  .preset.on { border-color: var(--text); }
  .mini { display: flex; height: 76px; border-radius: 10px; border: 1px solid; overflow: hidden; }
  .side { width: 26%; }
  .body { flex: 1; display: grid; align-content: center; gap: 6px; padding: 0 10px; }
  .ln { height: 5px; width: 70%; border-radius: 3px; }
  .ln.short { width: 45%; }
  .btn { height: 10px; width: 36px; border-radius: 6px; margin-top: 2px; }
  .pn { font-size: 13px; font-weight: 500; }
  .rows { display: grid; gap: 4px; background: var(--surface); border: 1px solid var(--line); border-radius: 16px; padding: 6px 14px; }
  .crow { display: flex; align-items: center; gap: 14px; padding: 10px 0; border-bottom: 1px solid var(--line); }
  .crow:last-child { border-bottom: 0; }
  .swatch { position: relative; width: 38px; height: 38px; border-radius: 12px; border: 1px solid var(--line-strong); overflow: hidden; cursor: pointer; flex: none; display: block; }
  .swatch input { position: absolute; inset: -6px; width: 60px; height: 60px; opacity: 0; cursor: pointer; padding: 0; border: 0; }
  .n { font-weight: 500; }
  .hex { width: 96px; font-family: inherit; font-size: 13px; text-transform: lowercase; padding: 8px 12px; }
  .preview { border: 1px solid; border-radius: 16px; padding: 18px; }
  .pcard { border: 1px solid; border-radius: 14px; padding: 16px; display: grid; gap: 4px; justify-items: start; }
  .pt { font-size: 18px; font-weight: 650; letter-spacing: -0.03em; }
  .ps { font-size: 12px; margin-bottom: 10px; }
  .pb { padding: 8px 20px; border-radius: 999px; font-weight: 600; font-size: 13px; }
  @media (max-width: 1100px) { .cols { grid-template-columns: minmax(0, 1fr); } }
</style>
