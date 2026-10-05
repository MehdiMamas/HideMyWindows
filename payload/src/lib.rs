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

use windows::Win32::Foundation::{BOOL, HMODULE, HWND, LPARAM, TRUE};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
use windows::Win32::UI::Shell::{ITaskbarList, TaskbarList};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
};

mod toasts;

/// Whether "hide all windows of this process" is currently active.
static HIDING: AtomicBool = AtomicBool::new(false);
/// Whether toast popups in this process are excluded from capture.
static HIDE_TOASTS: AtomicBool = AtomicBool::new(false);
/// Whether the background re-apply worker has been started.
static WORKER_STARTED: AtomicBool = AtomicBool::new(false);

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
        let _ = SetWindowDisplayAffinity(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::WINDOW_DISPLAY_AFFINITY(ctx.affinity),
        );
    }
    TRUE
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
        let _ = SetWindowDisplayAffinity(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::WINDOW_DISPLAY_AFFINITY(ctx.affinity),
        );
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

fn ensure_worker() {
    if WORKER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    // Safe to spawn here: exported functions run on their own remote thread,
    // not under the loader lock.
    std::thread::spawn(|| loop {
        if HIDING.load(Ordering::SeqCst) {
            set_all_windows(true);
        }
        if HIDE_TOASTS.load(Ordering::SeqCst) {
            set_toast_windows(true);
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    });
}

fn set_single(hwnd_param: *mut c_void, hidden: bool) {
    if hwnd_param.is_null() {
        return;
    }
    let hwnd = HWND(hwnd_param);
    unsafe {
        let _ = SetWindowDisplayAffinity(
            hwnd,
            if hidden {
                WDA_EXCLUDEFROMCAPTURE
            } else {
                WDA_NONE
            },
        );
    }
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
    set_all_windows(true);
    0
}

#[no_mangle]
pub extern "system" fn HmwUnhideAll(_param: *mut c_void) -> u32 {
    HIDING.store(false, Ordering::SeqCst);
    set_all_windows(false);
    0
}

#[no_mangle]
pub extern "system" fn HmwHideToasts(_param: *mut c_void) -> u32 {
    HIDE_TOASTS.store(true, Ordering::SeqCst);
    ensure_worker();
    set_toast_windows(true);
    0
}

#[no_mangle]
pub extern "system" fn HmwUnhideToasts(_param: *mut c_void) -> u32 {
    HIDE_TOASTS.store(false, Ordering::SeqCst);
    set_toast_windows(false);
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
