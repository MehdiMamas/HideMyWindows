//! Window helpers: set display affinity, enumerate top-level windows,
//! read titles/classes, and map windows to processes.

use crate::Result;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, TRUE};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
};

/// Hide or show a single window from screen capture.
/// Returns an error if the OS rejected the call (e.g. unsupported build).
pub fn set_capture_hidden(hwnd: isize, hidden: bool) -> Result<()> {
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    unsafe {
        SetWindowDisplayAffinity(
            hwnd,
            if hidden {
                WDA_EXCLUDEFROMCAPTURE
            } else {
                WDA_NONE
            },
        )?;
    }
    Ok(())
}

/// The process id that owns a window.
pub fn window_pid(hwnd: isize) -> u32 {
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    pid
}

/// A visible top-level window with basic metadata.
#[derive(Debug, Clone)]
pub struct TopWindow {
    pub hwnd: isize,
    pub pid: u32,
    pub title: String,
    pub class: String,
}

/// Read a window's title.
pub fn window_title(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        let read = GetWindowTextW(hwnd, &mut buf);
        crate::process::wide_to_string(&buf[..read as usize])
    }
}

/// Read a window's class name.
pub fn window_class(hwnd: HWND) -> String {
    unsafe {
        let mut buf = [0u16; 256];
        let read = GetClassNameW(hwnd, &mut buf);
        crate::process::wide_to_string(&buf[..read as usize])
    }
}

struct EnumState {
    windows: Vec<TopWindow>,
    visible_only: bool,
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let state = &mut *(lparam.0 as *mut EnumState);
    if state.visible_only && !IsWindowVisible(hwnd).as_bool() {
        return TRUE;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    state.windows.push(TopWindow {
        hwnd: hwnd.0 as isize,
        pid,
        title: window_title(hwnd),
        class: window_class(hwnd),
    });
    TRUE
}

/// Enumerate top-level windows (optionally only visible ones).
pub fn list_top_windows(visible_only: bool) -> Result<Vec<TopWindow>> {
    let mut state = EnumState {
        windows: Vec::new(),
        visible_only,
    };
    unsafe {
        EnumWindows(
            Some(enum_proc),
            LPARAM(&mut state as *mut EnumState as isize),
        )?;
    }
    Ok(state.windows)
}

/// All visible top-level windows owned by a given process.
pub fn windows_for_pid(pid: u32) -> Result<Vec<TopWindow>> {
    Ok(list_top_windows(true)?
        .into_iter()
        .filter(|w| w.pid == pid)
        .collect())
}
