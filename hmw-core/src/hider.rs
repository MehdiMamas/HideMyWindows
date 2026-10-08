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
