//! Dispatch to the bundled x86 helper; never inject a foreign-architecture DLL.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use crate::{Error, Result};

static HELPER: OnceLock<(PathBuf, PathBuf)> = OnceLock::new();

pub fn configure(helper: PathBuf, payload: PathBuf) {
    let _ = HELPER.set((helper, payload));
}

pub fn supports_export(export: &str) -> bool {
    matches!(
        export,
        "HmwHideAll"
            | "HmwUnhideAll"
            | "HmwHideWindow"
            | "HmwUnhideWindow"
            | "HmwHideTray"
            | "HmwUnhideTray"
            | "HmwHideToasts"
            | "HmwUnhideToasts"
            | "HmwPrepareProtection"
            | "HmwCheckProtection"
    )
}

pub(crate) fn call(pid: u32, export: &str, hwnd: isize) -> Result<()> {
    let (helper, payload) = HELPER.get().ok_or_else(|| {
        Error("The bundled 32-bit helper is unavailable. Reinstall the latest x64 build.".into())
    })?;
    if !supports_export(export) {
        return Err(Error(format!("Unknown payload export {export}")));
    }
    let output = Command::new(helper)
        .arg("--call-export")
        .arg(pid.to_string())
        .arg(payload)
        .arg(export)
        // HWNDs are 32-bit values on Windows, including when passed by x64.
        .arg((hwnd as u32).to_string())
        .creation_flags(0x08000000) // CREATE_NO_WINDOW: no console flashes.
        .output()
        .map_err(|e| Error(format!("Could not start the 32-bit helper: {e}")))?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr);
        Err(Error(format!("32-bit target: {}", detail.trim())))
    }
}

/// The helper accepts only the payload's known actions and valid numeric IDs.
pub fn run_call(args: &[String]) -> Result<()> {
    if args.len() != 4 || !supports_export(&args[2]) {
        return Err(Error("Invalid 32-bit helper request".into()));
    }
    let pid = args[0]
        .parse::<u32>()
        .map_err(|_| Error("Invalid PID".into()))?;
    let hwnd = args[3]
        .parse::<u32>()
        .map_err(|_| Error("Invalid HWND".into()))?;
    if pid == 0 {
        return Err(Error("Invalid PID".into()));
    }
    crate::inject::call_export(pid, &args[1], &args[2], hwnd as isize)
}
