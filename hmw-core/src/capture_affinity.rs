//! Keep capture-excluded windows out of DWM's minimize/restore animations.
//!
//! An inactive excluded window can leave a black local surface behind during
//! an animated minimize. Disable transitions on that HWND before exclusion;
//! never drop display affinity to work around a local rendering artifact.
//! Shared by the controller and injected payload, including pre-show hooks.

use std::sync::Mutex;
use windows::core::w;
use windows::Win32::Foundation::{BOOL, HANDLE, HWND};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    GetPropW, GetWindowDisplayAffinity, GetWindowThreadProcessId, IsWindowVisible, RemovePropW,
    SetPropW, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WINDOW_DISPLAY_AFFINITY,
};

const TRANSITIONS: windows::core::PCWSTR = w!("HideMyWindows.CaptureTransitions");
static LOCK: Mutex<()> = Mutex::new(());

fn set_transitions_disabled(hwnd: HWND, disabled: bool) -> windows::core::Result<()> {
    let value = BOOL::from(disabled);
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            &value as *const _ as _,
            std::mem::size_of::<BOOL>() as u32,
        )
    }
}

/// Best effort: capture protection must still work if DWM rejects the hint.
/// Returns true only when this call newly applied the transition override.
pub fn suppress_transitions(hwnd: HWND) -> bool {
    let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    unsafe {
        if !GetPropW(hwnd, TRANSITIONS).is_invalid() {
            return false;
        }
        // Reserve the marker first so a failed property write cannot leave an
        // override which unhide has no way to find. HWND properties disappear
        // with their window, so recycled handles cannot inherit this state.
        if SetPropW(
            hwnd,
            TRANSITIONS,
            HANDLE(std::ptr::dangling_mut::<u8>().cast()),
        )
        .is_err()
        {
            return false;
        }
        if set_transitions_disabled(hwnd, true).is_err() {
            let _ = RemovePropW(hwnd, TRANSITIONS);
            return false;
        }
    }
    true
}

/// Reset only windows on which we applied an override. This DWM attribute is
/// setter-only; FALSE restores normal transitions (and respects the system's
/// animation preference), rather than changing a global Windows setting.
pub fn restore_transitions(hwnd: HWND) {
    let _guard = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    unsafe {
        if !GetPropW(hwnd, TRANSITIONS).is_invalid()
            && set_transitions_disabled(hwnd, false).is_ok()
        {
            let _ = RemovePropW(hwnd, TRANSITIONS);
        }
    }
}

/// Idempotent affinity updates for single-window hides, toasts and our UI.
pub fn set_affinity(hwnd: HWND, affinity: u32) -> windows::core::Result<()> {
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid != GetCurrentProcessId() {
            // Retain the API's ownership/error behavior for foreign handles,
            // including no-op requests. Never cloak another process from here.
            return SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(affinity));
        }
    }
    let added = affinity == WDA_EXCLUDEFROMCAPTURE.0 && suppress_transitions(hwnd);
    let mut current = 0;
    unsafe {
        if !IsWindowVisible(hwnd).as_bool()
            || GetWindowDisplayAffinity(hwnd, &mut current).is_err()
            || current != affinity
        {
            if let Err(error) = SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(affinity)) {
                if added {
                    restore_transitions(hwnd);
                }
                return Err(error);
            }
        }
        // A process-hide hook can override an unhide request. Keep transitions
        // suppressed until exclusion actually ends, including no-op updates.
        if affinity != WDA_EXCLUDEFROMCAPTURE.0
            && ((!IsWindowVisible(hwnd).as_bool())
                || (GetWindowDisplayAffinity(hwnd, &mut current).is_ok()
                    && current != WDA_EXCLUDEFROMCAPTURE.0))
        {
            restore_transitions(hwnd);
        }
    }
    crate::capture_presentation::protection_updated(hwnd);
    Ok(())
}
