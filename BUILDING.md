# Building HideMyWindows

HideMyWindows is a [Tauri 2](https://tauri.app) app: a Rust core with a Svelte
frontend. These steps build it on **Windows** (the only supported runtime OS).

## Prerequisites

- **Rust** (stable) — <https://rustup.rs>
- **Node.js** 18+ and npm
- **Microsoft C++ Build Tools** (MSVC) + the **WebView2 runtime** (preinstalled
  on Windows 10/11). Tauri's prerequisites guide covers these:
  <https://tauri.app/start/prerequisites/>

## 1. Install dependencies

```bash
npm install
```

## 2. Build the injected payload

The app bundles a tiny helper DLL (`hmw_payload.dll`) that it loads into target
processes. Build it for your architecture and copy it where the app expects it:

```bash
# x64 (most PCs)
cargo build -p hmw-payload --release --target x86_64-pc-windows-msvc
copy target\x86_64-pc-windows-msvc\release\hmw_payload.dll src-tauri\resources\hmw_payload.dll
```

For other architectures use `i686-pc-windows-msvc` (x86) or
`aarch64-pc-windows-msvc` (ARM64) and the matching `target\…` path.

## 3. Run or build

```bash
# Hot-reloading dev build
npm run tauri dev

# Release build + installer (output under src-tauri/target/release/bundle/)
npm run tauri build
```

The installer lands in `src-tauri/target/release/bundle/nsis/`.

## Releases (CI)

Pushing a `v*` tag runs `.github/workflows/release.yml`, which builds x64, x86
and ARM64 installers on GitHub's Windows runners and publishes them to a GitHub
release automatically — so you don't need every toolchain locally.

## Project layout

```
hmw-core/     Rust: window affinity, process list, injection, rules, config
payload/      Rust cdylib injected into target processes (hmw_payload.dll)
src-tauri/    Tauri app: commands, tray, watcher loop, config, autostart
src/          Svelte UI (pages, components, i18n)
```
