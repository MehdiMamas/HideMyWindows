//! Undo capture-exclusion and taskbar hides that outlive the app.
//!
//! Both this version and the original .NET app inject a DLL into the target
//! process. Uninstall deletes HideMyWindows, but the DLL stays mapped and
//! keeps the hide in force. This module calls the unhide exports already
//! loaded in those processes, then deletes the files and autostart value the
//! installers leave behind.
//!
//! Only processes that still have one of our DLLs loaded are touched. Other
//! apps set `WDA_EXCLUDEFROMCAPTURE` on their own windows, and those stay as
//! they are.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::process::{list_processes, wide_to_string, SafeHandle};
use crate::window::list_top_windows;
use crate::{Error, Result};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, HANDLE, WAIT_FAILED, WAIT_TIMEOUT};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
#[cfg(target_arch = "x86")]
use windows::Win32::System::Diagnostics::Debug::FlushInstructionCache;
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW,
    MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
    PAGE_READWRITE,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{
    CreateRemoteThread, GetCurrentProcess, GetCurrentProcessId, GetExitCodeProcess, OpenProcess,
    OpenProcessToken, TerminateProcess, WaitForSingleObject, INFINITE, LPTHREAD_START_ROUTINE,
    PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION, PROCESS_TERMINATE, PROCESS_VM_OPERATION,
    PROCESS_VM_READ, PROCESS_VM_WRITE,
};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};

/// The inner function-pointer type of `LPTHREAD_START_ROUTINE`.
type ThreadStart = unsafe extern "system" fn(*mut core::ffi::c_void) -> u32;

const LEGACY_X64: &str = "HideMyWindows.DLL.x64.dll";
const LEGACY_X86: &str = "HideMyWindows.DLL.Win32.dll";
const PAYLOAD: &str = "hmw_payload.dll";
const SHELL_PACKAGE: &str = "Microsoft.Windows.ShellExperienceHost_cw5n1h2txyewy";
const RUN_VALUE: &str = "HideMyWindows";

/// Which injected DLL a module name belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InjectedKind {
    /// `hmw_payload.dll` from this version.
    Payload,
    /// `HideMyWindows.DLL.*.dll` from the original app.
    Legacy,
}

/// What [`run`] should do besides walking processes.
#[derive(Debug, Clone)]
pub struct ReleaseOptions {
    /// Show a UAC prompt when a user-session process refuses access.
    /// Spawned helpers pass `false` so only the top-level process elevates.
    pub allow_elevate: bool,
    /// The process is already elevated, or was relaunched with `--elevated`.
    pub already_elevated: bool,
    /// 32-bit `hmw-release` to run for WOW64 targets. x64 only.
    pub wow64_helper: Option<PathBuf>,
}

/// Outcome of one release pass. Partial success is normal: protected system
/// processes cannot be opened, and a loaded DLL file often cannot be deleted.
#[derive(Debug, Default)]
pub struct ReleaseReport {
    pub released: Vec<String>,
    pub removed: Vec<String>,
    pub errors: Vec<String>,
    /// User-session processes that denied the rights needed to release a hide.
    pub access_denied: u32,
}

impl ReleaseReport {
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();
        if self.released.is_empty() {
            lines.push("No injected HideMyWindows modules were found.".to_string());
        } else {
            lines.push(format!("Released {}:", self.released.len()));
            for item in &self.released {
                lines.push(format!("  {item}"));
            }
        }
        if !self.removed.is_empty() {
            lines.push("Removed leftovers:".to_string());
            for item in &self.removed {
                lines.push(format!("  {item}"));
            }
        }
        if !self.errors.is_empty() {
            lines.push("Problems:".to_string());
            for item in &self.errors {
                lines.push(format!("  {item}"));
            }
        }
        if self.access_denied > 0 {
            lines.push(format!(
                "{} user process{} could not be opened.",
                self.access_denied,
                if self.access_denied == 1 { "" } else { "es" }
            ));
        }
        lines.join("\n")
    }
}

