import { listen } from "@tauri-apps/api/event";
import { config, notify } from "./stores.js";

let revision = 0;
export const trayConfigRevision = () => revision;

export async function connectTrayEvents() {
  let receivedConfig = false;
  const stopConfig = await listen("config-changed", ({ payload }) => {
    receivedConfig = true;
    revision += 1;
    // Tray actions change only protection. Preserve in-progress edits to
    // unrelated settings or rules in the open window.
    config.update((current) => current ? {
      ...current,
      normalLaunchProtection: payload.normalLaunchProtection,
      normalLaunchPauseUntilMs: payload.normalLaunchPauseUntilMs,
    } : payload);
  });
  let stopError;
  try {
    stopError = await listen("tray-action-error", ({ payload }) => notify(String(payload), "error", 0));
  } catch (error) {
    stopConfig();
    throw error;
  }
  return {
    receivedConfig: () => receivedConfig,
    stop: () => { stopConfig(); stopError(); },
  };
}
