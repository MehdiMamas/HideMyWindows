//! Quick launch: start an executable suspended, hide its windows, then resume
//! it, so the application is hidden from capture before it ever draws.

use crate::hider::apply_to_process;
use crate::model::HideAction;
use crate::{Error, Result};
use windows::core::PWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    CreateProcessW, ResumeThread, CREATE_SUSPENDED, PROCESS_INFORMATION, STARTUPINFOW,
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

    // Hide before the app's main thread gets to run and draw.
    let hide_result = apply_to_process(pid, HideAction::HideProcessWindows, payload_path);

    unsafe {
        ResumeThread(info.hThread);
        let _ = CloseHandle(info.hThread);
        let _ = CloseHandle(info.hProcess);
    }

    hide_result?;
    Ok(pid)
}
