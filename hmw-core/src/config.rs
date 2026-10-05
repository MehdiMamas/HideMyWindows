//! Application configuration: load/save a JSON file under %APPDATA%\HideMyWindows.

use crate::model::{QuickLaunchEntry, WindowRule};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

fn default_true() -> bool {
    true
}
fn default_interval() -> u64 {
    1000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default)]
    pub theme: Theme,

    /// Hide HideMyWindows' own window from screen capture.
    /// Off by default so the app stays visible to remote-desktop/capture tools
    /// (e.g. AnyDesk); it hides other apps, not itself, unless turned on.
    #[serde(default)]
    pub hide_self: bool,

    /// Exclude every app's notification toasts from screen capture.
    /// The toasts stay visible on this PC. Off by default so existing configs
    /// keep showing notifications in screenshots and recordings.
    #[serde(default)]
    pub hide_notification_toasts: bool,

    /// Minimize to the system tray instead of the taskbar.
    #[serde(default)]
    pub minimize_to_tray: bool,

    /// Keep running in the tray when the window is closed.
    #[serde(default = "default_true")]
    pub close_to_tray: bool,

    /// Start automatically with Windows (managed via the registry Run key).
    #[serde(default)]
    pub start_with_windows: bool,

    /// How often persistent rules are re-applied, in milliseconds.
    #[serde(default = "default_interval")]
    pub rule_reapply_interval_ms: u64,

    /// How often the process list / new-process rules are polled, in ms.
    #[serde(default = "default_interval")]
    pub process_poll_interval_ms: u64,

    /// UI language (BCP-47 / two-letter). `None` means follow the system.
    #[serde(default)]
    pub language: Option<String>,

    /// The guided tour has been completed/dismissed.
    #[serde(default)]
    pub tour_completed: bool,

    #[serde(default)]
    pub window_rules: Vec<WindowRule>,

    #[serde(default)]
    pub quick_launch: Vec<QuickLaunchEntry>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: Theme::default(),
            hide_self: false,
            hide_notification_toasts: false,
            minimize_to_tray: false,
            close_to_tray: true,
            start_with_windows: false,
            rule_reapply_interval_ms: 1000,
            process_poll_interval_ms: 1000,
            language: None,
            tour_completed: false,
            window_rules: Vec::new(),
            quick_launch: Vec::new(),
        }
    }
}

impl Config {
    /// Default on-disk location: `%APPDATA%\HideMyWindows\config.json`
    /// (falls back to the executable directory if APPDATA is unavailable).
    pub fn default_path() -> PathBuf {
        if let Ok(appdata) = std::env::var("APPDATA") {
            PathBuf::from(appdata)
                .join("HideMyWindows")
                .join("config.json")
        } else {
            PathBuf::from("config.json")
        }
    }

    /// Load from `path`, returning defaults if the file is missing or invalid.
    pub fn load(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Config::default(),
        }
    }

    /// Persist to `path`, creating the parent directory if needed.
    pub fn save(&self, path: &std::path::Path) -> crate::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| crate::Error(e.to_string()))?;
        std::fs::write(path, text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn old_config_leaves_notification_hiding_off() {
        let cfg: Config = serde_json::from_str(r#"{"hideSelf":true}"#).unwrap();
        assert!(cfg.hide_self);
        assert!(!cfg.hide_notification_toasts);
    }
}