/// Build options from the process command line.
///
/// `--no-elevate` is set by a parent that will elevate itself if needed.
/// `--elevated` marks the process started from the UAC prompt.
pub fn options_from_args(wow64_helper: Option<PathBuf>) -> ReleaseOptions {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flagged = args.iter().any(|arg| arg == "--elevated");
    ReleaseOptions {
        allow_elevate: !args.iter().any(|arg| arg == "--no-elevate"),
        already_elevated: flagged || is_elevated(),
        wow64_helper,
    }
}

/// Stop our own still-running app, unhide every injected process we can open,
/// delete leftovers, and (on x64) ask the 32-bit helper to do the same.
///
/// Returns 0 after a finished attempt, including when the user cancels UAC.
/// Returns 2 when access was denied and this process was told not to elevate,
/// so the parent can relaunch itself elevated.
pub fn run(options: ReleaseOptions) -> i32 {
    let mut report = ReleaseReport::default();
    stop_other_instances(&mut report);
    release_loaded_modules(&mut report);
    remove_leftovers(&mut report);

    let mut helper_needs_elevation = false;
    if cfg!(target_arch = "x86_64") {
        if let Some(path) = options
            .wow64_helper
            .as_deref()
            .filter(|path| path.is_file())
            .map(Path::to_path_buf)
            .or_else(wow64_helper_beside_exe)
        {
            match run_wow64_helper(&path) {
                Ok(code) => {
                    if code == 2 {
                        helper_needs_elevation = true;
                    }
                }
                Err(error) => report.errors.push(format!("32-bit helper: {error}")),
            }
        }
    }

    let needs_elevation =
        (report.access_denied > 0 || helper_needs_elevation) && !options.already_elevated;
    if needs_elevation && options.allow_elevate {
        println!("Requesting administrator approval to release elevated apps...");
        match relaunch_elevated() {
            Ok(code) => return code as i32,
            Err(error) => report
                .errors
                .push(format!("Could not request administrator rights: {error}")),
        }
    }

    println!("{}", report.summary());
    if needs_elevation && !options.allow_elevate {
        2
    } else {
        0
    }
}

fn classify_module(name: &str) -> Option<InjectedKind> {
    if name.eq_ignore_ascii_case(PAYLOAD) {
        Some(InjectedKind::Payload)
    } else if name.eq_ignore_ascii_case(LEGACY_X64) || name.eq_ignore_ascii_case(LEGACY_X86) {
        Some(InjectedKind::Legacy)
    } else {
        None
    }
}

fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let token = SafeHandle(token);
        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0u32;
        if GetTokenInformation(
            token.0,
            TokenElevation,
            Some(&mut elevation as *mut TOKEN_ELEVATION as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
        .is_err()
        {
            return false;
        }
        elevation.TokenIsElevated != 0
    }
}

fn is_access_denied(error: &windows::core::Error) -> bool {
    error.code() == ERROR_ACCESS_DENIED.into()
}

/// Session 0 is services. A denial there is not a hide we can reach by elevating.
fn is_user_session(pid: u32) -> bool {
    let mut session = 0u32;
    unsafe {
        if ProcessIdToSessionId(pid, &mut session).is_err() {
            return false;
        }
    }
    session != 0
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn parent_pid(pid: u32) -> Option<u32> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let snapshot = SafeHandle(snapshot);
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot.0, &mut entry).is_err() {
            return None;
        }
        loop {
            if entry.th32ProcessID == pid {
                let parent = entry.th32ParentProcessID;
                return (parent != 0).then_some(parent);
            }
            if Process32NextW(snapshot.0, &mut entry).is_err() {
                break;
            }
        }
        None
    }
}

fn app_exe_names() -> Vec<String> {
    let mut names = vec!["hidemywindows.exe".to_string()];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(name) = exe
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        {
            if !names
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&name))
            {
                names.push(name);
            }
        }
    }
    names
}

