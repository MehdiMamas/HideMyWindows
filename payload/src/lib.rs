//! HideMyWindows injected payload.
//!
//! This DLL is loaded into a target process so its windows can be excluded from
//! screen capture (`SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` must be
//! called from a thread inside the window's own process).
//!
//! The host (HideMyWindows.exe) injects this DLL and then starts remote threads
//! at the exported functions below; the HWND an action needs is passed directly
//! as the thread parameter. A lightweight background worker re-applies the
//! "hide all" state so windows created later are hidden too — replacing the old
//! API-hooking approach with something far simpler and more robust.
#![cfg(windows)]

use core::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use windows::Win32::Foundation::{BOOL, HMODULE, HWND, LPARAM, TRUE};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
use windows::Win32::UI::Shell::{ITaskbarList, TaskbarList};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowDisplayAffinity, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
    WINDOW_DISPLAY_AFFINITY,
};

mod toasts;

/// Whether "hide all windows of this process" is currently active.
static HIDING: AtomicBool = AtomicBool::new(false);
/// Whether toast popups in this process are excluded from capture.
static HIDE_TOASTS: AtomicBool = AtomicBool::new(false);
/// Whether the background re-apply worker has been started.
static WORKER_STARTED: AtomicBool = AtomicBool::new(false);
/// Serializes affinity updates so a worker pass cannot re-hide windows after
/// `HmwUnhideAll` has already cleared the flag.
static APPLY_LOCK: Mutex<()> = Mutex::new(());

#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn DllMain(hinst: HMODULE, reason: u32, _reserved: *mut c_void) -> BOOL {
    // DLL_PROCESS_ATTACH == 1. We avoid doing real work here (loader lock);
    // the worker thread is started lazily from an exported function instead.
    if reason == 1 {
        unsafe {
            let _ = DisableThreadLibraryCalls(hinst);
        }
    }
    TRUE
}

struct EnumCtx {
    pid: u32,
    affinity: u32,
}

unsafe extern "system" fn apply_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &*(lparam.0 as *const EnumCtx);
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == ctx.pid && IsWindowVisible(hwnd).as_bool() {
        set_affinity(hwnd, ctx.affinity);
    }
    TRUE
}

/// Write display affinity only when it is not already the requested value.
///
/// `SetWindowDisplayAffinity` notifies the desktop compositor. Calling it again
/// with the same flag makes capture clients rebuild the redacted frame, which
/// shows up as GPU time on the app that is streaming. A read of the current
/// affinity does not.
fn set_affinity(hwnd: HWND, affinity: u32) {
    unsafe {
        let mut current = 0u32;
        if GetWindowDisplayAffinity(hwnd, &mut current).is_ok() && current == affinity {
            return;
        }
        let _ = SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(affinity));
    }
}

fn set_all_windows(hidden: bool) {
    let ctx = EnumCtx {
        pid: current_pid(),
        affinity: if hidden {
            WDA_EXCLUDEFROMCAPTURE.0
        } else {
            WDA_NONE.0
        },
    };
    unsafe {
        let _ = EnumWindows(Some(apply_cb), LPARAM(&ctx as *const EnumCtx as isize));
    }
}

fn current_pid() -> u32 {
    unsafe { windows::Win32::System::Threading::GetCurrentProcessId() }
}

fn wide_prefix(buf: &[u16], len: i32) -> String {
    let n = len.max(0) as usize;
    String::from_utf16_lossy(&buf[..n.min(buf.len())])
}

fn is_toast_hwnd(hwnd: HWND) -> bool {
    unsafe {
        let mut class_buf = [0u16; 64];
        let class_len = GetClassNameW(hwnd, &mut class_buf);
        let mut title_buf = [0u16; 256];
        let title_len = GetWindowTextW(hwnd, &mut title_buf);
        toasts::is_toast_window(
            &wide_prefix(&class_buf, class_len),
            &wide_prefix(&title_buf, title_len),
        )
    }
}

unsafe extern "system" fn toast_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &*(lparam.0 as *const EnumCtx);
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == ctx.pid && IsWindowVisible(hwnd).as_bool() && is_toast_hwnd(hwnd) {
        set_affinity(hwnd, ctx.affinity);
    }
    TRUE
}

