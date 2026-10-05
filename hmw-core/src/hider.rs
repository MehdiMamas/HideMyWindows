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
    let hwnd = if needs_window(action) {
        windows_for_pid(pid)?
            .first()
            .map(|w| w.hwnd)
            .ok_or_else(|| Error("The process has no visible window".into()))?
    } else {
        0
    };
    call_export(pid, payload_path, export_for(action), hwnd)
}

/// Apply an action targeting a specific window handle.
pub fn apply_to_window(hwnd: isize, action: HideAction, payload_path: &str) -> Result<()> {
    let pid = window_pid(hwnd);
    if pid == 0 {
        return Err(Error("Could not resolve the window's process".into()));
    }
    let target = if needs_window(action) { hwnd } else { 0 };
    call_export(pid, payload_path, export_for(action), target)
}
