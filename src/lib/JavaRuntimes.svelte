<script lang="ts">
  import { onMount } from 'svelte'
  import { api, type RuntimeEntry } from './api'
  import { stageText, t } from './i18n.svelte'
  import { app, askConfirm, installRuntime, notify, reloadRuntimes, saveSettings } from './store.svelte'

  let { onchange }: { onchange?: () => void } = $props()

  onMount(reloadRuntimes)

  const note = (r: RuntimeEntry) =>
    r.flavor === 'graalvm'
      ? t('Fast peak performance, ideal for modern versions.')
      : r.major <= 8
        ? t('For Minecraft 1.12 and older, and Forge up to 1.16.')
        : t('Lightweight and very stable.')

  const ESSENTIAL = new Set(['graalvm-25', 'graalvm-21', 'temurin-8'])
  const rank = (r: RuntimeEntry) => (r.flavor === 'graalvm' ? 1000 : 0) + r.major
  const shown = (r: RuntimeEntry) => ESSENTIAL.has(r.id) || r.installed || app.settings.preferredJava === r.id || app.javaBusy === r.id
  const main = $derived(app.runtimes.filter(shown).sort((a, b) => rank(b) - rank(a)))
  const others = $derived(app.runtimes.filter((r) => !shown(r)).sort((a, b) => rank(b) - rank(a)))
  let showOthers = $state(false)

  async function install(r: RuntimeEntry) {
    if (await installRuntime(r)) onchange?.()
  }

  async function remove(r: RuntimeEntry) {
    if (!(await askConfirm(t('Delete {name}?', { name: `${r.label} ${r.major}` })))) return
    try {
      await api.deleteRuntime(r.id)
      if (app.settings.preferredJava === r.id) {
        app.settings.preferredJava = null
        await saveSettings()
      }
      await reloadRuntimes()
      onchange?.()
    } catch (e) {
      notify(e)
    }
  }

  async function toggleDefault(r: RuntimeEntry) {
    app.settings.preferredJava = app.settings.preferredJava === r.id ? null : r.id
    await saveSettings()
  }
</script>

{#snippet runtime(r: RuntimeEntry)}
  <div class="item">
    <div class="txt">
      <div class="n">
        {r.label} {r.major}
        {#if r.flavor === 'graalvm' && ESSENTIAL.has(r.id)}<span class="tag rec">{t('Recommended')}</span>{/if}
        {#if app.settings.preferredJava === r.id}<span class="tag def">{t('Default')}</span>{/if}
      </div>
      <div class="d">
        {#if app.javaBusy === r.id}
          {app.install ? `${stageText(app.install.stage)}${app.install.total > 0 ? ` · ${Math.round((app.install.done / app.install.total) * 100)}%` : ''}` : t('Installing…')}
        {:else if r.installed}
          {r.version} · {note(r)}
        {:else}
          {note(r)}
        {/if}
      </div>
    </div>
    <div class="acts">
      {#if r.installed}
        <button class:primary={app.settings.preferredJava !== r.id} onclick={() => toggleDefault(r)}>
          {app.settings.preferredJava === r.id ? t('Remove default') : t('Set as default')}
        </button>
        <button class="ghost" onclick={() => remove(r)}>{t('Delete')}</button>
      {:else}
        <button onclick={() => install(r)} disabled={app.javaBusy !== null}>{app.javaBusy === r.id ? t('Installing…') : t('Install')}</button>
      {/if}
    </div>
  </div>
{/snippet}

{#each main as r (r.id)}
  {@render runtime(r)}
{/each}

{#if others.length}
  <button class="ghost more" onclick={() => (showOthers = !showOthers)} aria-expanded={showOthers}>
    {showOthers ? t('Hide other versions') : t('Other Java versions ({n})', { n: others.length })}
  </button>
  {#if showOthers}
    <p class="muted small hint">{t('Only needed if a modpack asks for a specific Java.')}</p>
    {#each others as r (r.id)}
      {@render runtime(r)}
    {/each}
  {/if}
{/if}

<style>
  .item { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 16px 0; border-bottom: 1px solid var(--line); }
  .txt { min-width: 0; }
  .n { font-weight: 500; }
  .d { margin-top: 2px; color: var(--muted); font-size: 13px; line-height: 1.45; max-width: 520px; }
  .acts { display: flex; gap: 8px; align-items: center; flex: none; }
  .tag { margin-left: 8px; }
  .def { color: var(--text); }
  .rec { color: var(--inv-text); background: var(--inv-bg); border-color: var(--inv-bg); }
  .more { justify-self: start; margin-top: 10px; padding: 6px 12px; margin-left: -12px; font-size: 13px; color: var(--muted); }
  .more:hover:not(:disabled) { color: var(--text); }
  .hint { margin: 4px 0 0; }
</style>
