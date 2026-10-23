<script lang="ts">
  import { causes } from './crashinfo'
  import { fade, slide } from 'svelte/transition'
  import { api, type CrashCause, type CrashFix } from './api'
  import { app, notify, play, reloadInstances } from './store.svelte'
  import { stageText, t } from './i18n.svelte'
  import Icon from './Icon.svelte'

  const modCauses: CrashCause[] = ['missingDeps', 'incompatibleMods', 'duplicateMods', 'mixin', 'suspectedMod']
  const shards = [
    [-1, -1, -14], [0, -1.4, 8], [1, -1, 16],
    [-1.4, 0, -6], [0, 0, 0], [1.4, 0, 10],
    [-1, 1, 12], [0, 1.4, -9], [1, 1, -18],
  ]

  const crash = $derived(app.crash)
  const inst = $derived(crash ? app.instances.find((i) => i.id === crash.instanceId) : undefined)
  let phase = $state<'idle' | 'working' | 'done'>('idle')
  let details = $state(false)
  let primary: HTMLButtonElement | undefined = $state()

  $effect(() => {
    if (crash) {
      phase = 'idle'
      details = false
      setTimeout(() => primary?.focus(), 0)
    }
  })

  const body = $derived.by(() => {
    if (!crash) return ''
    if (crash.cause === 'javaVersion' && crash.subject) return t('This game or one of its mods needs Java {version} or newer.', { version: crash.subject })
    return t(causes[crash.cause][1])
  })
  const chips = $derived(crash?.subject && modCauses.includes(crash.cause) ? crash.subject.split(', ') : [])
  const quote = $derived(crash?.subject && ['jvmArgs', 'unknown', 'nativeCrash'].includes(crash.cause) ? crash.subject : null)
  const progress = $derived.by(() => {
    const p = app.install
    if (phase !== 'working' || !p) return null
    return `${stageText(p.stage)}${p.total > 0 ? ` · ${Math.round((p.done / p.total) * 100)}%` : ''}`
  })

  function fixLabel(fix: CrashFix): string {
    switch (fix.kind) {
      case 'disableMods':
        return fix.files.length === 1 ? t('Disable the mod') : t('Disable {count} mods', { count: fix.files.length })
      case 'installMods':
        return t('Install from Modrinth')
      case 'memory':
        return t('Set memory to {mb} MB', { mb: fix.mb })
      case 'autoJava':
        return t('Use automatic Java')
      case 'clearJvmArgs':
        return t('Remove extra JVM arguments')
    }
  }

  const close = () => {
    if (phase !== 'working') app.crash = null
  }

  async function fix() {
    if (!crash || phase !== 'idle') return
    phase = 'working'
    app.install = null
    try {
      await api.applyCrashFix(crash.instanceId)
      await reloadInstances()
      phase = 'done'
      setTimeout(() => primary?.focus(), 0)
    } catch (e) {
      phase = 'idle'
      notify(e)
    } finally {
      app.install = null
    }
  }

  function again() {
    const id = crash?.instanceId
    app.crash = null
    if (id) play(id)
  }

  function onKey(e: KeyboardEvent) {
    if (crash && e.key === 'Escape') {
      e.preventDefault()
      close()
    }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if crash}
  <div class="backdrop" role="presentation" transition:fade={{ duration: 180 }} onclick={close}></div>
  <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="crash-title" aria-describedby="crash-body" out:fade={{ duration: 140 }}>
    {#if phase === 'working'}
      <div class="line indeterminate top"><div></div></div>
    {/if}

    <div class="head">
      <svg class="block" class:healed={phase === 'done'} viewBox="-2 -2 24 24" aria-hidden="true">
        {#each shards as [x, y, r], i}
          <rect x={1 + (i % 3) * 6} y={1 + Math.floor(i / 3) * 6} width="5.4" height="5.4" rx="1" style:--x="{x}px" style:--y="{y}px" style:--r="{r}deg" style:--d={i} />
        {/each}
        <path class="crack" pathLength="1" d="M10 -1 L8.6 5.5 L11.6 8.4 L8.4 12.6 L11 16 L9.6 21" />
      </svg>
      <div class="grow">
        <div class="eyebrow">
          {t('The game crashed')}{#if inst}<span class="sep">·</span>{inst.name}{/if}
          {#if crash.code !== null && crash.code !== 0}<span class="tag">{t('Exit code {code}', { code: crash.code })}</span>{/if}
        </div>
        <h3 id="crash-title">{t(causes[crash.cause][0])}</h3>
      </div>
    </div>

    <p id="crash-body" class="body">{body}</p>

    {#if chips.length}
      <div class="chips">
        {#each chips as c, i}<span class="tag mod" style:--d={i}>{c}</span>{/each}
      </div>
    {/if}
    {#if quote}<code class="quote">{quote}</code>{/if}

    {#if phase === 'done'}
      <div class="ok" in:slide={{ duration: 260 }}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="10" pathLength="1" /><path d="m7.5 12.5 3 3 6-6.5" pathLength="1" /></svg>
        {t('Fixed. You can try playing again.')}
      </div>
    {:else if progress}
      <div class="muted small progress" aria-live="polite">{progress}</div>
    {/if}

    {#if crash.excerpt.length}
      <button class="ghost toggle" aria-expanded={details} onclick={() => (details = !details)}>
        <span class="chev" class:open={details}><Icon name="chevron" size={14} /></span>{t('Details')}
      </button>
      {#if details}
        <pre class="log" transition:slide={{ duration: 220 }}>{#each crash.excerpt as l, i}<span style:--d={i}>{l}</span>{/each}</pre>
      {/if}
    {/if}

    <div class="actions">
      {#if crash.hasReport}
        <button class="ghost" onclick={() => api.openCrashReport(crash.instanceId).catch(notify)}><Icon name="file" size={15} />{t('Open report')}</button>
      {/if}
      <span class="grow"></span>
      <button class="ghost" onclick={close} disabled={phase === 'working'}>{t('Close')}</button>
      {#if phase === 'done'}
        <button bind:this={primary} class="primary go" onclick={again}><Icon name="play" size={15} />{t('Play again')}</button>
      {:else if crash.fix}
        <button bind:this={primary} class="primary go" onclick={fix} disabled={phase === 'working'}>
          <Icon name="wrench" size={15} />{phase === 'working' ? t('Fixing…') : fixLabel(crash.fix)}
        </button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .backdrop { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.62); z-index: 60; }
  .dialog { position: fixed; z-index: 61; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(540px, calc(100vw - 32px)); max-height: calc(100vh - 48px); overflow: hidden auto; display: grid; gap: 16px; padding: 26px; background: var(--surface); border: 1px solid var(--line-strong); border-radius: 20px; box-shadow: 0 30px 90px rgba(0, 0, 0, 0.55); animation: crash-in 520ms var(--spring) both; }
  .top { position: absolute; top: 0; left: 0; right: 0; border-radius: 0; }
  .head { display: flex; gap: 16px; align-items: center; }
  .block { width: 54px; height: 54px; flex: none; overflow: visible; }
  .block rect { fill: var(--text); transform-box: fill-box; transform-origin: center; animation: shatter 900ms var(--spring) calc(120ms + var(--d) * 18ms) both; }
  .crack { fill: none; stroke: #e5484d; stroke-width: 1.3; stroke-linecap: round; stroke-linejoin: round; stroke-dasharray: 1; stroke-dashoffset: 1; animation: draw 460ms ease-out 420ms forwards; }
  .healed rect { animation: heal 560ms var(--spring) calc(var(--d) * 26ms) both; }
  .healed .crack { animation: undraw 300ms ease-in forwards; }
  .eyebrow { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; animation: swapin 400ms var(--spring) 180ms backwards; }
  .sep { color: var(--faint); }
  .eyebrow .tag { margin-left: 4px; font-size: 10.5px; padding: 1px 8px; }
  h3 { margin: 4px 0 0; font-size: 21px; font-weight: 650; letter-spacing: -0.03em; line-height: 1.2; animation: swapin 460ms var(--spring) 240ms backwards; }
  .body { color: var(--muted); line-height: 1.55; animation: swapin 460ms var(--spring) 300ms backwards; }
  .chips { gap: 6px; }
  .mod { font-size: 12px; padding: 3px 11px; color: var(--text); animation: gopop 380ms var(--spring) calc(360ms + var(--d) * 50ms) backwards; }
  .quote { display: block; font: 12px/1.5 ui-monospace, 'JetBrains Mono', monospace; padding: 10px 12px; border-radius: 12px; background: var(--surface-2); border: 1px solid var(--line); overflow-wrap: anywhere; animation: swapin 460ms var(--spring) 360ms backwards; }
  .progress { animation: swapin 300ms var(--spring) backwards; }
  .ok { display: flex; align-items: center; gap: 10px; font-weight: 550; }
  .ok svg { width: 22px; height: 22px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
  .ok circle, .ok path { stroke-dasharray: 1; stroke-dashoffset: 1; animation: draw 520ms var(--spring) forwards; }
  .ok path { animation-delay: 260ms; }
  .toggle { justify-self: start; display: inline-flex; align-items: center; gap: 6px; padding: 6px 12px 6px 8px; font-size: 13px; margin: -4px 0 -4px -8px; }
  .chev { display: inline-grid; transform: rotate(-90deg); transition: transform 260ms var(--spring); }
  .chev.open { transform: none; }
  .log { margin: 0; max-height: 200px; overflow: auto; padding: 12px 14px; border-radius: 12px; background: var(--bg); border: 1px solid var(--line); font: 11.5px/1.6 ui-monospace, 'JetBrains Mono', monospace; color: var(--muted); }
  .log span { display: block; white-space: pre-wrap; overflow-wrap: anywhere; animation: swapin 320ms var(--spring) calc(var(--d) * 28ms) backwards; }
  .actions { display: flex; align-items: center; gap: 8px; margin-top: 4px; }
  .actions button { display: inline-flex; align-items: center; gap: 8px; }
  .go { animation: gopop 380ms var(--spring) 380ms backwards; }
  @keyframes crash-in { 0% { opacity: 0; transform: translate(-50%, -46%) scale(0.96); } 55% { transform: translate(calc(-50% - 5px), -50%); } 70% { transform: translate(calc(-50% + 4px), -50%); } 85% { transform: translate(calc(-50% - 2px), -50%); } }
  @keyframes shatter { 0% { transform: scale(0.4); opacity: 0; } 30% { transform: none; opacity: 1; } 60% { transform: translate(calc(var(--x) * 2.6), calc(var(--y) * 2.6)) rotate(var(--r)); } 100% { transform: translate(var(--x), var(--y)) rotate(calc(var(--r) / 3)); } }
  @keyframes heal { from { transform: translate(var(--x), var(--y)) rotate(calc(var(--r) / 3)); } 50% { transform: scale(0.86); } to { transform: none; } }
  @keyframes draw { to { stroke-dashoffset: 0; } }
  @keyframes undraw { from { stroke-dashoffset: 0; } to { stroke-dashoffset: 1; opacity: 0; } }
  :global(:root[data-reduce]) .ok :is(circle, path), :global(:root[data-reduce]) .block:not(.healed) .crack { stroke-dashoffset: 0; }
  @media (prefers-reduced-motion: reduce) { .ok :is(circle, path), .block:not(.healed) .crack { stroke-dashoffset: 0; } }
  @media (max-width: 560px) { .actions { flex-wrap: wrap; } .actions .grow { display: none; } }
</style>
