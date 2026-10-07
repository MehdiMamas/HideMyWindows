//! Window helpers: set display affinity, enumerate top-level windows,
//! read titles/classes, and map windows to processes.

use crate::Result;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, TRUE};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowDisplayAffinity, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindow, IsWindowVisible, SetWindowDisplayAffinity,
    WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
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

/// Read the window's actual capture-exclusion state rather than its saved setting.
/// WDA_MONITOR blacks out captured content but does not exclude the window.
pub fn is_capture_hidden(hwnd: isize) -> Result<bool> {
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    let mut affinity = 0;
    unsafe { GetWindowDisplayAffinity(hwnd, &mut affinity)? };
    Ok(affinity == WDA_EXCLUDEFROMCAPTURE.0)
}

/// Observe all visible top-level windows, including those owned by other apps.
pub fn capture_snapshot() -> Result<crate::model::CaptureSnapshot> {
    use crate::model::{CaptureSnapshot, CaptureStatus};
    use std::collections::HashMap;

    let mut windows = HashMap::new();
    let mut by_process: HashMap<u32, Vec<Option<bool>>> = HashMap::new();
    for window in list_top_windows(true)? {
        let hidden = is_capture_hidden(window.hwnd).ok();
        windows.insert(window.hwnd, CaptureStatus::from_window_states([hidden]));
        by_process.entry(window.pid).or_default().push(hidden);
    }
    Ok(CaptureSnapshot {
        windows,
        processes: by_process
            .into_iter()
            .map(|(pid, states)| (pid, CaptureStatus::from_window_states(states)))
            .collect(),
    })
}

/// True when `hwnd` still refers to a window.
pub fn window_is_alive(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    unsafe { IsWindow(hwnd).as_bool() }
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

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, CW_USEDEFAULT, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
        WS_VISIBLE,
    };

    #[test]
    fn capture_status_rejects_an_invalid_window() {
        assert!(is_capture_hidden(0).is_err());
    }

    #[test]
    fn capture_status_reads_back_the_window_affinity() {
        struct TestWindow(HWND);
        impl Drop for TestWindow {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                }
            }
        }

        let window = TestWindow(unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("HideMyWindows capture status test"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                100,
                100,
                None,
                None,
                None,
                None,
            )
            .expect("create test window")
        });
        let hwnd = window.0 .0 as isize;
        assert!(!is_capture_hidden(hwnd).unwrap());
        set_capture_hidden(hwnd, true).unwrap();
        assert!(is_capture_hidden(hwnd).unwrap());
        assert_eq!(
            capture_snapshot().unwrap().windows.get(&hwnd),
            Some(&crate::model::CaptureStatus::Hidden)
        );
        set_capture_hidden(hwnd, false).unwrap();
        assert!(!is_capture_hidden(hwnd).unwrap());
        assert_eq!(
            capture_snapshot().unwrap().windows.get(&hwnd),
            Some(&crate::model::CaptureStatus::Visible)
        );
    }
}