/// The tray app's watcher re-injects hides until that process is gone.
/// Skip this process and its parent so a helper does not kill the release
/// command that launched it.
fn stop_other_instances(report: &mut ReleaseReport) {
    let self_pid = unsafe { GetCurrentProcessId() };
    let parent = parent_pid(self_pid);
    let names = app_exe_names();
    let processes = match list_processes() {
        Ok(processes) => processes,
        Err(error) => {
            report.errors.push(error.0);
            return;
        }
    };
    let mut stopped = false;
    for process in processes {
        if process.pid == self_pid || Some(process.pid) == parent {
            continue;
        }
        if !names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&process.name))
        {
            continue;
        }
        match terminate_pid(process.pid) {
            Ok(()) => {
                stopped = true;
                report.released.push(format!(
                    "Stopped {} ({}) so it cannot re-apply hides",
                    process.name, process.pid
                ));
            }
            Err(error) if is_access_denied(&error) && is_user_session(process.pid) => {
                report.access_denied += 1;
                report.errors.push(format!(
                    "Could not stop {} ({}): access denied",
                    process.name, process.pid
                ));
            }
            Err(error) => report.errors.push(format!(
                "Could not stop {} ({}): {error}",
                process.name, process.pid
            )),
        }
    }
    if stopped {
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn terminate_pid(pid: u32) -> windows::core::Result<()> {
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, false, pid)?;
        let handle = SafeHandle(handle);
        TerminateProcess(handle.0, 0)
    }
}

fn release_loaded_modules(report: &mut ReleaseReport) {
    let windows = list_top_windows(true).unwrap_or_default();
    let processes = match list_processes() {
        Ok(processes) => processes,
        Err(error) => {
            report.errors.push(error.0);
            return;
        }
    };
    let self_pid = unsafe { GetCurrentProcessId() };
    for process in processes {
        if process.pid == self_pid {
            continue;
        }
        match open_target(process.pid) {
            Ok(handle) => {
                if let Err(error) =
                    release_one(&handle, process.pid, &process.name, &windows, report)
                {
                    report
                        .errors
                        .push(format!("{} ({}): {error}", process.name, process.pid));
                }
            }
            Err(error) if is_access_denied(&error) && is_user_session(process.pid) => {
                // Elevating is only useful when we can see our DLL and still
                // cannot call it. Other denials are system processes we never
                // injected into.
                if matches!(loaded_modules(process.pid), Ok(Some(_))) {
                    report.access_denied += 1;
                    report.errors.push(format!(
                        "{} ({}) still has HideMyWindows loaded, but access was denied",
                        process.name, process.pid
                    ));
                }
            }
            Err(_) => {}
        }
    }
}

fn open_target(pid: u32) -> windows::core::Result<SafeHandle> {
    let handle = unsafe {
        OpenProcess(
            PROCESS_CREATE_THREAD
                | PROCESS_QUERY_INFORMATION
                | PROCESS_VM_OPERATION
                | PROCESS_VM_READ
                | PROCESS_VM_WRITE,
            false,
            pid,
        )?
    };
    Ok(SafeHandle(handle))
}

struct LoadedModules {
    legacy: Option<usize>,
    payload: Option<usize>,
}

fn release_one(
    handle: &SafeHandle,
    pid: u32,
    name: &str,
    windows: &[crate::window::TopWindow],
    report: &mut ReleaseReport,
) -> Result<()> {
    let Some(modules) = loaded_modules(pid)? else {
        return Ok(());
    };
    // The original hook rewrites every affinity change back to
    // WDA_EXCLUDEFROMCAPTURE until UnhideAllWindows clears its flag.
    if let Some(base) = modules.legacy {
        call_legacy(
            handle.0,
            export_remote(handle.0, base, "UnhideAllWindows")?,
            None,
        )?;
    }
    if let Some(base) = modules.payload {
        call_payload(
            handle.0,
            export_remote(handle.0, base, "HmwUnhideAll")?,
            None,
        )?;
        if let Ok(toasts) = export_remote(handle.0, base, "HmwUnhideToasts") {
            call_payload(handle.0, toasts, None)?;
        }
    }

    let hwnds: Vec<isize> = windows
        .iter()
        .filter(|window| window.pid == pid)
        .map(|window| window.hwnd)
        .collect();
    let payload_tray = modules
        .payload
        .and_then(|base| export_remote(handle.0, base, "HmwUnhideTray").ok());
    let legacy_tray = modules
        .legacy
        .and_then(|base| export_remote(handle.0, base, "UnhideTrayIcon").ok());
    for hwnd in hwnds {
        let restored = if let Some(unhide_tray) = payload_tray {
            call_payload(handle.0, unhide_tray, Some(hwnd as usize))
        } else if let Some(unhide_tray) = legacy_tray {
            call_legacy_tray(handle.0, unhide_tray, hwnd)
        } else {
            Ok(())
        };
        if let Err(error) = restored {
            report
                .errors
                .push(format!("{name} ({pid}) taskbar button {hwnd:#x}: {error}"));
        }
    }

    let mut what = Vec::new();
    if modules.legacy.is_some() {
        what.push("original hooks");
    }
    if modules.payload.is_some() {
        what.push("capture exclusion");
    }
    what.push("taskbar buttons");
    report
        .released
        .push(format!("{name} ({pid}): {}", what.join(", ")));
    Ok(())
}

