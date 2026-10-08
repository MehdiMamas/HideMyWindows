//! Wake rule discovery on top-level window creation/showing, without waiting
//! for the discovery timer. This reduces latency; it is not a pre-show gate.
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::OnceLock;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::*;

static WAKE: OnceLock<Sender<()>> = OnceLock::new();
static LAST_WAKE: AtomicU64 = AtomicU64::new(0);

unsafe extern "system" fn event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    if object != 0 || child != 0 || hwnd.is_invalid() || GetAncestor(hwnd, GA_ROOT) != hwnd {
        return;
    }
    let now = GetTickCount64();
    let previous = LAST_WAKE.load(Ordering::Relaxed);
    if now.saturating_sub(previous) < 25
        || LAST_WAKE
            .compare_exchange(previous, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
    {
        return;
    }
    if let Some(wake) = WAKE.get() {
        let _ = wake.send(());
    }
}

pub fn start(wake: Sender<()>) {
    if WAKE.set(wake).is_err() {
        return;
    }
    std::thread::spawn(|| unsafe {
        let hook = SetWinEventHook(
            EVENT_OBJECT_CREATE,
            EVENT_OBJECT_SHOW,
            None,
            Some(event),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if hook.is_invalid() {
            return;
        }
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = UnhookWinEvent(hook);
    });
}
