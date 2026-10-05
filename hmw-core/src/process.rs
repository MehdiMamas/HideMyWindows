//! Process enumeration and inspection (Windows only).

use crate::model::ProcessInfo;
use crate::Result;
use windows::Win32::Foundation::{CloseHandle, HANDLE, MAX_PATH};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    IsWow64Process, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
    PROCESS_QUERY_LIMITED_INFORMATION,
};

/// A RAII wrapper so handles are always closed.
pub struct SafeHandle(pub HANDLE);
impl Drop for SafeHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// Read a wide (UTF-16) buffer up to its first NUL into a String.
pub(crate) fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Enumerate all running processes.
pub fn list_processes() -> Result<Vec<ProcessInfo>> {
    let mut out = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
        let snapshot = SafeHandle(snapshot);

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot.0, &mut entry).is_ok() {
            loop {
                let name = wide_to_string(&entry.szExeFile);
                let pid = entry.th32ProcessID;
                if pid != 0 {
                    out.push(ProcessInfo {
                        pid,
                        name,
                        path: process_path(pid).unwrap_or_default(),
                    });
                }
                if Process32NextW(snapshot.0, &mut entry).is_err() {
                    break;
                }
            }
        }
    }
    // Stable, human-friendly ordering.
    out.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.pid.cmp(&b.pid))
    });
    Ok(out)
}

/// Full image path of a process, if accessible.
pub fn process_path(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let handle = SafeHandle(handle);
        let mut buf = [0u16; MAX_PATH as usize];
        let mut size = buf.len() as u32;
        QueryFullProcessImageNameW(
            handle.0,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut size,
        )
        .ok()?;
        Some(wide_to_string(&buf[..size as usize]))
    }
}

/// The file name (with extension) of a process, e.g. `notepad.exe`.
pub fn process_name(pid: u32) -> Option<String> {
    process_path(pid).map(|p| {
        std::path::Path::new(&p)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or(p)
    })
}

/// True if the target process is 64-bit (always false on a 32-bit OS).
pub fn is_process_64bit(handle: HANDLE) -> Result<bool> {
    if !is_os_64bit() {
        return Ok(false);
    }
    let mut wow64 = windows::Win32::Foundation::BOOL(0);
    unsafe {
        IsWow64Process(handle, &mut wow64)?;
    }
    // A 64-bit process is NOT running under WOW64.
    Ok(!wow64.as_bool())
}

/// Is the operating system 64-bit?
pub fn is_os_64bit() -> bool {
    cfg!(target_pointer_width = "64") || std::env::var("PROCESSOR_ARCHITEW6432").is_ok()
}
