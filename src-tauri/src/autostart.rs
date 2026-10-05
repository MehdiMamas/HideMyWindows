//! Start-with-Windows support via the per-user Run registry key.
#![cfg(windows)]

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_SET_VALUE, REG_SZ,
};

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: PCWSTR = w!("HideMyWindows");

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Enable or disable launching HideMyWindows at sign-in.
pub fn set_autostart(enable: bool) -> Result<(), String> {
    unsafe {
        let mut key = HKEY::default();
        let status = RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, 0, KEY_SET_VALUE, &mut key);
        if status != ERROR_SUCCESS {
            return Err(format!("Could not open Run key: {}", status.0));
        }
        let result = if enable {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let command = format!("\"{}\"", exe.to_string_lossy());
            let wide = to_wide(&command);
            let bytes = std::slice::from_raw_parts(
                wide.as_ptr() as *const u8,
                wide.len() * std::mem::size_of::<u16>(),
            );
            let st = RegSetValueExW(key, VALUE_NAME, 0, REG_SZ, Some(bytes));
            if st != ERROR_SUCCESS {
                Err(format!("Could not set Run value: {}", st.0))
            } else {
                Ok(())
            }
        } else {
            let st = RegDeleteValueW(key, VALUE_NAME);
            // Deleting a missing value is fine.
            if st != ERROR_SUCCESS && st.0 != 2 {
                Err(format!("Could not clear Run value: {}", st.0))
            } else {
                Ok(())
            }
        };
        let _ = RegCloseKey(key);
        result
    }
}
