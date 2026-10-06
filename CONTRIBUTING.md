# Contributing to HideMyWindows

Thanks for your interest! HideMyWindows 2.0 was rebuilt and improved by
[Mehdi](https://github.com/mehdimamas) from the original app, and contributions
are very welcome.

## Getting started

1. Read [BUILDING.md](BUILDING.md) to get a local build running.
2. Skim [ARCHITECTURE.md](ARCHITECTURE.md) to see where things live.

## Ways to help

- 🐛 **Bug reports** — open an issue with your Windows version, architecture, the
  app you were trying to hide, and what happened.
- 🌍 **Translations** — add a `src/lib/locales/<code>.json` (copy `en.json`) and
  register it in `src/lib/i18n.js`.
- ✨ **Features / fixes** — open an issue first for larger changes so we can
  agree on the approach.

## Guidelines

- Keep the **Win32 logic in `hmw-core`** and keep it small and well-typed; the
  Tauri layer should stay thin.
- Match the existing code style; run `cargo fmt` and `cargo clippy` for Rust.
- The UI stays dependency-light — prefer a small Svelte component over a new
  library.
- Don't break the config format; add fields with `#[serde(default)]`.

## Commit / PR

- One logical change per PR, with a clear description.
- Make sure `cargo build` (on Windows) and `npm run build` succeed.
- By contributing, you agree your work is licensed under the project's
  MIT + Commons Clause license.
