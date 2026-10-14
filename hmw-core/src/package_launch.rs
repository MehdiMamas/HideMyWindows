//! Protected launch of Store (packaged) apps.
//!
//! A Store app cannot be started with `CreateProcessW(CREATE_SUSPENDED)` like
//! Quick Launch does for desktop apps. Instead, Windows' package debugging
//! setting (`IPackageDebugSettings::EnableDebugging`) is turned on for the
//! package just for this launch. Windows then creates the app suspended and
//! starts our helper (`--protect-launch <token>`) with ` -p <pid> -tid <tid>`
//! appended. Windows refuses commands of 260 characters or more, so the
//! payload path travels in a file named after the token instead. The helper protects the process and resumes it, or terminates it
//! when protection fails. The setting is removed as soon as activation returns.

use crate::apps::ComGuard;
use crate::hider::apply_to_process;
use crate::model::HideAction;
use crate::process::{wide_to_string, SafeHandle};
use crate::{Error, Result};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use windows::core::{HSTRING, PCWSTR, PWSTR};
use windows::Win32::Storage::Packaging::Appx::{
    GetPackagePathByFullName, GetPackagesByPackageFamily,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::Threading::{
    OpenProcess, OpenThread, ResumeThread, TerminateProcess, PROCESS_TERMINATE,
    THREAD_SUSPEND_RESUME,
};
use windows::Win32::UI::Shell::{
    ApplicationActivationManager, IApplicationActivationManager, IPackageDebugSettings,
    PackageDebugSettings, AO_NONE,
};

/// Command-line switch of the helper Windows starts for a protected launch.
pub const HELPER_ARG: &str = "--protect-launch";

/// Full names of the installed packages in `family`.
pub fn packages_for_family(family: &str) -> Vec<String> {
    let family = HSTRING::from(family);
    let (mut count, mut length) = (0u32, 0u32);
    unsafe {
        let _ = GetPackagesByPackageFamily(&family, &mut count, None, &mut length, PWSTR::null());
        if count == 0 || length == 0 {
            return Vec::new();
        }
        let mut names = vec![PWSTR::null(); count as usize];
        let mut buffer = vec![0u16; length as usize];
        if GetPackagesByPackageFamily(
            &family,
            &mut count,
            Some(names.as_mut_ptr()),
            &mut length,
            PWSTR(buffer.as_mut_ptr()),
        )
        .is_err()
        {
            return Vec::new();
        }
        names
            .iter()
            .take(count as usize)
            .filter_map(|name| name.to_string().ok())
            .collect()
    }
}

fn package_path(full_name: &str) -> Option<String> {
    let name = HSTRING::from(full_name);
    let mut length = 0u32;
    unsafe {
        let _ = GetPackagePathByFullName(&name, &mut length, PWSTR::null());
        let mut buffer = vec![0u16; length.max(1) as usize];
        GetPackagePathByFullName(&name, &mut length, PWSTR(buffer.as_mut_ptr()))
            .ok()
            .ok()?;
        Some(wide_to_string(&buffer))
    }
}

/// This session's processes started from the package's own folder. Shared
/// runtimes the app uses, like WebView2, live elsewhere and are left out.
fn running_package_processes(full_names: &[String]) -> Vec<u32> {
    let roots: Vec<String> = full_names
        .iter()
        .filter_map(|name| package_path(name))
        .map(|root| format!("{}\\", root.trim_end_matches('\\').to_lowercase()))
        .collect();
    crate::process::list_processes()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| {
            let path = p.path.to_lowercase();
            roots.iter().any(|root| path.starts_with(root))
                && crate::process::in_current_session(p.pid)
        })
        .map(|p| p.pid)
        .collect()
}

/// Package debugging for one launch, turned off again on drop.
struct DebugGuard {
    settings: IPackageDebugSettings,
    packages: Vec<String>,
}

impl DebugGuard {
    fn enable(packages: &[String], command: &str) -> Result<Self> {
        let settings: IPackageDebugSettings =
            unsafe { CoCreateInstance(&PackageDebugSettings, None, CLSCTX_ALL)? };
        let guard = DebugGuard {
            settings,
            packages: Vec::new(),
        };
        let mut guard = guard;
        for package in packages {
            unsafe {
                guard
                    .settings
                    .EnableDebugging(
                        &HSTRING::from(package.as_str()),
                        &HSTRING::from(command),
                        PCWSTR::null(),
                    )
                    .map_err(|e| {
                        Error(format!(
                            "Could not prepare the protected launch of {package}: {}",
                            e.message()
                        ))
                    })?;
            }
            guard.packages.push(package.clone());
        }
        Ok(guard)
    }
}

