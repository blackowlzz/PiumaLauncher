import { mount } from 'svelte'
import '@fontsource-variable/inter-tight'
import './app.css'
import App from './App.svelte'
import ConsoleApp from './ConsoleApp.svelte'

const consoleFor = new URLSearchParams(location.search).get('console')
const app = consoleFor
  ? mount(ConsoleApp, { target: document.getElementById('app')!, props: { instanceId: consoleFor } })
  : mount(App, { target: document.getElementById('app')! })

export default app
