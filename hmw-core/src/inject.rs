//! Loads the HideMyWindows helper payload into a target process and calls its
//! exported functions, so windows owned by another process can be hidden from
//! screen capture.
//!
//! The technique is the classic `CreateRemoteThread` + `LoadLibraryW` loader:
//! we write the payload's path into the target, start a thread at
//! `LoadLibraryW`, then start further threads at the payload's exported
//! functions (resolved from the remote module export table). The HWND a
//! function needs is passed directly as the thread parameter, so no extra
//! remote allocation is required for calls.
//!
//! The x64 controller delegates x86 targets to a bundled x86 helper and DLL.
//! Exports are resolved from the DLL actually loaded in the target, including
//! when an earlier HideMyWindows version left its payload running.

use crate::process::{is_process_64bit, wide_to_string, SafeHandle};
use crate::{Error, Result};
use windows::core::PCWSTR;
use windows::Win32::Foundation::WAIT_OBJECT_0;
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W, TH32CS_SNAPMODULE,
    TH32CS_SNAPMODULE32,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateRemoteThread, GetExitCodeThread, OpenProcess, WaitForSingleObject,
    LPTHREAD_START_ROUTINE, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION,
    PROCESS_VM_READ, PROCESS_VM_WRITE,
};

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
    let mut safe_to_free = true;
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
        if unsafe { WaitForSingleObject(thread.0, 10_000) } != WAIT_OBJECT_0 {
            // The loader may still read this path. Keep it valid until the
            // target exits; a failed Quick Launch terminates that target.
            safe_to_free = false;
            return Err(Error(
                "The target did not load capture protection within 10 seconds".into(),
            ));
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

    if safe_to_free {
        unsafe {
            let _ = VirtualFreeEx(handle.0, remote_mem, 0, MEM_RELEASE);
        }
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

    // Delegate WOW64 targets; each process still loads a matching DLL.
    let target_64 = is_process_64bit(handle.0)?;
    if !target_64 && cfg!(target_arch = "x86_64") {
        drop(handle);
        return crate::wow64::call(pid, export, hwnd);
    }
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
    let func_addr =
        crate::cleanup::export_remote(handle.0, base as usize, export)? as *const core::ffi::c_void;

    let start: LPTHREAD_START_ROUTINE =
        Some(unsafe { std::mem::transmute::<*const core::ffi::c_void, ThreadStart>(func_addr) });
    let param = if hwnd != 0 {
        Some(hwnd as *const core::ffi::c_void)
    } else {
        None
    };
    let thread = unsafe { CreateRemoteThread(handle.0, None, 0, start, param, 0, None)? };
    let thread = SafeHandle(thread);
    if unsafe { WaitForSingleObject(thread.0, 5_000) } != WAIT_OBJECT_0 {
        return Err(Error(
            "The target did not finish the hide/unhide action".into(),
        ));
    }
    let mut exit = 0;
    unsafe {
        GetExitCodeThread(thread.0, &mut exit)?;
    }
    if exit != 0 {
        return Err(Error(format!("The target action failed (code {exit:#x})")));
    }
    Ok(())
}
