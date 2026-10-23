import type { CrashCause } from './api'

export const causes: Record<CrashCause, [string, string]> = {
  outOfMemory: ['The game ran out of memory', 'Minecraft used all the memory it was given. Giving it more usually solves this.'],
  heapTooLarge: ['Not enough memory to start', "Java couldn't reserve the memory set for this instance. Lower it or close other programs."],
  javaVersion: ['Wrong Java version', 'This game or one of its mods needs a newer version of Java.'],
  jvmArgs: ['Invalid Java option', "Java didn't recognize one of the options it was given."],
  missingDeps: ['Some mods are missing', 'Other mods need these to work. They can be installed from Modrinth.'],
  incompatibleMods: ['Incompatible mods', "These mods don't work with this version of the game or with each other."],
  duplicateMods: ['Duplicate mods', 'The same mod is installed more than once. The older copies can be turned off.'],
  mixin: ['A mod failed to load', "A mod couldn't change the game, usually because it was made for another version."],
  suspectedMod: ['A mod caused the crash', 'The crash report points to these mods. Try turning them off.'],
  graphics: ['Graphics driver problem', "Your graphics driver couldn't start OpenGL. Updating the GPU driver usually fixes it."],
  nativeCrash: ['Java crashed', 'The Java runtime itself stopped. Updating the graphics driver or trying another Java can help.'],
  unknown: ['Unknown cause', 'No known problem was recognized. The details below can help you find it.'],
}
