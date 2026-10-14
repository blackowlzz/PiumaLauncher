import { api, type Instance } from './api'
import type { MenuItem } from './contextmenu'
import { t } from './i18n.svelte'
import { app, askConfirm, createShortcut, isBusy, notify, openInstance, play, reloadInstances, stop } from './store.svelte'

export function instanceItems(inst: Instance, withPlay = true): MenuItem[] {
  const running = app.games[inst.id]?.state === 'running'
  const busy = isBusy(inst.id)
  const locked = busy || running

  const duplicate = async () => {
    try {
      const copy = await api.duplicateInstance(inst.id)
      await reloadInstances()
      notify(t('Created "{name}"', { name: copy.name }))
    } catch (e) {
      notify(e)
    }
  }

  const remove = async () => {
    if (!(await askConfirm(t('Delete "{name}" with all its worlds and mods?', { name: inst.name })))) return
    try {
      await api.deleteInstance(inst.id)
      if (app.selectedId === inst.id) app.selectedId = null
      await reloadInstances()
    } catch (e) {
      notify(e)
    }
  }

  const shortcut = async () => {
    if (await createShortcut(inst.id)) notify(t('Shortcut for "{name}" created', { name: inst.name }))
  }

  const head: MenuItem[] = withPlay
    ? [
        running
          ? { label: t('Stop'), icon: 'x', onclick: () => void stop(inst.id) }
          : { label: t('Play'), icon: 'play', onclick: () => void play(inst.id), disabled: busy || !app.accounts.active },
      ]
    : []

  return [
    ...head,
    { label: t('Details'), icon: 'settings', onclick: () => openInstance(inst.id) },
    { label: t('Open instance folder'), icon: 'folder', onclick: () => void api.openFolder(inst.id, 'game').catch(notify) },
    { label: t('Share'), icon: 'share', onclick: () => (app.sharingId = inst.id) },
    { label: t('Create shortcut'), icon: 'shortcut', onclick: shortcut },
    { label: t('Duplicate'), icon: 'file', onclick: duplicate, disabled: locked },
    'sep',
    { label: t('Delete instance'), icon: 'trash', onclick: remove, danger: true, disabled: locked },
  ]
}
