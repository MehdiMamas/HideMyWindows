# Changelog

All notable changes to this project are documented here.
This project adheres to [Semantic Versioning](https://semver.org/).

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
