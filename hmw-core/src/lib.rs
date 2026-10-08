//! Core logic for HideMyWindows: hiding windows from screen capture,
//! enumerating processes, injecting the helper payload into a target process,
//! matching window rules, and launching quick-launch entries.
//!
//! All Windows-specific code is gated behind `cfg(windows)` so the crate still
//! type-checks and documents on other platforms.

pub mod config;
pub mod model;
pub mod rule_status;

#[cfg(windows)]
pub mod indicator;
#[cfg(any(windows, test))]
mod indicator_position;

#[cfg(windows)]
pub mod cleanup;
#[cfg(windows)]
pub mod hider;
#[cfg(windows)]
pub mod inject;
#[cfg(windows)]
pub mod launch;
#[cfg(windows)]
pub mod notifications;
#[cfg(windows)]
pub mod process;
#[cfg(windows)]
pub mod rule_events;
#[cfg(windows)]
pub mod watcher;
#[cfg(windows)]
pub mod window;
#[cfg(windows)]
pub mod wow64;

pub use config::Config;
pub use model::{
    HideAction, ProcessInfo, QuickLaunchEntry, RuleComparator, RuleTarget, WindowRule,
};

/// A simple result type used across the crate.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error(s)
    }
}
impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error(s.to_string())
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error(e.to_string())
    }
}

#[cfg(windows)]
impl From<windows::core::Error> for Error {
    fn from(e: windows::core::Error) -> Self {
        Error(e.message())
    }
}
