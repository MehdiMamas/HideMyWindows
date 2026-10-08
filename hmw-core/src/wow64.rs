//! Dispatch to the bundled x86 helper; never inject a foreign-architecture DLL.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::process::{Child, Stdio};
use std::sync::OnceLock;

use crate::{Error, Result};

static HELPER: OnceLock<(PathBuf, PathBuf)> = OnceLock::new();

pub struct X86Gate(Child);
impl Drop for X86Gate {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl X86Gate {
    pub fn check(&mut self) -> Result<()> {
        if let Some(status) = self.0.try_wait()? {
            Err(Error(format!(
                "32-bit normal-launch gate stopped ({status})"
            )))
        } else {
            Ok(())
        }
    }
}

pub fn start_gate(config: &std::path::Path) -> Result<X86Gate> {
    use std::io::{BufRead, BufReader};
    let (helper, payload) = HELPER
        .get()
        .ok_or_else(|| Error("The bundled 32-bit window gate is unavailable".into()))?;
    let mut child = Command::new(helper)
        .args(["--normal-gate", &std::process::id().to_string()])
        .arg(payload)
        .arg(config)
        .creation_flags(0x08000000)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error("No 32-bit gate status pipe".into()))?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(stdout).read_line(&mut line);
        let _ = tx.send(line);
    });
    let gate = X86Gate(child);
    match rx.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(line) if line.trim() == "HMW_GATE_READY" => Ok(gate),
        _ => Err(Error(
            "The 32-bit normal-launch window gate failed to start".into(),
        )),
    }
}

/// Keep an x86 global hook alive with a message-pumping thread. Retain a real
/// parent handle so PID reuse cannot make an orphaned helper outlive the app.
pub fn run_gate(args: &[String]) -> Result<()> {
    use std::io::Write;
    use windows::Win32::Foundation::WAIT_TIMEOUT;
    use windows::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };
    if args.len() != 3 {
        return Err(Error("Invalid normal-launch gate request".into()));
    }
    let parent = args[0]
        .parse::<u32>()
        .map_err(|_| Error("Invalid controller PID".into()))?;
    if parent == 0 {
        return Err(Error("Invalid controller PID".into()));
    }
    let parent_handle =
        crate::process::SafeHandle(unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, parent)? });
    let path = std::path::Path::new(&args[2]);
    let mut rules = crate::Config::load(path).window_rules;
    let mut gate = crate::normal_gate::WindowGate::start(&args[1], &rules, parent)?;
    println!("HMW_GATE_READY");
    std::io::stdout().flush()?;
    while unsafe { WaitForSingleObject(parent_handle.0, 200) } == WAIT_TIMEOUT {
        // Preserve the last complete snapshot if a config write is in progress.
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(config) = serde_json::from_str::<crate::Config>(&text) {
                rules = config.window_rules;
                gate.update(&rules)?;
            }
        }
    }
    Ok(())
}

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
