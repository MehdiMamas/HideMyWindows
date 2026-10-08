//! Quick launch: start an executable suspended, hide its windows, then resume
//! it, so the application is hidden from capture before it ever draws.

use crate::hider::apply_to_process;
use crate::model::HideAction;
use crate::process::SafeHandle;
use crate::{Error, Result};
use windows::core::PWSTR;
use windows::Win32::System::Threading::{
    CreateProcessW, ResumeThread, TerminateProcess, CREATE_SUSPENDED, PROCESS_INFORMATION,
    STARTUPINFOW,
};

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Launch `path` with `arguments`, hide all of its windows, then resume it.
/// Returns the new process id.
pub fn launch_hidden(path: &str, arguments: &str, payload_path: &str) -> Result<u32> {
    if path.trim().is_empty() {
        return Err(Error("No executable path specified".into()));
    }

    // Command line: "path" args  (quote the path to tolerate spaces).
    let command_line = if arguments.trim().is_empty() {
        format!("\"{path}\"")
    } else {
        format!("\"{path}\" {arguments}")
    };
    let mut command_line_wide = to_wide(&command_line);

    let working_dir = std::path::Path::new(path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned());
    let working_dir_wide = working_dir.as_ref().map(|d| to_wide(d));

    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut info = PROCESS_INFORMATION::default();

    unsafe {
        CreateProcessW(
            None,
            PWSTR(command_line_wide.as_mut_ptr()),
            None,
            None,
            false,
            CREATE_SUSPENDED,
            None,
            working_dir_wide
                .as_ref()
                .map(|w| windows::core::PCWSTR(w.as_ptr()))
                .unwrap_or(windows::core::PCWSTR::null()),
            &startup,
            &mut info,
        )?;
    }

    let pid = info.dwProcessId;
    let process = SafeHandle(info.hProcess);
    let thread = SafeHandle(info.hThread);

    // Hide before the app's main thread gets to run and draw.
    if let Err(error) = apply_to_process(pid, HideAction::HideProcessWindows, payload_path) {
        unsafe {
            let _ = TerminateProcess(process.0, 1);
        }
        return Err(Error(format!(
            "Protected launch stopped before the app could start: {error}"
        )));
    }
    if unsafe { ResumeThread(thread.0) } == u32::MAX {
        unsafe {
            let _ = TerminateProcess(process.0, 1);
        }
        return Err(Error("Protected launch could not resume the app".into()));
    }
    Ok(pid)
}