/// `None` when the process is a different architecture (the matching helper
/// owns those). `Err` when the snapshot itself failed for another reason.
fn loaded_modules(pid: u32) -> Result<Option<LoadedModules>> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, pid) };
    let snapshot = match snapshot {
        Ok(snapshot) => SafeHandle(snapshot),
        Err(error) if is_partial_copy(&error) => return Ok(None),
        Err(error) => return Err(Error(error.to_string())),
    };
    let mut found = LoadedModules {
        legacy: None,
        payload: None,
    };
    let mut any = false;
    unsafe {
        let mut entry = MODULEENTRY32W {
            dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        if Module32FirstW(snapshot.0, &mut entry).is_err() {
            return Ok(None);
        }
        loop {
            let name = wide_to_string(&entry.szModule);
            match classify_module(&name) {
                Some(InjectedKind::Legacy) => {
                    any = true;
                    found.legacy = Some(entry.modBaseAddr as usize);
                }
                Some(InjectedKind::Payload) => {
                    any = true;
                    found.payload = Some(entry.modBaseAddr as usize);
                }
                None => {}
            }
            if Module32NextW(snapshot.0, &mut entry).is_err() {
                break;
            }
        }
    }
    Ok(any.then_some(found))
}

fn is_partial_copy(error: &windows::core::Error) -> bool {
    let code = error.code().0 as u32;
    code == 299 || code & 0xFFFF == 299
}

fn export_remote(process: HANDLE, base: usize, name: &str) -> Result<usize> {
    export_va(base, name, |addr, buf| read_remote(process, addr, buf))
        .ok_or_else(|| Error(format!("Could not resolve {name} in the loaded module")))
}

fn read_remote(process: HANDLE, addr: usize, buf: &mut [u8]) -> bool {
    if addr == 0 || buf.is_empty() {
        return false;
    }
    let mut read = 0usize;
    unsafe {
        ReadProcessMemory(
            process,
            addr as *const core::ffi::c_void,
            buf.as_mut_ptr() as *mut core::ffi::c_void,
            buf.len(),
            Some(&mut read),
        )
        .is_ok()
            && read == buf.len()
    }
}

/// Resolve `name` in a module mapped at `base`. `read` fills a buffer from an
/// absolute address and returns false on a short or failed read.
fn export_va(
    base: usize,
    name: &str,
    mut read: impl FnMut(usize, &mut [u8]) -> bool,
) -> Option<usize> {
    let mut dos = [0u8; 0x40];
    if !read(base, &mut dos) || &dos[0..2] != b"MZ" {
        return None;
    }
    let e_lfanew = read_u32(&dos, 0x3C)? as usize;
    let mut nt = [0u8; 0x108];
    if !read(base.checked_add(e_lfanew)?, &mut nt) || &nt[0..4] != b"PE\0\0" {
        return None;
    }
    let magic = read_u16(&nt, 24)?;
    let data_dir = match magic {
        0x20B => 24 + 112,
        0x10B => 24 + 96,
        _ => return None,
    };
    let export_rva = read_u32(&nt, data_dir)?;
    let export_size = read_u32(&nt, data_dir + 4)?;
    if export_rva == 0 {
        return None;
    }
    let mut directory = [0u8; 40];
    if !read(base.checked_add(export_rva as usize)?, &mut directory) {
        return None;
    }
    let num_names = read_u32(&directory, 24)?;
    if num_names == 0 || num_names > 4096 {
        return None;
    }
    let funcs_rva = read_u32(&directory, 28)? as usize;
    let names_rva = read_u32(&directory, 32)? as usize;
    let ords_rva = read_u32(&directory, 36)? as usize;
    let mut name_rvas = vec![0u8; num_names as usize * 4];
    let mut ordinals = vec![0u8; num_names as usize * 2];
    if !read(base.checked_add(names_rva)?, &mut name_rvas)
        || !read(base.checked_add(ords_rva)?, &mut ordinals)
    {
        return None;
    }
    for index in 0..num_names as usize {
        let name_rva = read_u32(&name_rvas, index * 4)? as usize;
        let mut name_buf = [0u8; 64];
        if !read(base.checked_add(name_rva)?, &mut name_buf) {
            continue;
        }
        let end = name_buf
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(name_buf.len());
        if &name_buf[..end] != name.as_bytes() {
            continue;
        }
        let ordinal = read_u16(&ordinals, index * 2)? as usize;
        let mut func_buf = [0u8; 4];
        let func_addr = base.checked_add(funcs_rva)?.checked_add(ordinal * 4)?;
        if !read(func_addr, &mut func_buf) {
            return None;
        }
        let func_rva = u32::from_le_bytes(func_buf);
        if export_size != 0 && func_rva >= export_rva && func_rva < export_rva + export_size {
            return None;
        }
        return base.checked_add(func_rva as usize);
    }
    None
}

