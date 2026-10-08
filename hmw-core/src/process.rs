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

/// True when `pid` still refers to a running process.
pub fn process_is_alive(pid: u32) -> bool {
    process_started_at(pid).is_some()
}

/// Stable identity for monitored launches, avoiding PID reuse and handle leaks.
pub fn process_started_at(pid: u32) -> Option<u64> {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::{GetExitCodeProcess, GetProcessTimes};
    if pid == 0 {
        return None;
    }
    unsafe {
        let handle = SafeHandle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?);
        let mut exit_code = 0;
        GetExitCodeProcess(handle.0, &mut exit_code).ok()?;
        if exit_code != 259 {
            return None;
        } // STILL_ACTIVE
        let (mut created, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user).ok()?;
        Some((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
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

/// Background services and other sign-in sessions cannot create windows on
/// this user's desktop. Do not inject them while anticipating UI processes.
pub fn in_current_session(pid: u32) -> bool {
    use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    unsafe {
        let mut target = 0;
        let mut current = 0;
        ProcessIdToSessionId(pid, &mut target).is_ok()
            && ProcessIdToSessionId(
                windows::Win32::System::Threading::GetCurrentProcessId(),
                &mut current,
            )
            .is_ok()
            && target == current
    }
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
