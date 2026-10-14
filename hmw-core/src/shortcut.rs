//! Start menu shortcuts ("WhatsApp HideMyWindows") that open an app protected.
//!
//! A shortcut runs this app with `--launch-app <AppUserModelID>` (Store apps)
//! or `--launch-exe <path>` (desktop apps), plus `--launch-args <arguments>`.
//! It carries its whole target, so it keeps working if the Quick Launch entry
//! is edited or removed.

use crate::apps::{ico_from_pngs, icon_png, shell_item, ComGuard};
use crate::package_launch::quote_arg;
use crate::{Error, Result};
use std::path::{Path, PathBuf};
use windows::core::{Interface, HSTRING};
use windows::Win32::System::Com::{
    CoCreateInstance, CoTaskMemFree, IPersistFile, CLSCTX_INPROC_SERVER,
};
use windows::Win32::UI::Shell::{
    FOLDERID_Programs, IShellLinkW, SHGetKnownFolderPath, ShellLink, KF_FLAG_DEFAULT,
};

pub const APP_ARG: &str = "--launch-app";
pub const EXE_ARG: &str = "--launch-exe";
pub const ARGS_ARG: &str = "--launch-args";

/// What a launcher shortcut asks for.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LaunchRequest {
    pub app_id: String,
    pub path: String,
    pub arguments: String,
}

impl LaunchRequest {
    pub fn from_args<S: AsRef<str>>(args: &[S]) -> Option<Self> {
        let value = |key: &str| {
            args.iter()
                .position(|a| a.as_ref() == key)
                .and_then(|i| args.get(i + 1))
                .map(|v| v.as_ref().to_string())
        };
        let request = LaunchRequest {
            app_id: value(APP_ARG).unwrap_or_default(),
            path: value(EXE_ARG).unwrap_or_default(),
            arguments: value(ARGS_ARG).unwrap_or_default(),
        };
        (request.app_id.is_empty() != request.path.is_empty()).then_some(request)
    }

    fn command_line(&self) -> String {
        let mut line = if self.app_id.is_empty() {
            format!("{EXE_ARG} {}", quote_arg(&self.path))
        } else {
            format!("{APP_ARG} {}", quote_arg(&self.app_id))
        };
        if !self.arguments.is_empty() {
            line.push_str(&format!(" {ARGS_ARG} {}", quote_arg(&self.arguments)));
        }
        line
    }
}

/// "WhatsApp HideMyWindows.lnk", without characters Windows refuses in names.
pub fn shortcut_file_name(app_name: &str) -> String {
    let clean: String = app_name
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean = clean.trim_end_matches('.');
    let base = if clean.is_empty() { "App" } else { clean };
    format!("{base} HideMyWindows.lnk")
}

fn programs_dir() -> Result<PathBuf> {
    unsafe {
        let raw = SHGetKnownFolderPath(&FOLDERID_Programs, KF_FLAG_DEFAULT, None)?;
        let path = raw.to_string().map_err(|e| Error(e.to_string()));
        CoTaskMemFree(Some(raw.0 as *const _));
        Ok(PathBuf::from(path?))
    }
}

/// Create (or replace) the Start menu shortcut for `request`. Its icon is the
/// app's own, saved under `icon_dir`. Returns the shortcut's path.
pub fn create(
    app_name: &str,
    request: &LaunchRequest,
    self_exe: &str,
    icon_dir: &Path,
) -> Result<String> {
    let _com = ComGuard::new();
    let file_name = shortcut_file_name(app_name);
    let target = shell_item(&request.app_id, &request.path)?;
    let images: Vec<(u32, Vec<u8>)> = [16u32, 32, 48, 256]
        .into_iter()
        .filter_map(|size| icon_png(&target, size as i32).map(|png| (size, png)))
        .collect();
    let icon_path = icon_dir.join(file_name.replace(".lnk", ".ico"));
    let has_icon = !images.is_empty() && {
        std::fs::create_dir_all(icon_dir)?;
        std::fs::write(&icon_path, ico_from_pngs(&images))?;
        true
    };

    let path = programs_dir()?.join(&file_name);
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(&HSTRING::from(self_exe))?;
        link.SetArguments(&HSTRING::from(request.command_line()))?;
        link.SetDescription(&HSTRING::from(format!(
            "Open {app_name} hidden from screen capture"
        )))?;
        if let Some(dir) = Path::new(self_exe).parent() {
            link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()))?;
        }
        if has_icon {
            link.SetIconLocation(&HSTRING::from(icon_path.as_os_str()), 0)?;
        } else if !request.path.is_empty() {
            link.SetIconLocation(&HSTRING::from(request.path.as_str()), 0)?;
        }
        link.cast::<IPersistFile>()?
            .Save(&HSTRING::from(path.as_os_str()), true)?;
    }
    Ok(path.to_string_lossy().into_owned())
}

/// Delete a shortcut made by [`create`] and its saved icon.
pub fn remove(shortcut_path: &str, icon_dir: &Path) {
    let path = Path::new(shortcut_path);
    // Only files this module names, so a bad config value cannot delete others.
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return;
    };
    if !name.ends_with(" HideMyWindows.lnk") {
        return;
    }
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(icon_dir.join(name.replace(".lnk", ".ico")));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(line: &str) -> Vec<String> {
        // Enough of CommandLineToArgvW for these cases.
        let mut args = vec!["exe".to_string()];
        let (mut current, mut quoted, mut chars) = (String::new(), false, line.chars().peekable());
        let mut started = false;
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    let mut n = 1;
                    while chars.peek() == Some(&'\\') {
                        chars.next();
                        n += 1;
                    }
                    if chars.peek() == Some(&'"') {
                        current.extend(std::iter::repeat_n('\\', n / 2));
                        if n % 2 == 1 {
                            chars.next();
                            current.push('"');
                        }
                    } else {
                        current.extend(std::iter::repeat_n('\\', n));
                    }
                    started = true;
                }
                '"' => {
                    quoted = !quoted;
                    started = true;
                }
                ' ' if !quoted => {
                    if started {
                        args.push(std::mem::take(&mut current));
                        started = false;
                    }
                }
                _ => {
                    current.push(c);
                    started = true;
                }
            }
        }
        if started {
            args.push(current);
        }
        args
    }

    #[test]
    fn launch_requests_survive_the_shortcut_command_line() {
        for request in [
            LaunchRequest {
                app_id: "5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App".into(),
                ..Default::default()
            },
            LaunchRequest {
                path: "C:\\Program Files\\My App\\app.exe".into(),
                arguments: "--profile \"Work stuff\" C:\\dir\\".into(),
                ..Default::default()
            },
        ] {
            assert_eq!(
                LaunchRequest::from_args(&split(&request.command_line())),
                Some(request)
            );
        }
    }

    #[test]
    fn needs_exactly_one_target() {
        assert_eq!(LaunchRequest::from_args(&["exe", "--minimized"]), None);
        assert_eq!(
            LaunchRequest::from_args(&["exe", APP_ARG, "a!b", EXE_ARG, "c.exe"]),
            None
        );
    }

    #[test]
    fn names_shortcuts_after_the_app() {
        assert_eq!(shortcut_file_name("WhatsApp"), "WhatsApp HideMyWindows.lnk");
        assert_eq!(shortcut_file_name("A/B: C?"), "A B C HideMyWindows.lnk");
        assert_eq!(shortcut_file_name(" ... "), "App HideMyWindows.lnk");
    }
}