fn read_u16(buf: &[u8], offset: usize) -> Option<u16> {
    let bytes = buf.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(buf: &[u8], offset: usize) -> Option<u32> {
    let bytes = buf.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn call_payload(process: HANDLE, func: usize, param: Option<usize>) -> Result<()> {
    start_thread(process, func, param)
}

fn call_legacy(process: HANDLE, func: usize, param: Option<usize>) -> Result<()> {
    // The original exports are zero-argument cdecl. On x64 and ARM64 that
    // matches the thread start convention. On x86 the thread stub expects
    // stdcall, so a few-byte thunk performs the call and returns cleanly.
    #[cfg(target_arch = "x86")]
    {
        call_through_x86_stub(process, func, param)
    }
    #[cfg(not(target_arch = "x86"))]
    {
        start_thread(process, func, param)
    }
}

fn call_legacy_tray(process: HANDLE, func: usize, hwnd: isize) -> Result<()> {
    let remote = write_remote(process, &(hwnd as usize).to_ne_bytes(), false)?;
    call_legacy(process, func, Some(remote.addr as usize))
}

fn start_thread(process: HANDLE, func: usize, param: Option<usize>) -> Result<()> {
    let start: LPTHREAD_START_ROUTINE =
        Some(unsafe { std::mem::transmute::<usize, ThreadStart>(func) });
    let param = param.map(|value| value as *const core::ffi::c_void);
    let thread = unsafe { CreateRemoteThread(process, None, 0, start, param, 0, None)? };
    let thread = SafeHandle(thread);
    let wait = unsafe { WaitForSingleObject(thread.0, 8_000) };
    if wait == WAIT_TIMEOUT {
        return Err(Error(
            "The target did not finish releasing hides in time".into(),
        ));
    }
    if wait == WAIT_FAILED {
        return Err(Error("WaitForSingleObject failed".into()));
    }
    Ok(())
}

/// Machine code for an x86 stdcall thread start that calls a cdecl export.
///
/// `with_param` pushes the thread parameter (a remote pointer) before the call.
/// Compiled on every architecture so the encoding can be tested; only x86 calls it.
#[cfg_attr(not(target_arch = "x86"), allow(dead_code))]
fn x86_stdcall_stub(func: u32, stub_addr: u32, with_param: bool) -> Vec<u8> {
    if with_param {
        // push dword ptr [esp+4]; call func; add esp, 4; xor eax, eax; ret 4
        let rel = func.wrapping_sub(stub_addr.wrapping_add(9)) as i32;
        let mut code = vec![0xFF, 0x74, 0x24, 0x04, 0xE8];
        code.extend_from_slice(&rel.to_le_bytes());
        code.extend_from_slice(&[0x83, 0xC4, 0x04, 0x33, 0xC0, 0xC2, 0x04, 0x00]);
        code
    } else {
        // call func; xor eax, eax; ret 4
        let rel = func.wrapping_sub(stub_addr.wrapping_add(5)) as i32;
        let mut code = vec![0xE8];
        code.extend_from_slice(&rel.to_le_bytes());
        code.extend_from_slice(&[0x33, 0xC0, 0xC2, 0x04, 0x00]);
        code
    }
}

#[cfg(target_arch = "x86")]
fn call_through_x86_stub(process: HANDLE, func: usize, param: Option<usize>) -> Result<()> {
    let remote = write_remote(process, &[], true)?;
    let code = x86_stdcall_stub(func as u32, remote.addr as u32, param.is_some());
    unsafe {
        WriteProcessMemory(
            process,
            remote.addr,
            code.as_ptr() as *const core::ffi::c_void,
            code.len(),
            None,
        )?;
        let _ = FlushInstructionCache(process, Some(remote.addr), code.len());
    }
    start_thread(process, remote.addr as usize, param)
}

struct RemoteMem {
    process: HANDLE,
    addr: *mut core::ffi::c_void,
}

impl Drop for RemoteMem {
    fn drop(&mut self) {
        if !self.addr.is_null() {
            unsafe {
                let _ = VirtualFreeEx(self.process, self.addr, 0, MEM_RELEASE);
            }
        }
    }
}

fn write_remote(process: HANDLE, bytes: &[u8], executable: bool) -> Result<RemoteMem> {
    let size = bytes.len().max(64);
    let protect = if executable {
        PAGE_EXECUTE_READWRITE
    } else {
        PAGE_READWRITE
    };
    let addr = unsafe { VirtualAllocEx(process, None, size, MEM_COMMIT | MEM_RESERVE, protect) };
    if addr.is_null() {
        return Err(Error("VirtualAllocEx failed".into()));
    }
    let remote = RemoteMem { process, addr };
    if !bytes.is_empty() {
        unsafe {
            WriteProcessMemory(
                process,
                addr,
                bytes.as_ptr() as *const core::ffi::c_void,
                bytes.len(),
                None,
            )?;
        }
    }
    Ok(remote)
}

fn remove_leftovers(report: &mut ReleaseReport) {
    match delete_run_value() {
        Ok(true) => report.removed.push(format!("HKCU Run value {RUN_VALUE}")),
        Ok(false) => {}
        Err(error) => report.errors.push(error),
    }

    let mut config_dir = None;
    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = PathBuf::from(appdata).join("HideMyWindows");
        try_remove_file(&dir.join("config.json"), report);
        try_remove_file(&dir.join("HideMyWindows.json"), report);
        config_dir = Some(dir);
    }
    let temp = std::env::temp_dir();
    for name in [
        LEGACY_X64,
        LEGACY_X86,
        "HideMyWindows.DLL.x64.dll.bak",
        "HideMyWindows.DLL.Win32.dll.bak",
    ] {
        try_remove_file(&temp.join(name), report);
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        try_remove_file(
            &PathBuf::from(local)
                .join("Packages")
                .join(SHELL_PACKAGE)
                .join("TempState")
                .join(PAYLOAD),
            report,
        );
    }
    if let Some(dir) = config_dir {
        if dir.is_dir()
            && dir
                .read_dir()
                .ok()
                .and_then(|mut entries| entries.next())
                .is_none()
            && std::fs::remove_dir(&dir).is_ok()
        {
            report.removed.push(dir.display().to_string());
        }
    }
}

/// `Ok(true)` when the value was deleted, `Ok(false)` when it was already gone.
fn delete_run_value() -> std::result::Result<bool, String> {
    unsafe {
        let mut key = HKEY::default();
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            0,
            KEY_SET_VALUE,
            &mut key,
        );
        if status.0 != 0 {
            return Err(format!("Could not open Run key: {}", status.0));
        }
        let name = to_wide(RUN_VALUE);
        let status = RegDeleteValueW(key, PCWSTR(name.as_ptr()));
        let _ = RegCloseKey(key);
        // 2 is "not found", which means there was nothing to undo.
        if status.0 == 2 {
            return Ok(false);
        }
        if status.0 != 0 {
            return Err(format!("Could not clear Run value: {}", status.0));
        }
    }
    Ok(true)
}

