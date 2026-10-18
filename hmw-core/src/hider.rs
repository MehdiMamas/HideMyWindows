//! High-level hide/unhide operations that map [`HideAction`] onto the payload.

use crate::inject::call_export;
use crate::model::HideAction;
use crate::window::{window_pid, windows_for_pid};
use crate::{Error, Result};

fn export_for(action: HideAction) -> &'static str {
    match action {
        HideAction::HideProcessWindows => "HmwHideAll",
        HideAction::UnhideProcessWindows => "HmwUnhideAll",
        HideAction::HideWindow => "HmwHideWindow",
        HideAction::UnhideWindow => "HmwUnhideWindow",
        HideAction::HideTrayIcon => "HmwHideTray",
        HideAction::UnhideTrayIcon => "HmwUnhideTray",
    }
}

fn needs_window(action: HideAction) -> bool {
    !matches!(
        action,
        HideAction::HideProcessWindows | HideAction::UnhideProcessWindows
    )
}

/// Apply an action to every window of a process (or its main window, for
/// single-window/tray actions).
pub fn apply_to_process(pid: u32, action: HideAction, payload_path: &str) -> Result<()> {
    // Controller-owned title-bar dots must remain capture-excluded when the
    // user unhides HideMyWindows itself. The filtered list omits these HWNDs.
    if pid == unsafe { windows::Win32::System::Threading::GetCurrentProcessId() }
        && matches!(
            action,
            HideAction::HideProcessWindows | HideAction::UnhideProcessWindows
        )
    {
        for window in windows_for_pid(pid)? {
            crate::window::set_capture_hidden(
                window.hwnd,
                action == HideAction::HideProcessWindows,
            )?;
        }
        return Ok(());
    }
    let hwnd = if needs_window(action) {
        windows_for_pid(pid)?
            .first()
            .map(|w| w.hwnd)
            .ok_or_else(|| Error("The process has no visible window".into()))?
    } else {
        0
    };
    if action == HideAction::HideProcessWindows {
        prepare_protection(pid, payload_path)?;
    }
    call_export(pid, payload_path, export_for(action), hwnd)
}

/// Apply an action targeting a specific window handle.
pub fn apply_to_window(hwnd: isize, action: HideAction, payload_path: &str) -> Result<()> {
    let pid = window_pid(hwnd);
    if pid == 0 {
        return Err(Error("Could not resolve the window's process".into()));
    }
    let target = if needs_window(action) { hwnd } else { 0 };
    if action == HideAction::HideProcessWindows {
        prepare_protection(pid, payload_path)?;
    }
    call_export(pid, payload_path, export_for(action), target)
}

fn prepare_protection(pid: u32, payload_path: &str) -> Result<()> {
    call_export(pid, payload_path, "HmwPrepareProtection", 0).map_err(|error| {
        Error(format!("Pre-show protection could not be installed. Restart the target app after updating HideMyWindows. {}", error.0))
    })
}

pub fn check_protection(pid: u32, payload_path: &str) -> Result<()> {
    if pid == unsafe { windows::Win32::System::Threading::GetCurrentProcessId() } {
        return Ok(());
    }
    call_export(pid, payload_path, "HmwCheckProtection", 0).map_err(|error| {
        Error(format!(
            "A window was kept invisible because Windows could not confirm capture protection. {}",
            error.0
        ))
    })
}

/// How long a steady target goes without a protection health check.
pub const PROTECTION_RECHECK: std::time::Duration = std::time::Duration::from_secs(15);

/// Decides when `check_protection` is worth a remote thread. A target is
/// checked when its visible window set changed since the last check, or when
/// the last check is older than `PROTECTION_RECHECK`.
#[derive(Debug, Default)]
pub struct ProtectionThrottle {
    seen: std::collections::HashMap<u32, (u64, std::time::Instant)>,
}

impl ProtectionThrottle {
    /// True when `pid` should be checked now; records the check when it is.
    pub fn due(&mut self, pid: u32, signature: u64, now: std::time::Instant) -> bool {
        match self.seen.get(&pid) {
            Some(&(last, at)) if last == signature && now.duration_since(at) < PROTECTION_RECHECK => {
                false
            }
            _ => {
                self.seen.insert(pid, (signature, now));
                true
            }
        }
    }

    /// Forget a pid, so its next check is immediate.
    pub fn forget(&mut self, pid: u32) {
        self.seen.remove(&pid);
    }

    /// Drop entries for pids no longer being tracked.
    pub fn retain(&mut self, keep: impl Fn(u32) -> bool) {
        self.seen.retain(|&pid, _| keep(pid));
    }
}

/// Order-independent fingerprint of a set of window handles.
pub fn window_signature(hwnds: impl IntoIterator<Item = isize>) -> u64 {
    hwnds.into_iter().fold(0u64, |acc, h| {
        acc.wrapping_add((h as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(17))
    })
}