impl Drop for DebugGuard {
    fn drop(&mut self) {
        for package in &self.packages {
            unsafe {
                let _ = self
                    .settings
                    .DisableDebugging(&HSTRING::from(package.as_str()));
            }
        }
    }
}

/// Turn off package debugging that a crashed launch may have left on.
pub fn clear_launch_debugging(app_id: &str) {
    let Some(family) = crate::apps::package_family(app_id) else {
        return;
    };
    let _com = ComGuard::new();
    let Ok(settings) = (unsafe {
        CoCreateInstance::<_, IPackageDebugSettings>(&PackageDebugSettings, None, CLSCTX_ALL)
    }) else {
        return;
    };
    for package in packages_for_family(family) {
        unsafe {
            let _ = settings.DisableDebugging(&HSTRING::from(package.as_str()));
        }
    }
}

fn result_dir() -> PathBuf {
    std::env::temp_dir().join("HideMyWindows").join("launches")
}

/// Quote one argument the way `CommandLineToArgvW` reads it back.
pub fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_string();
    }
    let mut out = String::from('"');
    let mut slashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => slashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
                out.push('"');
                slashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n('\\', slashes));
                out.push(c);
                slashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', slashes * 2));
    out.push('"');
    out
}

/// Start the Store app `app_id` protected from its first frame. When the app
/// already runs (for example in the background), protect it, then show it.
/// `helper_exe` is this app's executable. Returns the app's process id.
pub fn launch_packaged(
    app_id: &str,
    arguments: &str,
    payload_path: &str,
    helper_exe: &str,
) -> Result<u32> {
    let family = crate::apps::package_family(app_id)
        .ok_or_else(|| Error(format!("\"{app_id}\" is not a Store app")))?;
    let packages = packages_for_family(family);
    if packages.is_empty() {
        return Err(Error(format!("{family} is not installed")));
    }
    let _com = ComGuard::new();
    let activator: IApplicationActivationManager =
        unsafe { CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_ALL) }.map_err(
            |e| {
                Error(format!(
                    "Windows app activation is unavailable: {}",
                    e.message()
                ))
            },
        )?;
    let activate = || -> Result<u32> {
        Ok(unsafe {
            activator.ActivateApplication(
                &HSTRING::from(app_id),
                &HSTRING::from(arguments),
                AO_NONE,
            )?
        })
    };

    // A running app (for example in the background) is protected before it
    // shows a window. Activation may still start another process, so the
    // launch helper stays registered either way.
    let running = running_package_processes(&packages);
    for &pid in &running {
        apply_to_process(pid, HideAction::HideProcessWindows, payload_path).map_err(|e| {
            Error(format!(
                "Could not protect the running app, so it was not opened: {e}"
            ))
        })?;
    }

    let token = format!(
        "{}-{}",
        std::process::id(),
        crate::launch_protection::now_ms()
    );
    let command = format!("{} {HELPER_ARG} {token}", quote_arg(helper_exe));
    if command.encode_utf16().count() >= 260 {
        return Err(Error("HideMyWindows is installed in a folder whose path is too long for protected Store-app launches".into()));
    }
    let dir = result_dir();
    std::fs::create_dir_all(&dir)?;
    let request_file = dir.join(format!("{token}.payload"));
    let result_file = dir.join(format!("{token}.txt"));
    std::fs::write(&request_file, payload_path)?;
    let activated = DebugGuard::enable(&packages, &command).and_then(|_debugging| activate());
    if let Ok(pid) = activated {
        if running.contains(&pid) {
            let _ = std::fs::remove_file(&request_file);
            return Ok(pid);
        }
    }

    // Activation returns once the helper resumed or terminated the app.
    let deadline = Instant::now() + Duration::from_secs(20);
    let outcome = loop {
        if let Ok(text) = std::fs::read_to_string(&result_file) {
            if text.ends_with('\n') {
                let _ = std::fs::remove_file(&result_file);
                break Some(text);
            }
        }
        if Instant::now() > deadline || activated.is_err() && !result_file.exists() {
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let _ = std::fs::remove_file(&request_file);
    match (outcome, activated) {
        (Some(text), activated) => match text.trim_end().split_once(' ') {
            Some(("ok", pid)) => Ok(activated.unwrap_or_else(|_| pid.parse().unwrap_or(0))),
            Some((_, message)) => Err(Error(format!(
                "Protected launch stopped before the app could start: {message}"
            ))),
            None => Err(Error(
                "Protected launch returned an unreadable result".into(),
            )),
        },
        // No helper ran: Windows reused a process the check above missed.
        (None, Ok(pid)) => {
            apply_to_process(pid, HideAction::HideProcessWindows, payload_path)?;
            Ok(pid)
        }
        (None, Err(error)) => Err(Error(format!("Windows could not start the app: {error}"))),
    }
}

/// Parsed `--protect-launch <token> -p <pid> -tid <tid>`.
#[derive(Debug, PartialEq)]
pub struct HelperArgs {
    pub token: String,
    pub pid: u32,
    pub tid: u32,
}

pub fn parse_helper_args(args: &[String]) -> Option<HelperArgs> {
    let value = |key: &str| {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
    };
    let token = value(HELPER_ARG)?;
    // Only a plain token: it names the result file.
    if token.is_empty() || !token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    Some(HelperArgs {
        token: token.clone(),
        pid: value("-p")?.parse().ok()?,
        tid: value("-tid")?.parse().ok()?,
    })
}

/// Run as the helper Windows starts for a protected Store-app launch. The app
/// stays suspended until this returns; it is resumed only once protected.
pub fn run_helper(args: &HelperArgs) -> i32 {
    let result = protect_and_resume(args);
    let line = match &result {
        Ok(()) => format!("ok {}\n", args.pid),
        Err(error) => format!("error {}\n", error.0.replace('\n', " ")),
    };
    let dir = result_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(format!("{}.txt", args.token)), line);
    i32::from(result.is_err())
}