fn try_remove_file(path: &Path, report: &mut ReleaseReport) {
    if !path.is_file() {
        return;
    }
    match std::fs::remove_file(path) {
        Ok(()) => report.removed.push(path.display().to_string()),
        Err(error) => report
            .errors
            .push(format!("Could not remove {}: {error}", path.display())),
    }
}

fn wow64_helper_beside_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidates = [
        dir.join("hmw-release-x86.exe"),
        dir.join("resources").join("hmw-release-x86.exe"),
    ];
    candidates.into_iter().find(|path| path.is_file())
}

fn run_wow64_helper(path: &Path) -> Result<i32> {
    let status = std::process::Command::new(path)
        .arg("--release-all")
        .arg("--no-elevate")
        .status()?;
    Ok(status.code().unwrap_or(1))
}

fn relaunch_elevated() -> Result<u32> {
    let exe = std::env::current_exe()?;
    let exe_wide = to_wide(&exe.to_string_lossy());
    let params = to_wide("--release-all --elevated");
    let verb = to_wide("runas");
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(exe_wide.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: 1,
        ..Default::default()
    };
    unsafe {
        ShellExecuteExW(&mut info)?;
    }
    let process = elevated_process(&info);
    if process.is_invalid() {
        return Err(Error(
            "Administrator approval was cancelled or did not start".into(),
        ));
    }
    let process = SafeHandle(process);
    unsafe {
        WaitForSingleObject(process.0, INFINITE);
        let mut code = 0u32;
        GetExitCodeProcess(process.0, &mut code)?;
        Ok(code)
    }
}

