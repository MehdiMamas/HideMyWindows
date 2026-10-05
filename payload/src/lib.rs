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
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetWindowDisplayAffinity,
    WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
};

/// Whether "hide all windows of this process" is currently active.
static HIDING: AtomicBool = AtomicBool::new(false);
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
