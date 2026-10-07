# Changelog

All notable changes to this project are documented here.
This project adheres to [Semantic Versioning](https://semver.org/).

## [2.0.8] — 2026-10-07

### Added
- **Red dots on the hidden apps' own title bars.** While HideMyWindows runs,
  a small red dot appears beside each capture-excluded window's heading,
  including custom title bars. It follows the target as it moves, scales with
  DPI, stays underneath covering windows, and disappears on unhide, minimize,
  or close. The dot is click-through and excluded from screen capture itself.
  Borderless fullscreen windows and notification popups do not get title dots.

## [2.0.7] — 2026-10-07

### Fixed
- **Hidden badges belong to the targets.** The Home list now marks windows and
  processes that Windows reports as hidden from capture. Processes with a mix
  of hidden and visible windows show **Partly hidden**; unverifiable states show
  **Status unknown**. Status refreshes after actions and every second, including
  changes from automatic rules and Quick Launch. The app-header badge is removed.

## [2.0.6] — 2026-10-06

### Added
- **Capture status in the app header.** A small indicator shows whether this
  HideMyWindows window is currently excluded from capture, using the actual
  Windows display-affinity state. It refreshes when settings change, when the
  window regains focus, and every second. An unavailable check is shown explicitly.

## [2.0.4] — 2026-10-06

### Added
- **Updates from GitHub releases.** The app checks for a newer release on
  startup and asks before downloading and installing it. About has a Check
  for updates button for the same flow.

## [2.0.3] — 2026-10-06

### Added
- **Reset all settings.** Settings can restore factory defaults and delete
  settings left by older versions, including saved rules and quick launch
  entries. The old `HideMyWindows.json` file is removed when it is still
  present.

### Changed
- **Start with Windows opens in the tray.** Sign-in no longer shows the main
  window. Click the tray icon to open it. Opening the app yourself still
  shows the window.

## [2.0.2] — 2026-10-05

### Fixed
- **Uninstall now restores windows that were still hidden.** Capture
  exclusion and taskbar buttons stay inside the other apps after
  HideMyWindows is removed, because the injected helper keeps running there.
  The uninstaller releases those hides first, including hooks left by the
  original app (`HideMyWindows.DLL.x64.dll` and `HideMyWindows.DLL.Win32.dll`).
  It also removes the autostart entry, saved settings, and old helper copies
  in Temp. `hidemywindows.exe --release-all` does the same thing if the app
  was already uninstalled.

## [2.0.1] — 2026-10-05

### Changed
- **HideMyWindows no longer hides its own window from screen capture by
  default.** The `hideSelf` setting now defaults to off, so the app stays
  visible to remote-desktop and capture tools (e.g. AnyDesk) while it continues
  to hide the apps you target. Previously the default hid HMW itself, which
  could leave its window — and the toggle to change this — invisible over a
  remote session. Turn the setting back on in Settings if you want the old
  behaviour.

## [2.0.0] — 2026-10-05

A complete, community-maintained rewrite that **keeps the HideMyWindows name and
purpose** while rebuilding the app on a lighter, more maintainable foundation.
The original app by Cristian Gambino (@zCri) was no longer being updated.

### Changed
- **Rebuilt on Tauri 2 (Rust) + Svelte 5** instead of .NET/WPF. Installers are
  now a few MB instead of hundreds, and start-up is faster.
- **New native installer** (NSIS) with per-user and per-machine modes, plus a
  portable executable — built for **x64, x86 and ARM64**.
- Modern, lightweight UI with dark / light / system themes and the same five
  languages (EN, FR, IT, RO, PL).

### Added
- Start-with-Windows option.
- Close-to-tray / minimize-to-tray behaviour.
- GitHub Actions workflow that builds all three architectures and publishes
  releases automatically.

### Fixed / improved over the original
- **No administrator rights required.** Automatic rules use lightweight polling
  instead of WMI, removing the "needs admin" warnings and the WMI failure modes.
- **Simpler, more robust hiding.** Replaced the mailslot IPC, runtime PE
  export-parsing and MinHook API-detours with a small, well-typed injection path
  and a background re-apply thread in the payload. Fewer moving parts, fewer
  crashes.
- Cleaner error reporting surfaced as in-app toasts.

### Removed
- The experimental Direct3D desktop-preview page (heavy and unstable). May
  return in a future release if there's demand.
- Microsoft Store / MSIX packaging (the DLL-in-temp workaround it required is no
  longer needed).

### Notes
- This version injects into processes of the **same architecture** as the build
  you install. Install the x64 build on a 64-bit PC for the widest coverage.

### Credits
Original app and name by [@zCri](https://github.com/zCri); early help by
[@ad2017gd](https://github.com/ad2017gd) and [@minhprovjp](https://github.com/minhprovjp).
