import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { writable } from "svelte/store";
import { tr } from "./i18n.js";
import { dismiss, notify } from "./stores.js";

/** Download progress for the About page. `phase` is idle, checking, or downloading. */
export const updateStatus = writable({
  phase: "idle",
  version: "",
  downloaded: 0,
  total: 0,
});

let busy = false;

function setStatus(patch) {
  updateStatus.update((current) => ({ ...current, ...patch }));
}

async function revealWindow() {
  try {
    const window = getCurrentWindow();
    await window.show();
    await window.unminimize();
    await window.setFocus();
  } catch {
    // The prompt is a native dialog, so a hidden window should not block it.
  }
}

/**
 * Check GitHub releases for a newer build.
 * Startup calls this quietly. The About button passes `interactive` so a
 * current version or a failed check is reported.
 */
export async function checkForUpdates({ interactive = false } = {}) {
  if (import.meta.env.DEV) {
    if (interactive) notify(tr("about.updateDev"), "info");
    return;
  }
  if (busy) return;
  busy = true;
  setStatus({ phase: "checking", downloaded: 0, total: 0, version: "" });
  let accepted = false;
  let downloadToast = 0;
  try {
    const update = await check();
    if (!update) {
      if (interactive) notify(tr("about.upToDate"), "success");
      return;
    }

    await revealWindow();
    const notes = (update.body || "").trim();
    const body = notes
      ? tr("about.updatePrompt", { version: update.version, notes })
      : tr("about.updatePromptNoNotes", { version: update.version });
    const yes = await ask(body, {
      title: tr("about.updateTitle"),
      kind: "info",
    });
    if (!yes) return;

    accepted = true;
    downloadToast = notify(tr("about.downloading", { version: update.version }), "info", 0);
    let downloaded = 0;
    let total = 0;
    setStatus({ phase: "downloading", version: update.version, downloaded: 0, total: 0 });
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") {
        total = event.data.contentLength ?? 0;
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
      }
      setStatus({ phase: "downloading", version: update.version, downloaded, total });
    });
    try {
      await relaunch();
    } catch {
      // The NSIS installer restarts the app after it replaces the files.
    }
  } catch (e) {
    if (interactive || accepted) {
      notify(tr("about.updateFailed", { error: String(e) }), "error", 8000);
    }
  } finally {
    if (downloadToast) dismiss(downloadToast);
    busy = false;
    setStatus({ phase: "idle", downloaded: 0, total: 0 });
  }
}
