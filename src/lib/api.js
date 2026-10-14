import { invoke } from "@tauri-apps/api/core";

export const getConfig = () => invoke("get_config");
export const getRuleStatus = () => invoke("get_rule_status");
export const getLaunchProtectionStatus = () => invoke("get_launch_protection_status");
export const getConfigDir = () => invoke("get_config_dir");
export const appVersion = () => invoke("app_version");
export const captureStatuses = () => invoke("capture_statuses");
export const saveConfig = (config) => invoke("save_config", { config });
export const resetSettings = () => invoke("reset_settings");
export const listProcesses = () => invoke("list_processes");
export const listWindows = () => invoke("list_windows");

export const hideProcess = (pid, action) => invoke("hide_process", { pid, action });
export const hideWindow = (hwnd, action) => invoke("hide_window", { hwnd, action });
export const quickLaunch = (path, args, appId = "") =>
  invoke("quick_launch", { path, arguments: args, appId });

// Apps from Start's "All apps" list; icons make it a few seconds slower.
export const listInstalledApps = (withIcons) =>
  invoke("list_installed_apps", { withIcons });

// Returns the path of the new "<name> HideMyWindows" Start menu shortcut.
export const createLaunchShortcut = (name, path, args, appId = "") =>
  invoke("create_launch_shortcut", { name, path, arguments: args, appId });

export const removeLaunchShortcut = (shortcut) =>
  invoke("remove_launch_shortcut", { shortcut });
