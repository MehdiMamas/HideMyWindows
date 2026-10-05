# Architecture

A short tour of how HideMyWindows 2.0 is put together, and why.

## Goals of the rewrite

The original app was a .NET/WPF application with a C++ injection DLL, a MinHook
API-hooking layer, a mailslot IPC channel and WMI-based process watching. It
worked, but it was heavy and several moving parts were fragile. This rewrite
keeps the same capabilities while being:

- **Lighter** — a Tauri 2 (Rust) core + Svelte UI. Installers are a few MB.
- **Simpler** — no mailslot IPC, no PE export-table parsing at runtime, no
  MinHook detours, no WMI.
- **Easier to maintain** — the risky Win32 logic lives in one small, well-typed
  crate (`hmw-core`) that is unit-checkable on its own.

## Crates

| Crate | Responsibility |
| --- | --- |
| `hmw-core` | All Win32 logic: setting display affinity, listing processes/windows, injecting the payload and calling its exports, matching window rules, launching hidden, config load/save. No UI, no Tauri. |
| `payload` | A `cdylib` (`hmw_payload.dll`) injected into target processes. Exposes `HmwHideAll` / `HmwUnhideAll` / `HmwHideWindow` / `HmwUnhideWindow` / `HmwHideTray` / `HmwUnhideTray`. |
| `src-tauri` | The Tauri app: exposes `hmw-core` as commands, owns config + state, builds the tray, and runs the rule-watcher loop. Thin wiring. |
| `src/` | Svelte 5 UI: Dashboard, Quick launch, Rules, Settings, About, plus i18n. |

## Hiding a window from capture

Windows exposes `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)`, which
tells the compositor to exclude a window from screen capture. The catch: it must
be called **by a thread inside the window's owning process**.

- **Our own windows** → we call it directly (`hmw_core::window::set_capture_hidden`).
- **Another app's windows** → we load `hmw_payload.dll` into that process and
  ask it to call the API from inside.

## Injecting the payload

A classic, well-documented loader — no API hooking:

1. `OpenProcess` with the rights needed to allocate memory and create a thread.
2. Write the payload's path into the target with `VirtualAllocEx` +
   `WriteProcessMemory`.
3. `CreateRemoteThread` starting at `LoadLibraryW` so the target loads the DLL.
4. Find the payload's base address in the target (`CreateToolhelp32Snapshot`),
   add the export's RVA (read once from a locally-loaded copy), and
   `CreateRemoteThread` at that address. The HWND an action needs is passed
   directly as the thread parameter — no extra remote allocation.

The payload starts a lightweight background thread that re-applies the
"hide all" state, so windows created later are hidden too. This replaces the old
`CreateWindowEx` API hook with something far simpler and more robust.

> **Same-architecture only (v2):** we inject into processes whose architecture
> matches the installed build (x64→x64, x86→x86, arm64→arm64). Cross-architecture
> injection is intentionally out of scope and surfaces a clear message instead of
> failing obscurely.

## Automatic rules

`hmw_core::watcher::apply_rules_once` enumerates visible top-level windows and
applies matching rules. The Tauri layer calls it on a timer:

- a frequent pass for **persistent** rules (keeps new windows hidden), and
- a slower discovery pass for all rules.

This polling approach needs **no administrator rights** (the old WMI watcher
sometimes did).

## Config

A single JSON file at `%APPDATA%\HideMyWindows\config.json`, (de)serialized with
`serde`. Unknown/missing fields fall back to defaults, so old files keep working.
