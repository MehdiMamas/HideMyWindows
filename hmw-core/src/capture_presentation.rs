//! Remove a minimized protected HWND's local DWM presentation, not its affinity.
//! Capture-excluded GPU-backed windows can retain a black compositor surface
//! even when transition animations are disabled. A temporary application cloak
//! makes the minimized window locally invisible; restore/unhide releases it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use windows::core::w;
use windows::Win32::Foundation::{BOOL, HANDLE, HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DwmSetWindowAttribute, DWMWA_CLOAK, DWMWA_CLOAKED, DWM_CLOAKED_APP,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::*;

const PROTECTED: windows::core::PCWSTR = w!("HideMyWindows.CaptureTransitions");
const CLOAK: windows::core::PCWSTR = w!("HideMyWindows.MinimizedCaptureCloak");
const SEEN_VISIBLE: windows::core::PCWSTR = w!("HideMyWindows.CaptureWasVisible");
static STARTED: AtomicBool = AtomicBool::new(false);

fn cloak(hwnd: HWND, enabled: bool) -> windows::core::Result<()> {
    let value = BOOL::from(enabled);
    unsafe {
        let result = DwmSetWindowAttribute(hwnd, DWMWA_CLOAK, &value as *const _ as _, 4);
        // Zero means no attempt; one represents S_OK. Preserve HRESULT bits
        // on both architectures for read-only troubleshooting.
        let code = result
            .as_ref()
            .err()
            .map_or(0, |error| error.code().0 as u32);
        let _ = SetPropW(
            hwnd,
            w!("HideMyWindows.CaptureCloakResult"),
            HANDLE((code as usize).wrapping_add(1) as *mut _),
        );
        result
    }
}

/// Only modify our own protected windows and only release cloaks we applied.
/// HWND properties die with the window, avoiding stale/recycled-handle state.
pub fn synchronize(hwnd: HWND) {
    unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid != GetCurrentProcessId() || GetAncestor(hwnd, GA_ROOT) != hwnd {
            return;
        }
        let owned = !GetPropW(hwnd, CLOAK).is_invalid();
        let tracked = !GetPropW(hwnd, PROTECTED).is_invalid();
        if !owned && !tracked {
            if !GetPropW(hwnd, SEEN_VISIBLE).is_invalid() {
                let _ = RemovePropW(hwnd, SEEN_VISIBLE);
            }
            return;
        }
        let _ = SetPropW(
            hwnd,
            w!("HideMyWindows.CapturePresentationObserved"),
            HANDLE(std::ptr::dangling_mut::<u8>().cast()),
        );
        let visible = IsWindowVisible(hwnd).as_bool();
        if tracked && visible && GetPropW(hwnd, SEEN_VISIBLE).is_invalid() {
            let _ = SetPropW(
                hwnd,
                SEEN_VISIBLE,
                HANDLE(std::ptr::dangling_mut::<u8>().cast()),
            );
        }
        let mut affinity = 0;
        // Some apps hide their window instead of using the native iconic
        // state. Do not cloak an initially invisible pre-show window: wait
        // until it has actually been visible at least once.
        let affinity_result = GetWindowDisplayAffinity(hwnd, &mut affinity);
        let _ = SetPropW(
            hwnd,
            w!("HideMyWindows.CapturePresentationAffinity"),
            HANDLE((affinity as usize + 1) as *mut _),
        );
        let _ = SetPropW(
            hwnd,
            w!("HideMyWindows.CapturePresentationAffinityResult"),
            HANDLE(
                (affinity_result
                    .as_ref()
                    .err()
                    .map_or(0, |e| e.code().0 as u32) as usize)
                    .wrapping_add(1) as *mut _,
            ),
        );
        let locally_hidden_protected = tracked
            && (IsIconic(hwnd).as_bool()
                || (!visible && !GetPropW(hwnd, SEEN_VISIBLE).is_invalid()))
            && affinity_result.is_ok()
            && affinity == WDA_EXCLUDEFROMCAPTURE.0;
        if !locally_hidden_protected {
            if owned && cloak(hwnd, false).is_ok() {
                let _ = RemovePropW(hwnd, CLOAK);
            }
            if !tracked {
                let _ = RemovePropW(hwnd, SEEN_VISIBLE);
            }
            return;
        }
        // Do not take ownership of another feature's existing application
        // cloak (or a shell cloak for a different virtual desktop).
        let mut flags = 0u32;
        if DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut flags as *mut _ as _, 4).is_err() {
            return;
        }
        if owned {
            if flags & DWM_CLOAKED_APP == 0 {
                let _ = cloak(hwnd, true);
            }
        } else if flags == 0
            && SetPropW(hwnd, CLOAK, HANDLE(std::ptr::dangling_mut::<u8>().cast())).is_ok()
            && cloak(hwnd, true).is_err()
        {
            let _ = RemovePropW(hwnd, CLOAK);
        }
    }
}

unsafe extern "system" fn event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    if object == OBJID_WINDOW.0 && child == 0 && !hwnd.is_invalid() {
        synchronize(hwnd);
    }
}

unsafe extern "system" fn scan(hwnd: HWND, _context: LPARAM) -> BOOL {
    synchronize(hwnd);
    BOOL(1)
}

/// Event callbacks run on a dedicated message-pumping thread. A bounded scan
/// recovers pre-state-change events, missed renderer events and failed DWM calls.
/// Never hold a Rust lock while asking DWM to change a window's presentation:
/// the window owner may be processing an affinity/restore request concurrently.
pub fn start() {
    if STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    if std::thread::Builder::new()
        .name("hmw-minimized-presentation".into())
        .spawn(|| unsafe {
            let pid = GetCurrentProcessId();
            let hooks = [
                SetWinEventHook(
                    EVENT_SYSTEM_MINIMIZESTART,
                    EVENT_SYSTEM_MINIMIZEEND,
                    None,
                    Some(event),
                    pid,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                ),
                SetWinEventHook(
                    EVENT_OBJECT_SHOW,
                    EVENT_OBJECT_HIDE,
                    None,
                    Some(event),
                    pid,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                ),
                SetWinEventHook(
                    EVENT_OBJECT_LOCATIONCHANGE,
                    EVENT_OBJECT_LOCATIONCHANGE,
                    None,
                    Some(event),
                    pid,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                ),
            ];
            let mut next_scan = Instant::now();
            loop {
                let mut message = MSG::default();
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    if message.message == WM_QUIT {
                        for hook in hooks {
                            if !hook.is_invalid() {
                                let _ = UnhookWinEvent(hook);
                            }
                        }
                        STARTED.store(false, Ordering::Release);
                        return;
                    }
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                if Instant::now() >= next_scan {
                    let _ = EnumWindows(Some(scan), LPARAM(0));
                    next_scan = Instant::now() + Duration::from_millis(200);
                }
                // Sleep until Windows queues an event, or the recovery scan
                // is due. Idle protected apps do not need a busy polling loop.
                MsgWaitForMultipleObjectsEx(None, 200, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            }
        })
        .is_err()
    {
        STARTED.store(false, Ordering::Release);
    }
}

pub fn protection_updated(hwnd: HWND) {
    synchronize(hwnd);
    unsafe {
        let mut affinity = 0;
        if !GetPropW(hwnd, PROTECTED).is_invalid()
            && affinity_result.is_ok()
            && affinity == WDA_EXCLUDEFROMCAPTURE.0
        {
            start();
        }
    }
}
