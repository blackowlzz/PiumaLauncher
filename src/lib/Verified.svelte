<script lang="ts">
  import { t } from './i18n.svelte'

  let { size = 18 }: { size?: number } = $props()
</script>

<svg class="verified" width={size} height={size} viewBox="0 0 24 24" role="img" aria-label={t('Verified Premium account')}>
  <title>{t('Verified Premium account')}</title>
  <path fill="currentColor" d="M12 1.6l2.5 1.9 3.1-.2 1.2 2.9 2.7 1.6-.7 3 1.4 2.8-2.1 2.3-.4 3.1-3 .8-2 2.4-2.8-1.3-2.8 1.3-2-2.4-3-.8-.4-3.1L1.6 13l1.4-2.8-.7-3 2.7-1.6 1.2-2.9 3.1.2z" />
  <path fill="none" stroke="#fff" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" d="M7.8 12.3l3 3 5.6-6" />
</svg>

<style>
  .verified { flex: none; color: #7c4dff; vertical-align: -0.12em; filter: drop-shadow(0 0 6px rgba(124, 77, 255, 0.45)); }
</style>