fn elevated_process(info: &SHELLEXECUTEINFOW) -> HANDLE {
    #[cfg(target_arch = "x86")]
    {
        // The x86 struct is packed; a normal field read is misaligned.
        unsafe { std::ptr::addr_of!(info.hProcess).read_unaligned() }
    }
    #[cfg(not(target_arch = "x86"))]
    {
        info.hProcess
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_module, x86_stdcall_stub, InjectedKind};

    #[test]
    fn classifies_current_and_original_dlls() {
        assert_eq!(
            classify_module("hmw_payload.dll"),
            Some(InjectedKind::Payload)
        );
        assert_eq!(
            classify_module("HIDEMYWINDOWS.DLL.X64.DLL"),
            Some(InjectedKind::Legacy)
        );
        assert_eq!(
            classify_module("HideMyWindows.DLL.Win32.dll"),
            Some(InjectedKind::Legacy)
        );
        assert_eq!(classify_module("kernel32.dll"), None);
    }

    #[test]
    fn x86_stub_calls_the_export_and_returns_stdcall() {
        let func = 0x1000_u32;
        let stub = 0x2000_u32;
        let no_param = x86_stdcall_stub(func, stub, false);
        assert_eq!(no_param[0], 0xE8);
        let rel = i32::from_le_bytes(no_param[1..5].try_into().unwrap());
        assert_eq!(stub.wrapping_add(5).wrapping_add(rel as u32), func);
        assert_eq!(&no_param[5..], &[0x33, 0xC0, 0xC2, 0x04, 0x00]);

        let with_param = x86_stdcall_stub(func, stub, true);
        assert_eq!(&with_param[..5], &[0xFF, 0x74, 0x24, 0x04, 0xE8]);
        let rel = i32::from_le_bytes(with_param[5..9].try_into().unwrap());
        assert_eq!(stub.wrapping_add(9).wrapping_add(rel as u32), func);
        assert_eq!(
            &with_param[9..],
            &[0x83, 0xC4, 0x04, 0x33, 0xC0, 0xC2, 0x04, 0x00]
        );
    }

    #[test]
    fn resolves_a_real_mapped_export() {
        use super::export_va;
        use windows::core::s;
        use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

        unsafe {
            let module = GetModuleHandleW(windows::core::w!("kernel32.dll")).unwrap();
            let expected = GetProcAddress(module, s!("GetCurrentProcessId")).unwrap() as usize;
            let found = export_va(module.0 as usize, "GetCurrentProcessId", |addr, buf| {
                std::ptr::copy_nonoverlapping(addr as *const u8, buf.as_mut_ptr(), buf.len());
                true
            })
            .unwrap();
            assert_eq!(found, expected);
        }
    }
}
