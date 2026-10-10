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

For the x64 app, also build the bundled x86 helper and payload. These are
required for release builds and for the native WOW64 integration test:

```bat
rustup target add i686-pc-windows-msvc
cargo build -p hmw-release -p hmw-payload --release --target i686-pc-windows-msvc
copy target\i686-pc-windows-msvc\release\hmw-release.exe src-tauri\resources\hmw-release-x86.exe
copy target\i686-pc-windows-msvc\release\hmw_payload.dll src-tauri\resources\hmw_payload_x86.dll
cargo test -p hmw-core -- --test-threads=1
```

The x64 app embeds both x86 files and extracts them into a version-specific
temporary directory. The helper runs without a console and shares the app's
permissions; elevated targets still require an elevated controller. The x86
helper also runs a separate message-pumping desktop window gate while the x64
app is running, so 32-bit apps can be opened normally. Its parent process handle
ensures the helper exits with the controller.

## 3. Run or build

```bash
# Hot-reloading dev build
npm run tauri dev

# Release build + installer (output under src-tauri/target/release/bundle/)
npm run tauri build
```

The installer lands in `src-tauri/target/release/bundle/nsis/`.

## Releases (CI)

Pushing a `v*` tag or an explicit `release/v*` branch runs
`.github/workflows/release.yml`. It first verifies the version and runs the
complete Windows check workflow, then builds x64, x86 and ARM64 installers on
GitHub's Windows runners. All three installers must succeed before publication.
For example, a `release/v2.1.1` branch builds version 2.1.1 and creates its tag at
the exact release commit. This also supports publishing through a GitHub
connection that can create branches but cannot push tags. You do not need every
toolchain locally.

Installed copies check that release for updates. Signing uses a minisign key
generated with `npm run tauri signer generate` and kept outside the repo. The
public key lives in `src-tauri/tauri.conf.json`. The private key and its
password are GitHub Actions secrets named `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. A release built without those secrets
does not publish `latest.json`, so the in-app updater has nothing to install.

## Project layout

```
hmw-core/     Rust: window affinity, process list, injection, rules, config
payload/      Rust cdylib injected into target processes (hmw_payload.dll)
src-tauri/    Tauri app: commands, tray, watcher loop, config, autostart
src/          Svelte UI (pages, components, i18n)
```
