# Architecture

A short tour of how HideMyWindows 2.0 is put together, and why.

## Goals of the rewrite

The original app was a .NET/WPF application with a C++ injection DLL, a MinHook
API-hooking layer, a mailslot IPC channel and WMI-based process watching. It
worked, but it was heavy and several moving parts were fragile. This rewrite
keeps the same capabilities while being:

- **Lighter** — a Tauri 2 (Rust) core + Svelte UI. Installers are a few MB.
- **Simpler** — no mailslot IPC, no
  MinHook detours, no WMI.
- **Easier to maintain** — the risky Win32 logic lives in one small, well-typed
  crate (`hmw-core`) that is unit-checkable on its own.

## Crates

| Crate | Responsibility |
| --- | --- |
| `hmw-core` | All Win32 logic: setting display affinity, listing processes/windows, injecting the payload and calling its exports, matching window rules, launching hidden, config load/save. No UI, no Tauri. |
| `payload` | A `cdylib` (`hmw_payload.dll`) injected into target processes. Exposes `HmwHideAll` / `HmwUnhideAll` / `HmwHideWindow` / `HmwUnhideWindow` / `HmwHideTray` / `HmwUnhideTray`. |
| `hmw-release` | Standalone cleanup, plus the bundled x86 helper that performs hide/unhide actions for the x64 controller. |
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

The loader uses the standard remote-thread mechanism:

1. `OpenProcess` with the rights needed to allocate memory and create a thread.
2. Write the payload's path into the target with `VirtualAllocEx` +
   `WriteProcessMemory`.
3. `CreateRemoteThread` starting at `LoadLibraryW` so the target loads the DLL.
4. Find the payload's base address in the target (`CreateToolhelp32Snapshot`),
   resolve the export from that module's own PE export table, and
   `CreateRemoteThread` at that address. The HWND an action needs is passed
   directly as the thread parameter — no extra remote allocation.

Process hiding installs a pinned Microsoft Detours engine (MIT, x86/x64/ARM64)
for CreateWindowExW/A, ShowWindow/ShowWindowAsync, SetWindowPos,
DeferWindowPos and SetWindowDisplayAffinity. Creation strips initial WS_VISIBLE
until capture exclusion is verified; show paths refuse visibility on failure.
A 300 ms worker remains for recovery, including safely replaying blocked show
requests once protection succeeds. Hooks remain installed but pass through
while process hiding is disabled.

Quick Launch creates the process suspended, requires HmwPrepareProtection and
HmwHideAll to finish successfully, then resumes the primary thread. Failure
terminates that launch. Older injected DLLs without the readiness export require
a target restart. Capture tests include pre-visibility affinity checks and actual
GDI desktop frames, with visible controls, on x64/x86. This does not guarantee
all capture/rendering APIs or separately launched child processes.

The x64 controller routes x86 targets through its bundled x86 helper and x86
payload. Each injector still runs in the same architecture as its target.
The x86 and ARM64 builds handle their native targets. Resolving exports from
the remote DLL also supports payloads left loaded by an earlier app version.

## Automatic rules

`hmw_core::watcher::RuleSession` enumerates visible top-level windows and
applies matching rules. The Tauri layer calls it on a timer:

- a frequent pass for **persistent** rules (keeps new windows hidden), and
- a slower discovery pass for all rules.

The controller retains one rule-status snapshot, reads capture affinity to
count verified hides, and emits changes. The frontend keeps one notification
for changed failures and shows all details in Window rules.

WinEvent creation/show notifications wake discovery; timers remain a fallback.
Process-name/PID rules enumerate processes in the current session before windows
appear. Events and polling remain fallback discovery for existing windows.

A desktop-wide, same-architecture WH_CBT hook now intercepts ordinary window
creation synchronously in the owning process. It temporarily removes initial
WS_VISIBLE and installs a same-thread subclass. A session-local shared-memory
snapshot supplies enabled capture-hide rules, using the same comparator logic
as the controller. WM_CREATE replay, WM_SHOWWINDOW and WM_WINDOWPOSCHANGING
verify exclusion before allowing the window to show. Matched processes install
the existing API detours to prevent subsequent affinity resets. Torn or invalid
snapshots hold show requests; a recovery worker retries and restores the prior
affinity when rules are removed or the controller exits. The controller reports
failure properties on invisible windows too.

The installing thread pumps messages; an x86 helper owns a separate x86 hook in
the x64 build and follows saved rule changes every 200 ms. It retains a parent
process handle and exits with the controller. Named mutexes prevent two writers
of an architecture's bounded 64 KiB policy; reader retries never block apps on
the controller. Hook callbacks and subclasses are outside DllMain, and target
modules are pinned until process exit to keep outstanding callbacks valid.

Coverage is limited to hookable desktop apps of the gate's architecture on the
current desktop and at accessible integrity levels. Existing windows, titles
assigned after visibility, protected apps, alternative renderers and native API
bypasses do not have a universal first-frame guarantee. Elevated targets still
need matching privileges. Native tests exercise normal x64/x86 launches, actual
GDI capture frames, pre-show affinity, policy failure/recovery and gate shutdown.

## Config

A single JSON file at `%APPDATA%\HideMyWindows\config.json`, (de)serialized with
`serde`. Unknown/missing fields fall back to defaults, so old files keep working.
