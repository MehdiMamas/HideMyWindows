import { invoke } from "@tauri-apps/api/core";

export const getConfig = () => invoke("get_config");
export const getConfigDir = () => invoke("get_config_dir");
export const appVersion = () => invoke("app_version");
export const captureHidden = () => invoke("capture_hidden");
export const saveConfig = (config) => invoke("save_config", { config });
export const resetSettings = () => invoke("reset_settings");
export const listProcesses = () => invoke("list_processes");
export const listWindows = () => invoke("list_windows");

export const hideProcess = (pid, action) => invoke("hide_process", { pid, action });
export const hideWindow = (hwnd, action) => invoke("hide_window", { hwnd, action });
export const quickLaunch = (path, args) =>
  invoke("quick_launch", { path, arguments: args });
