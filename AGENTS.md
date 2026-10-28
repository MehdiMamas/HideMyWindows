# AGENTS.md

## What it is
HideMyWindows: a Windows-only desktop app (Tauri 2, Rust core, Svelte 5 UI) that hides windows from screen capture, screenshots and recordings. It sets display affinity directly for its own windows and injects a small DLL into other processes for theirs. `README.md`, `BUILDING.md`, `ARCHITECTURE.md` and `CONTRIBUTING.md` hold the details.

## Setup
- Rust (stable), Node.js and npm, MSVC C++ Build Tools, WebView2 (see `BUILDING.md`).
- `npm install` (CI uses `npm ci`).
- Build the payload DLL before running the app, then copy it to `src-tauri/resources/`:
  `cargo build -p hmw-payload --profile payload --target x86_64-pc-windows-msvc` and copy `target\x86_64-pc-windows-msvc\payload\hmw_payload.dll` to `src-tauri\resources\hmw_payload.dll`.
- The x64 app also needs the x86 helper and payload (`hmw-release-x86.exe`, `hmw_payload_x86.dll`); the exact commands are in `BUILDING.md` and `.github/workflows/check.yml`.

## Commands
- `npm run tauri dev`: hot-reloading dev build. `npm run tauri build`: release build and installer (under `src-tauri/target/release/bundle/`).
- `npm run build`: frontend build. `npm test`: Vitest.
- `cargo test --locked -p hmw-core -- --test-threads=1`: core tests (run on Windows).
- `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings`: must be clean.

## Where things live
- `hmw-core/`: all Win32 logic (affinity, process and window listing, injection, rules, config, watcher). No UI.
- `payload/`: `cdylib` injected into target processes (`hmw_payload.dll`).
- `hmw-release/`: standalone cleanup and the bundled x86 helper.
- `src-tauri/`: thin Tauri layer (commands, tray, autostart, config, `tauri.conf.json`).
- `src/`: Svelte UI (`lib/pages`, `lib/components`, `lib/locales/*.json`, `lib/i18n.js`).
- `.claude/skills/release/`: the release procedure for this repo.

## Workflow and conventions
- CI (`check.yml`, on PRs and pushes to `main`): frontend build and tests, x64/x86/ARM64 payload builds, rustfmt, core tests, clippy, app build.
- Releases (`release.yml`): a `v*` tag or a `release/v*` branch builds x64, x86 and ARM64 installers; all three must succeed. `CHANGELOG.md` tracks releases.
- Keep Win32 logic in `hmw-core`; keep the Tauri layer thin. Add config fields with `#[serde(default)]` so the config format does not break.
- New language: copy `src/lib/locales/en.json` and register it in `src/lib/i18n.js`.

## Gotchas
- Windows is the only supported runtime; most native tests and builds need Windows.
- The payload uses the `payload` profile (`panic = "unwind"`) because it runs inside other apps.
- The updater needs signing secrets in GitHub Actions (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`); without them no `latest.json` is published.
- A release needs the same `X.Y.Z` in `package.json`, `src-tauri/tauri.conf.json`, `[workspace.package]` in `Cargo.toml` and the tag, plus a `## [X.Y.Z]` section in `CHANGELOG.md`; follow `.claude/skills/release/SKILL.md`. Lock files change only through their tools.
