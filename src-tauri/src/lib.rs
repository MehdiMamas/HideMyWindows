//! Tauri wiring for HideMyWindows: exposes the `hmw-core` operations to the
//! Svelte UI as commands, manages config + the tray, and runs the window-rule
//! watcher loop.

mod autostart;

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::time::Duration;

use hmw_core::config::{Config, Theme};
use hmw_core::model::{HideAction, ProcessInfo};
use serde::Serialize;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, State};

fn started_minimized<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter()
        .any(|arg| arg.as_ref() == autostart::START_MINIMIZED_ARG)
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Shared application state.
struct AppState {
    config: Mutex<Config>,
    config_path: std::path::PathBuf,
    /// Wakes the rule watcher so a saved config is applied immediately.
    rules_wake: Sender<()>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WindowDto {
    hwnd: i64,
    pid: u32,
    title: String,
    class: String,
}

/// Resolve the bundled payload DLL (matching this build's architecture).
#[cfg(windows)]
fn payload_path(app: &AppHandle) -> Result<String, String> {
    use tauri::path::BaseDirectory;
    let p = app
        .path()
        .resolve("resources/hmw_payload.dll", BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    Ok(p.to_string_lossy().into_owned())
}

#[cfg(not(windows))]
fn payload_path(_app: &AppHandle) -> Result<String, String> {
    Err("HideMyWindows only runs on Windows".into())
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_config(state: State<AppState>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn get_config_dir(state: State<AppState>) -> String {
    state
        .config_path
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The app version, shown on the About page.
#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn save_config(app: AppHandle, state: State<AppState>, config: Config) -> Result<(), String> {
    // Apply side effects that depend on config.
    apply_self_visibility(&app, config.hide_self);
    #[cfg(windows)]
    {
        let _ = autostart::set_autostart(config.start_with_windows);
    }

    let path = state.config_path.clone();
    config.save(&path).map_err(|e| e.to_string())?;
    *state.config.lock().unwrap() = config;
    // Unbounded channel: ignore a send only if the watcher has already exited.
    let _ = state.rules_wake.send(());
    Ok(())
}

/// Restore factory defaults and delete settings left by older versions.
#[tauri::command]
fn reset_settings(app: AppHandle, state: State<AppState>) -> Result<Config, String> {
    let path = state.config_path.clone();
    let dir = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    };
    let config = Config::reset_stored(&dir).map_err(|e| e.to_string())?;

    apply_self_visibility(&app, config.hide_self);
    #[cfg(windows)]
    {
        let _ = autostart::set_autostart(config.start_with_windows);
    }

    *state.config.lock().unwrap() = config.clone();
    let _ = state.rules_wake.send(());
    Ok(config)
}

#[tauri::command]
fn list_processes() -> Result<Vec<ProcessInfo>, String> {
    #[cfg(windows)]
    {
        hmw_core::process::list_processes().map_err(|e| e.0)
    }
    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}

#[tauri::command]
fn list_windows() -> Result<Vec<WindowDto>, String> {
    #[cfg(windows)]
    {
        let windows = hmw_core::window::list_top_windows(true).map_err(|e| e.0)?;
        Ok(windows
            .into_iter()
            .filter(|w| !w.title.is_empty())
            .map(|w| WindowDto {
                hwnd: w.hwnd as i64,
                pid: w.pid,
                title: w.title,
                class: w.class,
            })
            .collect())
    }
    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}

#[tauri::command]
fn hide_process(app: AppHandle, pid: u32, action: HideAction) -> Result<(), String> {
    #[cfg(windows)]
    {
        let payload = payload_path(&app)?;
        hmw_core::hider::apply_to_process(pid, action, &payload).map_err(|e| e.0)
    }
    #[cfg(not(windows))]
    {
        let _ = (app, pid, action);
        Err("Windows only".into())
    }
}

#[tauri::command]
fn hide_window(app: AppHandle, hwnd: i64, action: HideAction) -> Result<(), String> {
    #[cfg(windows)]
    {
        let payload = payload_path(&app)?;
        hmw_core::hider::apply_to_window(hwnd as isize, action, &payload).map_err(|e| e.0)
    }
    #[cfg(not(windows))]
    {
        let _ = (app, hwnd, action);
        Err("Windows only".into())
    }
}

#[tauri::command]
fn quick_launch(app: AppHandle, path: String, arguments: String) -> Result<u32, String> {
    #[cfg(windows)]
    {
        let payload = payload_path(&app)?;
        hmw_core::launch::launch_hidden(&path, &arguments, &payload).map_err(|e| e.0)
    }
    #[cfg(not(windows))]
    {
        let _ = (app, path, arguments);
        Err("Windows only".into())
    }
}

/// Apply this app's own "hide from capture" state to its visible windows.
fn apply_self_visibility(app: &AppHandle, hidden: bool) {
    #[cfg(windows)]
    {
        for (_, window) in app.webview_windows() {
            if let Ok(hwnd) = window.hwnd() {
                let _ = hmw_core::window::set_capture_hidden(hwnd.0 as isize, hidden);
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (app, hidden);
    }
}

// ---------------------------------------------------------------------------
// Watcher loop
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn spawn_watcher(app: AppHandle, rules_rx: Receiver<()>) {
    std::thread::spawn(move || {
        let payload = match payload_path(&app) {
            Ok(p) => p,
            Err(_) => return,
        };
        let mut tick: u64 = 0;
        // True after a successful hide, so turning the option off restores toasts once.
        let mut toasts_hidden = false;
        let mut last_toast_error = String::new();
        let mut session = hmw_core::watcher::RuleSession::new();
        // Startup applies every saved rule without waiting for the timer.
        let mut full = true;
        loop {
            let (rules, reapply_ms, poll_ms, hide_toasts) = {
                let state = app.state::<AppState>();
                let cfg = state.config.lock().unwrap();
                (
                    cfg.window_rules.clone(),
                    cfg.rule_reapply_interval_ms.max(200),
                    cfg.process_poll_interval_ms.max(200),
                    cfg.hide_notification_toasts,
                )
            };

            if hide_toasts {
                let errors = hmw_core::notifications::apply_notification_toasts(true, &payload);
                let message = errors.first().cloned().unwrap_or_default();
                if message != last_toast_error {
                    last_toast_error = message.clone();
                    if !message.is_empty() {
                        let _ = app.emit("rule-errors", vec![message]);
                    }
                }
                if errors.is_empty() {
                    toasts_hidden = true;
                }
            } else if toasts_hidden {
                let errors = hmw_core::notifications::apply_notification_toasts(false, &payload);
                if errors.is_empty() {
                    toasts_hidden = false;
                    last_toast_error.clear();
                } else if errors.first().map(String::as_str) != Some(last_toast_error.as_str()) {
                    last_toast_error = errors[0].clone();
                    let _ = app.emit("rule-errors", errors);
                }
            }

            if full {
                // A save (or the first pass) releases hides that no longer match,
                // then applies the ones the current rules ask for.
                let errors = session.apply(&rules, &payload, false);
                if !errors.is_empty() {
                    let _ = app.emit("rule-errors", errors);
                }
                full = false;
            } else {
                // Frequent pass: persistent rules only. Records hides, does not release.
                let errors = session.apply(&rules, &payload, true);

                // Slower discovery pass: all rules (every `poll_ms`).
                if tick.is_multiple_of((poll_ms / reapply_ms).max(1)) {
                    let more = session.apply(&rules, &payload, false);
                    if !more.is_empty() {
                        let _ = app.emit("rule-errors", more);
                    }
                }

                if !errors.is_empty() {
                    let _ = app.emit("rule-errors", errors);
                }

                tick = tick.wrapping_add(1);
            }

            match rules_rx.recv_timeout(Duration::from_millis(reapply_ms)) {
                Ok(()) => {
                    while rules_rx.try_recv().is_ok() {}
                    full = true;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Tray
// ---------------------------------------------------------------------------

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id("show", "Show HideMyWindows").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
    let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("HideMyWindows")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod wow64_helper {
    include!(concat!(env!("OUT_DIR"), "/wow64_helper.rs"));
}

/// Write the bundled 32-bit cleanup helper under `%TEMP%\HideMyWindows` so an
/// x64 `--release-all` can reach WOW64 processes. Absent when this build did
/// not embed one.
#[cfg(windows)]
fn materialize_wow64_helper() -> Option<std::path::PathBuf> {
    let bytes = wow64_helper::WOW64_HELPER?;
    let dir = std::env::temp_dir().join("HideMyWindows");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("hmw-release-x86.exe");
    if std::fs::write(&path, bytes).is_err() && !path.is_file() {
        return None;
    }
    Some(path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Before the single-instance plugin: uninstall must release hides even
    // while another copy of the app is still running.
    #[cfg(windows)]
    if std::env::args().any(|arg| arg == "--release-all") {
        let helper = materialize_wow64_helper();
        let code = hmw_core::cleanup::run(hmw_core::cleanup::options_from_args(helper));
        std::process::exit(code);
    }

    let config_path = Config::default_path();
    let config = Config::load(&config_path);
    let (rules_tx, rules_rx) = mpsc::channel();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // A second sign-in launch should leave the running copy in the tray.
            if !started_minimized(args) {
                show_main_window(app);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState {
            config: Mutex::new(config),
            config_path,
            rules_wake: rules_tx,
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_config_dir,
            app_version,
            save_config,
            reset_settings,
            list_processes,
            list_windows,
            hide_process,
            hide_window,
            quick_launch,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            build_tray(&handle)?;

            // Apply initial self-visibility + autostart from saved config.
            // Rewriting the Run key picks up the minimized flag for installs
            // that enabled start-with-Windows before it existed.
            let (hide_self, start_with_windows) = {
                let state = handle.state::<AppState>();
                let cfg = state.config.lock().unwrap();
                (cfg.hide_self, cfg.start_with_windows)
            };
            apply_self_visibility(&handle, hide_self);
            #[cfg(windows)]
            {
                let _ = autostart::set_autostart(start_with_windows);
            }
            #[cfg(not(windows))]
            {
                let _ = start_with_windows;
            }

            // The window is created hidden. A normal launch opens it; sign-in
            // (`--minimized` from the Run key) leaves only the tray icon.
            if !started_minimized(std::env::args()) {
                show_main_window(&handle);
            }

            #[cfg(windows)]
            spawn_watcher(handle.clone(), rules_rx);
            #[cfg(not(windows))]
            drop(rules_rx);

            // Close-to-tray / minimize-to-tray behaviour.
            if let Some(window) = app.get_webview_window("main") {
                let h = handle.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let close_to_tray = {
                            let state = h.state::<AppState>();
                            let cfg = state.config.lock().unwrap();
                            cfg.close_to_tray
                        };
                        if close_to_tray {
                            api.prevent_close();
                            if let Some(w) = h.get_webview_window("main") {
                                let _ = w.hide();
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running HideMyWindows");
}

/// Theme is applied in the frontend; this helper keeps the enum referenced for
/// potential native theming and documents the mapping.
#[allow(dead_code)]
fn theme_name(theme: Theme) -> &'static str {
    match theme {
        Theme::System => "system",
        Theme::Light => "light",
        Theme::Dark => "dark",
    }
}

#[cfg(test)]
mod tests {
    use super::started_minimized;

    #[test]
    fn sign_in_launch_stays_in_the_tray() {
        assert!(started_minimized(["hidemywindows.exe", "--minimized"]));
        assert!(!started_minimized(["hidemywindows.exe"]));
        assert!(!started_minimized(["hidemywindows.exe", "--release-all"]));
    }
}