/// Exclude or restore only toast popups. Other windows in this process stay as they are.
fn set_toast_windows(hidden: bool) {
    let ctx = EnumCtx {
        pid: current_pid(),
        affinity: if hidden {
            WDA_EXCLUDEFROMCAPTURE.0
        } else {
            WDA_NONE.0
        },
    };
    unsafe {
        let _ = EnumWindows(Some(toast_cb), LPARAM(&ctx as *const EnumCtx as isize));
    }
}

/// Apply whatever `HIDING` says right now.
///
/// The flag is read under `APPLY_LOCK`. A worker that observed `true` before
/// `HmwUnhideAll` stored `false` then blocks on the lock and, once it runs,
/// writes `WDA_NONE` instead of putting the hide back.
fn apply_hiding_flag() {
    let _guard = APPLY_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let hidden = HIDING.load(Ordering::SeqCst);
    set_all_windows(hidden);
}

/// Same serialization as `apply_hiding_flag`, for toast popups only.
fn apply_toast_flag() {
    let _guard = APPLY_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let hidden = HIDE_TOASTS.load(Ordering::SeqCst);
    set_toast_windows(hidden);
}

fn ensure_worker() {
    if WORKER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    // Safe to spawn here: exported functions run on their own remote thread,
    // not under the loader lock.
    std::thread::spawn(|| loop {
        if HIDING.load(Ordering::SeqCst) {
            apply_hiding_flag();
        }
        if HIDE_TOASTS.load(Ordering::SeqCst) {
            apply_toast_flag();
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    });
}

fn set_single(hwnd_param: *mut c_void, hidden: bool) {
    if hwnd_param.is_null() {
        return;
    }
    let hwnd = HWND(hwnd_param);
    set_affinity(
        hwnd,
        if hidden {
            WDA_EXCLUDEFROMCAPTURE.0
        } else {
            WDA_NONE.0
        },
    );
}

fn set_tray(hwnd_param: *mut c_void, visible: bool) {
    if hwnd_param.is_null() {
        return;
    }
    let hwnd = HWND(hwnd_param);
    unsafe {
        // Best-effort; ignore COM failures.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if let Ok(taskbar) =
            CoCreateInstance::<_, ITaskbarList>(&TaskbarList, None, CLSCTX_INPROC_SERVER)
        {
            let _ = taskbar.HrInit();
            if visible {
                let _ = taskbar.AddTab(hwnd);
            } else {
                let _ = taskbar.DeleteTab(hwnd);
            }
        }
    }
}

// --- Exported entry points (CreateRemoteThread targets) --------------------
// Each matches LPTHREAD_START_ROUTINE: extern "system" fn(*mut c_void) -> u32.

#[no_mangle]
pub extern "system" fn HmwHideAll(_param: *mut c_void) -> u32 {
    HIDING.store(true, Ordering::SeqCst);
    ensure_worker();
    apply_hiding_flag();
    0
}

#[no_mangle]
pub extern "system" fn HmwUnhideAll(_param: *mut c_void) -> u32 {
    HIDING.store(false, Ordering::SeqCst);
    apply_hiding_flag();
    0
}

#[no_mangle]
pub extern "system" fn HmwHideToasts(_param: *mut c_void) -> u32 {
    HIDE_TOASTS.store(true, Ordering::SeqCst);
    ensure_worker();
    apply_toast_flag();
    0
}

#[no_mangle]
pub extern "system" fn HmwUnhideToasts(_param: *mut c_void) -> u32 {
    HIDE_TOASTS.store(false, Ordering::SeqCst);
    apply_toast_flag();
    0
}

#[no_mangle]
pub extern "system" fn HmwHideWindow(param: *mut c_void) -> u32 {
    set_single(param, true);
    0
}

#[no_mangle]
pub extern "system" fn HmwUnhideWindow(param: *mut c_void) -> u32 {
    set_single(param, false);
    0
}

#[no_mangle]
pub extern "system" fn HmwHideTray(param: *mut c_void) -> u32 {
    set_tray(param, false);
    0
}

#[no_mangle]
pub extern "system" fn HmwUnhideTray(param: *mut c_void) -> u32 {
    set_tray(param, true);
    0
}
