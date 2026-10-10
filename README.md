# 🪟 HideMyWindows

> 🌐 Available in: [English](README.md) | [Français](README.fr.md) | [Italiano](README.it.md) | [Română](README.ro.md) | [Polski](README.pl.md)

![Banner](Assets/Banner.png)

**HideMyWindows** is a Windows app that **hides your windows from screen capture** — screenshots, screen recordings and streaming software like OBS.
It's built for **privacy-minded users, streamers and students** who want control over what others see when they share their screen.

> ℹ️ **About this version — rebuilt and improved by [Mehdi](https://github.com/mehdimamas).**
> The original HideMyWindows by [Cristian Gambino (@zCri)](https://github.com/zCri) was no longer being updated. This **2.0** release **keeps the same name and purpose** and rebuilds the app from scratch on a lighter, more maintainable foundation. Credit for the original idea and app goes to its original author — see [Credits](#-credits).

---

## ✨ Features

- 🔒 **Hide windows from screenshots & recordings** using the Windows `SetWindowDisplayAffinity` protection (`WDA_EXCLUDEFROMCAPTURE`).
- 🛡️ **Protection for normal app launches** — while HideMyWindows runs, supported desktop windows are checked before showing. Matching windows receive verified capture exclusion, then appear on your screen; failed checks keep them locally invisible for retry. Open apps as usual; Quick Launch is optional. The x64 build includes a separate x86 gate. Elevated, protected, other-desktop and foreign-architecture apps have Windows coverage limits.
- 🔴 **Red dot on hidden windows’ title bars** while HideMyWindows runs — see which windows are protected without opening the app. The dot stays out of captures and does not block clicks.
- 🎯 **Target by process, window, title, class or PID** — with `contains`, `starts/ends with`, `equals` and **regex** matching.
- ⚡ **Automatic window rules** — hide apps the moment they appear, and optionally keep re-applying so new windows stay hidden.
- 🚀 **Quick launch** — start an app already hidden, before it ever draws on screen.
- 🖥️ **Hide the taskbar button** of any app.
- 🪶 **Tiny & native** — a Tauri 2 (Rust) core with a Svelte UI. The installer is a few MB, not hundreds.
- 🔄 **Runs in the tray**, optional **start with Windows**, dark / light / system themes.
- 🌍 **Localized**: English, Français, Italiano, Română, Polski.

---

## 📥 Installation

### GitHub Releases
Download the latest installer for your architecture from the [**Releases page**](../../releases):

| Your PC | Download |
| --- | --- |
| 64-bit Intel/AMD (most PCs) | `HideMyWindows_x64-setup.exe` |
| 32-bit Windows | `HideMyWindows_x86-setup.exe` |
| ARM64 (e.g. Surface Pro X) | `HideMyWindows_arm64-setup.exe` |

Run the installer and launch HideMyWindows. No admin rights required.

> **Note on architecture:** install the x64 build on a 64-bit Intel/AMD PC. It handles both 64-bit and 32-bit apps automatically using its bundled x86 helper. The x86 build is for 32-bit Windows; the ARM64 build handles native ARM64 apps.

### Build from source
See [BUILDING.md](BUILDING.md).

---

## 🚀 Usage

1. **Home** — pick a running process (or a specific window) and click **Hide all windows** / **Hide this window**.
2. **Quick launch** — add the apps you hide most often and start them already hidden with one click.
3. **Window rules** — create and enable a process-name rule, then open that app normally. New supported windows are checked before showing, even without **Keep re-applying**. That option also reconciles existing windows on a timer. Process-name rules are preferable when a title is set after the window appears.
4. **Settings** — hide HideMyWindows itself, run in the tray, start with Windows, pick a theme and language. Under **Protection before apps appear**, turn ordinary-launch protection off or pause it for 15, 30, 60, or 120 minutes. **Resume now** ends a pause early.

![Thumbnail](Assets/Thumbnail.png)

---

## ⚙️ How it works

- HideMyWindows applies the Windows **`SetWindowDisplayAffinity`** capture-protection flag to the windows you choose. The same mechanism is used by password managers and DRM-protected apps to keep their own windows out of screen captures.
- Its **own** windows are protected with a direct call. To protect **another** app's windows, a small helper library (`hmw_payload.dll`) is loaded into that app so the flag can be set from inside it — because Windows only lets a window's own process set this flag.
- Automatic rules use a synchronous desktop window hook plus events and polling for existing or unsupported windows. The app must already be running. Elevated targets require an elevated controller; Windows can prevent interception of protected apps. A title that changes after visibility can only be discovered afterward.

### Game compatibility and timed pauses

Before launching a game, right-click the HideMyWindows tray icon and choose
**Pause protection before apps appear**, then pick 15, 30, 60, or 120 minutes.
The menu also offers enable/disable and **Resume now**, and shows the applied
status and remaining pause time. The same controls remain in Settings. Wait until the status says **paused** before launching. Both the native
and 32-bit global window hooks stop; the timer resumes them automatically even
while HideMyWindows is in the tray. The pause deadline is saved, so restarting
HideMyWindows preserves the remaining pause. Turning the setting off leaves it
off until you enable it again.

While off or paused, rule polling, targeted hiding, notification protection, and
Quick Launch still work. Ordinary launches no longer have a first-frame capture
protection guarantee. Avoid rules that target the game or its launcher. A pause
cannot unload a DLL already loaded into another app; restart the game if it was
already running. Resuming reinstalls desktop hooks even if a game is still open,
so choose a pause long enough for your session or turn the setting off.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full picture.

---

## 🧰 Tech stack

| Part | Technology |
| --- | --- |
| Core | [Tauri 2](https://tauri.app) + Rust (`windows` crate) |
| UI | [Svelte 5](https://svelte.dev) + Vite |
| Installer | NSIS (per-user & per-machine), portable exe |
| Arch support | x64, x86, ARM64 |

---

## 📜 License

Licensed under **MIT with the Commons Clause** — you may use, modify and share it freely, but you may not **sell** it. See [LICENSE.txt](LICENSE.txt).

---

## 🙏 Credits

- **Original app** by [Cristian Gambino (@zCri)](https://github.com/zCri) — the idea, the name and the first versions.
- Early development help by [@ad2017gd](https://github.com/ad2017gd), and contributions from [@minhprovjp](https://github.com/minhprovjp) and others.
- The white-box / tray-icon fixes from the original project are carried forward in spirit.
- **Rebuilt and improved by [Mehdi](https://github.com/mehdimamas)** on [Tauri](https://tauri.app) and [Svelte](https://svelte.dev).

This project exists only because of the original author's work. Thank you. 💙

---

## 🤝 Contributing

Issues and pull requests are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md).
