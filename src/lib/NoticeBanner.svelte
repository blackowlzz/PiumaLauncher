<script lang="ts">
  import { onMount } from 'svelte'
  import { slide } from 'svelte/transition'
  import Icon from './Icon.svelte'
  import { api, type Notice } from './api'
  import { i18n, t } from './i18n.svelte'
  import { notify } from './store.svelte'

  const KEY = 'piuma-notice-dismissed'
  let notice: Notice | null = $state(null)
  let dismissed: string | null = $state(stored())

  function stored() {
    try {
      return localStorage.getItem(KEY)
    } catch {
      return null
    }
  }

  async function load() {
    notice = await api.notice(i18n.lang).catch(() => null)
  }

  $effect(() => {
    if (i18n.ready && i18n.lang) void load()
  })

  onMount(() => {
    const timer = setInterval(load, 30 * 60 * 1000)
    return () => clearInterval(timer)
  })

  function dismiss() {
    if (!notice) return
    dismissed = notice.id
    try {
      localStorage.setItem(KEY, notice.id)
    } catch {}
  }

  function open() {
    if (notice?.link) api.openUrl(notice.link).catch(notify)
  }
</script>

{#if notice && notice.id !== dismissed}
  <div class="banner" role="alert" transition:slide={{ duration: 320 }}>
    <span class="ic"><Icon name="alert" size={18} /></span>
    <div class="txt">
      <strong>{notice.title}</strong>
      {#if notice.description}<span class="muted">{notice.description}</span>{/if}
    </div>
    {#if notice.link}<button class="primary" onclick={open}>{t('Learn more')}</button>{/if}
    <button class="ghost close" onclick={dismiss} aria-label={t('Close')} title={t('Close')}><Icon name="x" size={14} /></button>
  </div>
{/if}

<style>
  .banner { display: flex; align-items: center; gap: 14px; padding: 12px 16px 12px 24px; border-bottom: 1px solid var(--line); background: var(--surface-2); flex: none; }
  .ic { display: grid; place-items: center; width: 34px; height: 34px; border-radius: 50%; background: var(--inv-bg); color: var(--inv-text); flex: none; animation: pulse 2.4s ease-in-out 500ms 3; }
  .txt { display: flex; flex-direction: column; gap: 2px; flex: 1; min-width: 0; }
  .muted { white-space: pre-line; line-height: 1.45; }
  .close { display: grid; place-items: center; padding: 6px; flex: none; }
  @keyframes pulse { 50% { box-shadow: 0 0 0 6px rgba(var(--glow), 0.12); } }
</style>
