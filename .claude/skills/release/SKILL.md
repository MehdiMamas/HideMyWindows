---
name: release
description: Prepare and publish a HideMyWindows release (version bump in every file release.yml checks, CHANGELOG section, local checks, tag). Use when asked to release, ship or "prepare X.Y.Z".
disable-model-invocation: true
---

# Release HideMyWindows

## Rules
- `release.yml` refuses to publish unless these all say the same `X.Y.Z`: `package.json`, `src-tauri/tauri.conf.json`, `[workspace.package]` in `Cargo.toml`, and the tag `vX.Y.Z`. `CHANGELOG.md` needs a non-empty `## [X.Y.Z]` section.
- The CHANGELOG section (bold removed) is the update prompt installed copies show. Lead with what the user must know or do; say plainly if updating affects other apps.
- Lock files change only through their tools (the project hook blocks hand edits).
- Never move or reuse a published tag. A failed release gets a new patch version.

## Steps
1. Pick the version (SemVer) from the changes since the last tag: `git describe --tags --abbrev=0`, `git log <tag>..HEAD --oneline`.
2. Bump:
   - `npm version X.Y.Z --no-git-tag-version` (package.json + package-lock.json)
   - `"version"` in `src-tauri/tauri.conf.json`
   - `version` under `[workspace.package]` in `Cargo.toml`, then `cargo update --workspace` (Cargo.lock)
3. Add `## [X.Y.Z] — YYYY-MM-DD` at the top of `CHANGELOG.md` (Added / Changed / Fixed, user-facing words).
4. Check locally (Windows): `npm ci`, `npm run build`, `npm test`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --locked -p hmw-core -- --test-threads=1`. Report each command with its exit code.
5. Commit `Prepare X.Y.Z release`, merge to `main` through a PR, then push the tag: `git tag vX.Y.Z <merge commit>` and `git push origin vX.Y.Z` (or push a `release/vX.Y.Z` branch when tags can't be pushed).
6. Watch it: `gh run watch $(gh run list -w release.yml -L 1 --json databaseId -q '.[0].databaseId')`. All three installers (x64, x86, ARM64) must pass before the release is published. Give the run URL as proof.
