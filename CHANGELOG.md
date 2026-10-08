# Changelog

All notable changes to this project are documented here.
This project adheres to [Semantic Versioning](https://semver.org/).

## [2.1.1] — 2026-10-08

### Fixed
- **Work around black rectangles when minimizing a capture-hidden window while
  another app has focus.** Protected windows now disable their own DWM
  minimize/restore transitions before capture exclusion is applied. Capture
  exclusion remains active throughout minimizing and restoring. Unhiding a
  window restores normal transitions; Windows' global animation settings are
  unchanged. The workaround covers window hides, process hides, automatic
  normal-launch protection, toast hides and the controller's own windows.
- Repeated hide requests avoid resetting display affinity or reapplying the
  transition override. Failed exclusion rolls back newly applied overrides.
- Windows regression checks exercise inactive minimizing through synchronous,
  asynchronous and system-command paths, restore, repeated hides and unhide
  cleanup on x64 and x86. Existing pre-show and desktop-capture checks also
  verify transition suppression on newly protected windows.
- Releases now run the full Windows validation workflow before building and
  publishing installers. Explicit `release/v*` branches can start a release.

### Compatibility
- Restart protected apps after updating to load the new payload. Capture-hidden
  windows minimize and restore without DWM transition animations. The local
  artifact itself is excluded from screenshots, so regression checks verify
  the transition policy and uninterrupted protection; a physical Windows 11
  desktop is needed to confirm the original intermittent visual symptom.

## [2.1.0] — 2026-10-08

### Added
- **Open apps normally with automatic recording exclusion.** While
  HideMyWindows runs, a synchronous desktop window-creation gate checks enabled
  hide rules inside supported apps. It holds initial visibility until matching
  windows have verified capture exclusion, then shows them to you. Quick Launch
  is optional, and nonpersistent rules also cover newly created windows.
- **32-bit normal launches in the x64 build.** A bundled x86 helper maintains
  its own gate and follows saved rules. It exits when the controller exits.
- **Blocked-window recovery and reporting.** Failed decisions keep the window
  locally invisible. Repairs replay pending show requests; removing rules or
  stopping the gate restores affinity it applied. Invisible failure properties
  appear in Window rules alongside existing rule results.
- Native tests exercise normal x64/x86 launches without watcher injection,
  pre-show affinity, actual desktop captures, malformed-policy recovery and
  shutdown. Existing process-hide and Quick Launch capture tests remain.

### Compatibility
- Run HideMyWindows before opening matching apps. Restart target apps after
  updating to replace older payloads. Prefer process-name rules for first-window
  protection; titles assigned after visibility cannot be known beforehand.
- The gate covers hookable apps on the current desktop at accessible privilege
  levels and with a matching architecture (native x86/ARM64; x64 plus x86 in the
  x64 build). Windows can block protected or elevated apps. Existing windows,
  native API bypasses, alternate renderers and capture methods that ignore
  display affinity do not have a universal first-frame guarantee.
- Saved x86 rule changes take up to 200 ms to reach the helper. Gate policies
  are limited to 64 KiB; installation/update failures are reported explicitly.

## [2.0.10] — 2026-10-07

### Fixed
- **Pre-show protection for process hides.** Protected processes intercept
  standard Windows creation, show, asynchronous show and positioning paths,
  applying and verifying capture exclusion before visibility. App attempts to
  reset affinity remain excluded. Polling remains a recovery mechanism.
  Windows whose protection cannot be confirmed stay locally invisible, with
  failures reported in the rule result.
- **Quick Launch stops on protection failure.** The primary thread is resumed
  only after the new payload reports readiness and process protection is armed.
  A failed protected launch is terminated while still suspended. Later blocked
  windows are also reported for Quick Launch processes.
- **Faster rule discovery.** Window events wake discovery promptly, and
  process-name/PID rules can protect UI processes before their first window.
  Background services and other user sessions are omitted.

### Compatibility
- Restart target apps after updating to replace older, polling-only payloads.
- Ordinary launches still have a race before rule discovery; use Quick Launch
  and process-level hiding for the strongest protection. Window title/class
  rules cannot know a future window before it exists. Tests cover x64/x86
  desktop captures through the standard APIs; alternate renderers, native API
  bypasses and windows in separately launched child processes need separate
  validation/protection.

## [2.0.9] — 2026-10-07

### Fixed
- **32-bit apps work from the x64 build.** Hide/unhide, process rules, Quick
  Launch, and taskbar actions automatically use a bundled x86 helper and
  payload. Switching builds is no longer necessary for apps like AnyDesk.
  Uninstall cleanup still releases those targets.
- **One live rule result instead of stacked errors.** Window rules shows the
  number of matched windows Windows confirms are hidden, unknown states, and
  all current failures together. Repeated failures stay quiet; changed issues
  replace the existing warning and recovery clears it.
- Resolve actions from the payload actually loaded in the target, so a running
  payload from an earlier version does not use a newer DLL's export offsets.

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
