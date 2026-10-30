import { mount } from 'svelte'
import '@fontsource-variable/inter-tight'
import './app.css'
import App from './App.svelte'
import ConsoleApp from './ConsoleApp.svelte'
import Intro from './lib/Intro.svelte'
import { applyTheme } from './lib/palette.svelte'

const target = document.getElementById('app')!
const consoleFor = new URLSearchParams(location.search).get('console')

if (consoleFor) {
  mount(ConsoleApp, { target, props: { instanceId: consoleFor } })
} else {
  try {
    const mode = localStorage.getItem('piuma-mode')
    if (mode === 'light' || mode === 'dark') document.documentElement.dataset.theme = mode
  } catch {}
  applyTheme()
  mount(Intro, { target: document.body })
  requestAnimationFrame(() => setTimeout(() => mount(App, { target })))
}
