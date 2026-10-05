//! Loads the HideMyWindows helper payload into a target process and calls its
//! exported functions, so windows owned by another process can be hidden from
//! screen capture.
//!
//! The technique is the classic `CreateRemoteThread` + `LoadLibraryW` loader:
//! we write the payload's path into the target, start a thread at
//! `LoadLibraryW`, then start further threads at the payload's exported
//! functions (resolved as `remote_module_base + local_export_rva`). The HWND a
//! function needs is passed directly as the thread parameter, so no extra
//! remote allocation is required for calls.
//!
//! v2 supports injecting into processes of the **same architecture** as the
//! HideMyWindows build (x64→x64, x86→x86, arm64→arm64). Cross-architecture
//! injection is intentionally not attempted; the UI surfaces a clear message.

use crate::process::{is_process_64bit, wide_to_string, SafeHandle};
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::Mutex;
use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::WAIT_FAILED;
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W, TH32CS_SNAPMODULE,
    TH32CS_SNAPMODULE32,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateRemoteThread, GetExitCodeThread, OpenProcess, WaitForSingleObject, INFINITE,
    LPTHREAD_START_ROUTINE, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION,
    PROCESS_VM_READ, PROCESS_VM_WRITE,
};

/// Cache of export-name -> RVA within a locally loaded copy of a payload DLL.
/// Keyed by the payload path. Resolving RVAs once avoids repeated local loads.
static RVA_CACHE: Mutex<Option<HashMap<String, HashMap<String, isize>>>> = Mutex::new(None);

/// The inner function-pointer type of `LPTHREAD_START_ROUTINE`, used to give
/// `transmute` an explicit target type.
type ThreadStart = unsafe extern "system" fn(*mut core::ffi::c_void) -> u32;

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn open_target(pid: u32) -> Result<SafeHandle> {
    let handle = unsafe {
        OpenProcess(
            PROCESS_CREATE_THREAD
                | PROCESS_QUERY_INFORMATION
                | PROCESS_VM_OPERATION
                | PROCESS_VM_WRITE
                | PROCESS_VM_READ,
            false,
            pid,
        )?
    };
    Ok(SafeHandle(handle))
}

