//! Own the synchronous desktop window gate and keep its installing thread pumping.
use crate::{Error, Result, WindowRule};
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;
use std::time::Duration;
use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::WindowsAndMessaging::*;

enum Request {
    Update(Vec<u8>, Sender<Result<()>>),
    Stop,
}
pub struct WindowGate {
    tx: Sender<Request>,
    thread: Option<JoinHandle<()>>,
    policy: Vec<u8>,
}

fn policy(rules: &[WindowRule]) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(rules).map_err(|e| Error(e.to_string()))?;
    if bytes.len() > 65536 {
        return Err(Error(
            "Normal-launch rules exceed the 64 KiB policy limit".into(),
        ));
    }
    Ok(bytes)
}

impl WindowGate {
    pub fn start(payload: &str, rules: &[WindowRule], excluded_pid: u32) -> Result<Self> {
        let policy = policy(rules)?;
        let initial = policy.clone();
        let path: Vec<u16> = payload.encode_utf16().chain(Some(0)).collect();
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready) = mpsc::channel();
        let thread = std::thread::spawn(move || unsafe {
            // Keep the DLL loaded until process exit: its callbacks can still
            // be on a stack when UnhookWindowsHookEx returns.
            let module = match LoadLibraryW(PCWSTR(path.as_ptr())) {
                Ok(module) => module,
                Err(error) => {
                    let _ = ready_tx.send(Err(Error(error.to_string())));
                    return;
                }
            };
            let start = GetProcAddress(module, PCSTR(c"HmwGateStart".as_ptr().cast()));
            let update = GetProcAddress(module, PCSTR(c"HmwGateUpdate".as_ptr().cast()));
            let stop = GetProcAddress(module, PCSTR(c"HmwGateStop".as_ptr().cast()));
            let (Some(start), Some(update), Some(stop)) = (start, update, stop) else {
                let _ = ready_tx.send(Err(Error(
                    "The bundled payload lacks normal-launch gating. Reinstall the app.".into(),
                )));
                return;
            };
            let start: unsafe extern "system" fn(*const u8, u32, u32) -> u32 =
                std::mem::transmute(start);
            let update: unsafe extern "system" fn(*const u8, u32) -> u32 =
                std::mem::transmute(update);
            let stop: unsafe extern "system" fn() = std::mem::transmute(stop);
            let code = start(initial.as_ptr(), initial.len() as u32, excluded_pid);
            if code != 0 {
                let _ = ready_tx.send(Err(Error(format!(
                    "Window gate installation failed (Windows error {code})"
                ))));
                return;
            }
            if ready_tx.send(Ok(())).is_err() {
                stop();
                return;
            }
            loop {
                let mut message = MSG::default();
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                match rx.recv_timeout(Duration::from_millis(20)) {
                    Ok(Request::Update(bytes, reply)) => {
                        let code = update(bytes.as_ptr(), bytes.len() as u32);
                        let _ = reply.send(if code == 0 {
                            Ok(())
                        } else {
                            Err(Error(format!(
                                "Window gate update failed (Windows error {code})"
                            )))
                        });
                    }
                    Ok(Request::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                        stop();
                        return;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        });
        ready
            .recv()
            .map_err(|_| Error("Window gate stopped during startup".into()))??;
        Ok(Self {
            tx,
            thread: Some(thread),
            policy,
        })
    }

    pub fn update(&mut self, rules: &[WindowRule]) -> Result<()> {
        let bytes = policy(rules)?;
        if bytes == self.policy {
            return Ok(());
        }
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(Request::Update(bytes.clone(), tx))
            .map_err(|_| Error("Window gate stopped".into()))?;
        rx.recv()
            .map_err(|_| Error("Window gate stopped during rule update".into()))??;
        self.policy = bytes;
        Ok(())
    }
}
impl Drop for WindowGate {
    fn drop(&mut self) {
        let _ = self.tx.send(Request::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

unsafe extern "system" fn failures(hwnd: HWND, context: LPARAM) -> BOOL {
    let code = GetPropW(
        hwnd,
        PCWSTR(windows::core::w!("HideMyWindows.ProtectionFailure").as_ptr()),
    );
    if !code.is_invalid() {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let result = &mut *(context.0 as *mut Vec<String>);
        result.push(format!("Normal-launch window {} (process {pid}): protection blocked or interception unsupported (Windows error {})", hwnd.0 as isize, code.0 as usize));
    }
    BOOL(1)
}
/// Include invisible blocked windows, which a visible-window list cannot report.
pub fn blocked_windows() -> Vec<String> {
    let mut result = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(failures),
            LPARAM(&mut result as *mut Vec<String> as isize),
        );
    }
    result
}
