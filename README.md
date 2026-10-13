# PiumaLauncher

PiumaLauncher is a fast, lightweight and open source Minecraft launcher for Windows and Linux. It installs the right Java for you, plays on premium or offline accounts, and browses Modrinth for modpacks, mods, resource packs and shaders without leaving the app.

# https://www.piumalauncher.it/

<img width="1342" height="848" alt="immagine" src="https://github.com/user-attachments/assets/d4e5c6bb-a08f-4b60-8537-611711e9c093" />
<img width="1342" height="848" alt="immagine" src="https://github.com/user-attachments/assets/07c7084b-6db3-46fa-8564-812da32a874f" />
<img width="1342" height="848" alt="immagine" src="https://github.com/user-attachments/assets/a73bf252-759e-471d-bab0-5d573ecca64b" />
<img width="1342" height="848" alt="immagine" src="https://github.com/user-attachments/assets/4a23a111-3745-48ea-9eac-6a31afb93f4d" />



## Features

**Accounts**
- Sign in with your Microsoft account to play with your official Minecraft profile
- Offline accounts, each with its own skin head
- Real player skins with a 3D preview

**Instances**
- Vanilla, Fabric and Quilt, on any Minecraft version
- Per-instance memory, JVM profile (low usage, balanced or performance) and extra JVM arguments
- Mods on/off, world list, duplicate and delete from the instance page
- Live memory usage while the game runs

**Modrinth**
- Search modpacks, mods, resource packs and shaders with filters for version, loader and category
- One-click modpack install (`.mrpack`), with a details page showing contents, gallery and versions
- Mods install with their dependencies, and you get update, enable and disable actions per file

**Java**
- The right Java is downloaded automatically (GraalVM 21/25, Temurin 8/17) and verified with SHA-256
- Already have Java? PiumaLauncher detects it and you can pick it per instance

**Interface**
- Dark and light themes, a reduced-effects option and smooth scrolling
- Available in 16 languages: English, Italian, Spanish, French, German, Portuguese, Albanian, Hungarian, Russian, Bulgarian, Ukrainian, Polish, Romanian, Serbian, Greek and Turkish
- Tells you when a new version is out, so you can download it from the Releases page
- Home page with weekly playtime and the latest Minecraft news

## Installation

Download the latest version from the [Releases](../../releases) page.

| System | Files |
|---|---|
| Linux | `.AppImage` (runs anywhere), `.deb` (Debian, Ubuntu), `.rpm` (Fedora, openSUSE) |
| Windows | `.exe` installer or `.msi` |

You don't need to install Java. PiumaLauncher downloads Minecraft, its libraries and the right Java the first time you play, so the first launch needs an internet connection.

**Windows:** the installers are not code-signed yet, so SmartScreen may show a warning the first time. Choose "More info", then "Run anyway". Windows 10 and 11 already include WebView2, which the app needs. Windows support is newer than Linux support and has had less testing.

**Linux:** the `.deb` and `.rpm` packages install their own dependencies. For the AppImage, make it executable (`chmod +x PiumaLauncher*.AppImage`) and run it. If your system has no FUSE, start it with `--appimage-extract-and-run`.

## Getting started

1. Open **Accounts**. Choose **Sign in** under Microsoft, enter the code shown at `microsoft.com/link` and approve the sign-in. To play offline, type a player name instead.
2. Open **Library** and create an instance, or open **Explore** and install a modpack.
3. Press **Play**.

## Keyboard shortcuts

| Keys | Action |
|---|---|
| `Ctrl+K` or `/` | Search in Explore |
| `Ctrl+1` to `Ctrl+5` | Home, Library, Explore, Accounts, Settings |
| `Esc` | Close the open panel |

## Privacy and security

- Your Microsoft password is never typed into PiumaLauncher: you sign in on Microsoft's own page with a one-time code.
- Sign-in tokens stay on your computer, in `microsoft.json` (owner-only permissions on Linux). They are not shown in the interface.
- The game receives its access token through a private channel instead of the command line, so it does not appear in process lists or crash reports.
- Downloads are limited to Mojang, Modrinth and the official Java distributors, and file checksums are verified.
- At startup the launcher asks GitHub whether a newer release exists. Nothing else is sent, and you can turn it off in Settings under General.

## FAQ

**Where is my data stored?**
In `~/.local/share/PiumaLauncher` on Linux and `%APPDATA%\PiumaLauncher` on Windows. Settings has options to see disk usage, remove downloaded Java versions and reset everything.

**Can I play on online servers with an offline account?**
No. Servers that check accounts require a Microsoft sign-in. Offline accounts are for single player and offline servers.

**How do I update?**
When a new version is available, a banner appears at the top of the launcher with a Download button. Install the new version over the old one: your instances, accounts and settings are kept.

**Does it support Forge or NeoForge?**
Not yet. Fabric and Quilt are supported, and Quilt does not support Minecraft 26.x yet because its loader doesn't.

**Which version of a modpack gets installed?**
The newest release for the Minecraft version shown on the card. If that Minecraft version only has pre-release builds, the newest pre-release is installed. You can also pick any version from the modpack's details page.

## Building from source

You need stable Rust, Node 22.12 or newer and, on Linux, `webkit2gtk-4.1`.

```
npm install
npm run tauri dev
npm run tauri build
```

`npm run tauri dev` runs the app in development. `npm run tauri build -- --no-bundle` builds just the binary. Run the tests with `cargo test` inside `src-tauri`.

## Translating

Interface texts are written in English and used as keys, for example `t('Library')`. To add a language, copy `src/lib/lang/it.ts`, translate the values and register it in `src/lib/i18n.svelte.ts`. Missing entries fall back to English. Translations can contain mistakes, so fixes are welcome.

## License

GPL-3.0-or-later. See `LICENSE`.