/// Find the base address of `module_file_name` (e.g. `hmw_payload_x64.dll`)
/// within process `pid`, retrying briefly for freshly started processes.
fn remote_module_base(pid: u32, module_file_name: &str) -> Option<isize> {
    let wanted = module_file_name.to_lowercase();
    for _ in 0..10 {
        unsafe {
            if let Ok(snapshot) =
                CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid)
            {
                let snapshot = SafeHandle(snapshot);
                let mut entry = MODULEENTRY32W {
                    dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
                    ..Default::default()
                };
                if Module32FirstW(snapshot.0, &mut entry).is_ok() {
                    loop {
                        let name = wide_to_string(&entry.szModule).to_lowercase();
                        if name == wanted {
                            return Some(entry.modBaseAddr as isize);
                        }
                        if Module32NextW(snapshot.0, &mut entry).is_err() {
                            break;
                        }
                    }
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    None
}

/// Resolve (and cache) the RVAs of the payload's exports by loading a local
/// copy of the DLL. The local copy must be the same architecture as this
/// build (it is — we always pick the payload matching our own arch for
/// same-arch targets).
fn export_rvas(payload_path: &str) -> Result<HashMap<String, isize>> {
    {
        let guard = RVA_CACHE.lock().unwrap();
        if let Some(map) = guard.as_ref().and_then(|m| m.get(payload_path)) {
            return Ok(map.clone());
        }
    }

    const EXPORTS: &[&str] = &[
        "HmwHideAll",
        "HmwUnhideAll",
        "HmwHideWindow",
        "HmwUnhideWindow",
        "HmwHideTray",
        "HmwUnhideTray",
        "HmwHideToasts",
        "HmwUnhideToasts",
    ];

    let wide = to_wide(payload_path);
    let module = unsafe { LoadLibraryW(PCWSTR(wide.as_ptr()))? };
    if module.is_invalid() {
        return Err(Error("Failed to load payload DLL locally".into()));
    }
    let base = module.0 as isize;
    let mut map = HashMap::new();
    for &name in EXPORTS {
        let cname = std::ffi::CString::new(name).unwrap();
        let proc = unsafe { GetProcAddress(module, PCSTR(cname.as_ptr() as *const u8)) };
        match proc {
            Some(f) => {
                let addr = f as usize as isize;
                map.insert(name.to_string(), addr - base);
            }
            None => return Err(Error(format!("Payload is missing export {name}"))),
        }
    }

    let mut guard = RVA_CACHE.lock().unwrap();
    guard
        .get_or_insert_with(HashMap::new)
        .insert(payload_path.to_string(), map.clone());
    Ok(map)
}

/// Inject the payload into `pid` if it is not already present, returning the
/// remote base address of the payload module.
fn ensure_loaded(handle: &SafeHandle, pid: u32, payload_path: &str) -> Result<isize> {
    let file_name = std::path::Path::new(payload_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| Error("Invalid payload path".into()))?;

    if let Some(base) = remote_module_base(pid, &file_name) {
        return Ok(base);
    }

    // Write the payload path into the target process.
    let path_wide = to_wide(payload_path);
    let bytes = path_wide.len() * std::mem::size_of::<u16>();
    let remote_mem = unsafe {
        VirtualAllocEx(
            handle.0,
            None,
            bytes,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };
    if remote_mem.is_null() {
        return Err(Error("VirtualAllocEx failed".into()));
    }
    let result = (|| -> Result<()> {
        unsafe {
            WriteProcessMemory(
                handle.0,
                remote_mem,
                path_wide.as_ptr() as *const core::ffi::c_void,
                bytes,
                None,
            )?;
        }

        // LoadLibraryW lives at the same address in every same-arch process.
        let kernel32 = unsafe { GetModuleHandleW(PCWSTR(to_wide("kernel32.dll").as_ptr()))? };
        let load_library = unsafe { GetProcAddress(kernel32, windows::core::s!("LoadLibraryW")) }
            .ok_or_else(|| Error("Could not resolve LoadLibraryW".into()))?;

        let start: LPTHREAD_START_ROUTINE = Some(unsafe {
            std::mem::transmute::<unsafe extern "system" fn() -> isize, ThreadStart>(load_library)
        });
        let thread =
            unsafe { CreateRemoteThread(handle.0, None, 0, start, Some(remote_mem), 0, None)? };
        let thread = SafeHandle(thread);
        if unsafe { WaitForSingleObject(thread.0, INFINITE) } == WAIT_FAILED {
            return Err(Error("WaitForSingleObject failed".into()));
        }
        let mut exit = 0u32;
        unsafe { GetExitCodeThread(thread.0, &mut exit)? };
        if exit == 0 {
            return Err(Error(
                "LoadLibraryW failed in the target process. It may be protected, a different architecture, or unable to read the payload.".into(),
            ));
        }
        Ok(())
    })();

    unsafe {
        let _ = VirtualFreeEx(handle.0, remote_mem, 0, MEM_RELEASE);
    }
    result?;

    remote_module_base(pid, &file_name).ok_or_else(|| {
        Error(format!(
            "Payload did not appear in the target process. The target may be a different \
             architecture than HideMyWindows, protected, or elevated. ({file_name})"
        ))
    })
}

/// Run one of the payload's exported functions in the target process,
/// passing `hwnd` directly as the thread parameter (0 for whole-process calls).
pub fn call_export(pid: u32, payload_path: &str, export: &str, hwnd: isize) -> Result<()> {
    let handle = open_target(pid)?;

    // Guard against cross-architecture targets, which we don't support.
    let target_64 = is_process_64bit(handle.0)?;
    if target_64 != cfg!(target_pointer_width = "64") {
        return Err(Error(format!(
            "This {}-bit build of HideMyWindows cannot hide a {}-bit application. \
             Use the matching build for that target.",
            if cfg!(target_pointer_width = "64") {
                "64"
            } else {
                "32"
            },
            if target_64 { "64" } else { "32" },
        )));
    }

    let base = ensure_loaded(&handle, pid, payload_path)?;
    let rvas = export_rvas(payload_path)?;
    let rva = rvas
        .get(export)
        .copied()
        .ok_or_else(|| Error(format!("Unknown payload export {export}")))?;
    let func_addr = (base + rva) as *const core::ffi::c_void;

    let start: LPTHREAD_START_ROUTINE =
        Some(unsafe { std::mem::transmute::<*const core::ffi::c_void, ThreadStart>(func_addr) });
    let param = if hwnd != 0 {
        Some(hwnd as *const core::ffi::c_void)
    } else {
        None
    };
    let thread = unsafe { CreateRemoteThread(handle.0, None, 0, start, param, 0, None)? };
    let thread = SafeHandle(thread);
    unsafe {
        WaitForSingleObject(thread.0, 5_000);
    }
    Ok(())
}
