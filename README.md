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

> **Note on architecture:** this version hides apps that run on the **same architecture** as the build you install. On a 64-bit PC, install the x64 build (it covers the vast majority of modern apps). The x86 build exists for 32-bit Windows.

### Build from source
See [BUILDING.md](BUILDING.md).

---

## 🚀 Usage

1. **Home** — pick a running process (or a specific window) and click **Hide all windows** / **Hide this window**.
2. **Quick launch** — add the apps you hide most often and start them already hidden with one click.
3. **Window rules** — create rules (e.g. *process name contains "obs"* → *hide all windows*) to auto-hide apps as they start. Turn on **Keep re-applying** for apps that open new windows over time.
4. **Settings** — hide HideMyWindows itself, run in the tray, start with Windows, pick a theme and language.

![Thumbnail](Assets/Thumbnail.png)

---

## ⚙️ How it works

- HideMyWindows applies the Windows **`SetWindowDisplayAffinity`** capture-protection flag to the windows you choose. The same mechanism is used by password managers and DRM-protected apps to keep their own windows out of screen captures.
- Its **own** windows are protected with a direct call. To protect **another** app's windows, a small helper library (`hmw_payload.dll`) is loaded into that app so the flag can be set from inside it — because Windows only lets a window's own process set this flag.
- Automatic rules use lightweight **polling** instead of WMI, so **no administrator rights are needed**.

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