fn protect_and_resume(args: &HelperArgs) -> Result<()> {
    let thread = SafeHandle(unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, args.tid)? });
    let protected = std::fs::read_to_string(result_dir().join(format!("{}.payload", args.token)))
        .map_err(|_| Error("this launch was not started by HideMyWindows".into()))
        .and_then(|payload| apply_to_process(args.pid, HideAction::HideProcessWindows, &payload));
    if protected.is_ok() && unsafe { ResumeThread(thread.0) } != u32::MAX {
        return Ok(());
    }
    if let Ok(process) = unsafe { OpenProcess(PROCESS_TERMINATE, false, args.pid) } {
        let process = SafeHandle(process);
        unsafe {
            let _ = TerminateProcess(process.0, 1);
        }
    }
    protected.and(Err(Error("could not resume the app".into())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_the_command_windows_runs() {
        let parsed = parse_helper_args(&args(&[
            "C:\\Apps\\HideMyWindows.exe",
            "--protect-launch",
            "12-345",
            "-p",
            "28356",
            "-tid",
            "56756",
        ]));
        assert_eq!(
            parsed,
            Some(HelperArgs {
                token: "12-345".into(),
                pid: 28356,
                tid: 56756,
            })
        );
    }

    #[test]
    fn rejects_tokens_that_are_not_file_names() {
        let parsed = parse_helper_args(&args(&[
            "x",
            "--protect-launch",
            "..\\evil",
            "-p",
            "1",
            "-tid",
            "2",
        ]));
        assert_eq!(parsed, None);
        assert_eq!(
            parse_helper_args(&args(&["x", "--protect-launch", "1"])),
            None
        );
    }

    #[test]
    fn quotes_arguments_for_windows() {
        assert_eq!(quote_arg("plain"), "plain");
        assert_eq!(quote_arg(""), "\"\"");
        assert_eq!(
            quote_arg("C:\\Program Files\\App"),
            "\"C:\\Program Files\\App\""
        );
        assert_eq!(quote_arg("a \"b\""), "\"a \\\"b\\\"\"");
        assert_eq!(
            quote_arg("C:\\dir with space\\"),
            "\"C:\\dir with space\\\\\""
        );
    }
}
