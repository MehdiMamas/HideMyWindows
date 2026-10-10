import { writable } from "svelte/store";

/** The live config object (mirrors hmw_core::config::Config). */
export const config = writable(null);
export const launchProtectionStatus = writable(null);

/** Transient toast notifications shown bottom-right. */
export const toasts = writable([]);

let nextId = 1;

export function notify(message, severity = "info", timeout = 4000) {
  const id = nextId++;
  toasts.update((list) => [...list, { id, message, severity }]);
  if (timeout > 0) {
    setTimeout(() => dismiss(id), timeout);
  }
  return id;
}

export function dismiss(id) {
  toasts.update((list) => list.filter((t) => t.id !== id));
}

/** Apply a theme ("system" | "light" | "dark") to the document. */
export function applyTheme(theme) {
  const root = document.documentElement;
  if (theme === "system") {
    const dark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    root.setAttribute("data-theme", dark ? "dark" : "light");
  } else {
    root.setAttribute("data-theme", theme);
  }
}
