use crate::model::WindowRule;
use std::sync::Mutex;

static POLICY: Mutex<(Vec<u8>, Vec<WindowRule>)> = Mutex::new((Vec::new(), Vec::new()));

unsafe fn wide(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0;
    while len < 32768 && *ptr.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}

/// The native hook supplies a consistent, bounded snapshot from shared memory.
/// A malformed policy blocks showing rather than silently allowing a match.
#[no_mangle]
pub unsafe extern "C" fn HmwGateDecision(
    bytes: *const u8,
    len: u32,
    pid: u32,
    process: *const u16,
    title: *const u16,
    class: *const u16,
) -> i32 {
    if bytes.is_null() || len > 65536 {
        return -1;
    }
    let data = std::slice::from_raw_parts(bytes, len as usize);
    let mut cached = POLICY.lock().unwrap_or_else(|p| p.into_inner());
    if cached.0 != data {
        let Ok(rules) = serde_json::from_slice::<Vec<WindowRule>>(data) else {
            return -1;
        };
        *cached = (data.to_vec(), rules);
    }
    i32::from(crate::gate_policy::excludes_window(
        &cached.1,
        pid,
        &wide(process),
        &wide(title),
        &wide(class),
    ))
}
