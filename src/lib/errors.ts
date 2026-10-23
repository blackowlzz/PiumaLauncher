const TEMPLATES = [
  "No version compatible with Minecraft {mc}",
  "This version is not compatible with Minecraft {mc} on this instance",
  "This version requires Java {required}, but you have Java {current}",
  "The selected Java doesn't work: {path}",
  "Java {required} or newer is required and couldn't be obtained{reason}",
  "{loader} doesn't support Minecraft {mc} yet",
  "{loader} doesn't support version {mc}",
  "Unknown Minecraft version: {mc}",
  "Couldn't close the Minecraft still running for this instance (PID {list}). End it from Task Manager and try again",
  "Native libraries in {folder} are in use. Close any Minecraft still running for this instance (check Task Manager for javaw.exe) and try again",
  "Windows refused to start Java ({path}). Your antivirus may be blocking it, or pick another Java in the instance settings",
  "The launch wrapper \"{name}\" couldn't be started: {error}",
  "The loader installer failed: {detail}",
  "Sign-in failed ({status})",
  "Couldn't download the update: {error}",
  "Couldn't install the update: {error}",
  "Couldn't open the folder ({opener}): {error}",
  "Couldn't open the link ({opener}): {error}",
  "Temurin {major} isn't available for this system",
]

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

const compiled = TEMPLATES.map((key) => {
  const names: string[] = []
  const pattern = key
    .split(/\{(\w+)\}/)
    .map((part, i) => (i % 2 ? (names.push(part), '(.*?)') : escape(part)))
    .join('')
  return { key, names, re: new RegExp(`^${pattern}$`, 's') }
})

export function errorKey(text: string): { key: string; params?: Record<string, string> } {
  for (const { key, names, re } of compiled) {
    const m = text.match(re)
    if (m) return { key, params: Object.fromEntries(names.map((n, i) => [n, m[i + 1]])) }
  }
  return { key: text }
}
