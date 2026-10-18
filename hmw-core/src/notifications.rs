//! Exclude Windows toast popups from screen capture.
//!
//! Toasts belong to `ShellExperienceHost.exe`, not to the app that sent them.
//! The payload export filters by window class and localized title, so Start,
//! Search, and Quick Settings in that process are left alone.
//!
//! That process is an AppContainer. It can load a DLL from its own package
//! folder, and it cannot load one from a normal user path. The payload is
//! copied into that folder before injection.

use std::path::PathBuf;

use crate::inject::call_export;
use crate::process::list_processes;
use crate::{Error, Result};

/// Shell process that owns toast popups on Windows 10 and Windows 11.
const TOAST_HOSTS: &[&str] = &["ShellExperienceHost.exe"];

/// Inbox package whose TempState the shell host is allowed to read.
const SHELL_PACKAGE: &str = "Microsoft.Windows.ShellExperienceHost_cw5n1h2txyewy";

/// Copy the payload where ShellExperienceHost can `LoadLibrary` it.
/// Falls back to the original path when that package folder is absent.
fn staged_payload(payload_path: &str) -> Result<String> {
    let Ok(local) = std::env::var("LOCALAPPDATA") else {
        return Ok(payload_path.to_string());
    };
    let dest = PathBuf::from(local)
        .join("Packages")
        .join(SHELL_PACKAGE)
        .join("TempState")
        .join("hmw_payload.dll");
    if dest.parent().is_none_or(|dir| !dir.is_dir()) {
        return Ok(payload_path.to_string());
    }
    // Steady state: the staged copy is current, so leave the file alone.
    if let (Ok(src), Ok(dst)) = (std::fs::metadata(payload_path), std::fs::metadata(&dest)) {
        if src.len() == dst.len() && src.modified().ok() == dst.modified().ok() {
            return Ok(dest.to_string_lossy().into_owned());
        }
    }
    match std::fs::copy(payload_path, &dest) {
        Ok(_) => Ok(dest.to_string_lossy().into_owned()),
        Err(_) if dest.is_file() => Ok(dest.to_string_lossy().into_owned()),
        Err(e) => Err(Error(format!(
            "Could not stage the notification helper: {e}"
        ))),
    }
}

/// Apply or clear capture exclusion for notification toasts.
///
/// Returns human-readable errors. An empty list means every host was updated.
/// When turning the feature off and the host is not running, there is nothing
/// left to restore.
pub fn apply_notification_toasts(hidden: bool, payload_path: &str) -> Vec<String> {
    let export = if hidden {
        "HmwHideToasts"
    } else {
        "HmwUnhideToasts"
    };
    let payload_path = match staged_payload(payload_path) {
        Ok(path) => path,
        Err(e) => return vec![format!("Notifications: {e}")],
    };
    let processes = match list_processes() {
        Ok(processes) => processes,
        Err(e) => return vec![format!("Notifications: {e}")],
    };
    let pids: Vec<u32> = processes
        .into_iter()
        .filter(|process| {
            TOAST_HOSTS
                .iter()
                .any(|name| process.name.eq_ignore_ascii_case(name))
        })
        .map(|process| process.pid)
        .collect();
    if pids.is_empty() {
        if hidden {
            return vec!["Notifications: ShellExperienceHost is not running".into()];
        }
        return Vec::new();
    }

    let mut errors = Vec::new();
    for pid in pids {
        // The payload keeps toast exclusion on by itself. Re-send "hide" only
        // when the host is new (pid or start time) or on the health interval.
        let started = crate::process::process_started_at(pid).unwrap_or(0);
        if hidden && !toast_send_due(pid, started) {
            continue;
        }
        if let Err(e) = call_export(pid, &payload_path, export, 0) {
            toast_sent().remove(&pid);
            errors.push(format!("Notifications: {e}"));
        }
    }
    if !hidden {
        toast_sent().clear();
    }
    errors
}

type ToastSent = std::collections::HashMap<u32, (u64, std::time::Instant)>;

fn toast_sent() -> std::sync::MutexGuard<'static, ToastSent> {
    static SENT: std::sync::OnceLock<std::sync::Mutex<ToastSent>> = std::sync::OnceLock::new();
    SENT.get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

/// True, and recorded, when `pid` has not been sent "hide" for this process
/// instance within `PROTECTION_RECHECK`.
fn toast_send_due(pid: u32, started: u64) -> bool {
    let now = std::time::Instant::now();
    let mut sent = toast_sent();
    match sent.get(&pid) {
        Some(&(s, at)) if s == started && now.duration_since(at) < crate::hider::PROTECTION_RECHECK => {
            false
        }
        _ => {
            sent.insert(pid, (started, now));
            true
        }
    }
}
