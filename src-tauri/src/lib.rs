//! Tauri wiring for HideMyWindows: exposes the `hmw-core` operations to the
//! Svelte UI as commands, manages config + the tray, and runs the window-rule
//! watcher loop.

mod autostart;

use std::sync::Mutex;
use std::time::Duration;

use hmw_core::config::{Config, Theme};
use hmw_core::model::{HideAction, ProcessInfo};
use serde::Serialize;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, State};

/// Shared application state.
struct AppState {
    config: Mutex<Config>,
    config_path: std::path::PathBuf,
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
    Ok(())
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
fn spawn_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let payload = match payload_path(&app) {
            Ok(p) => p,
            Err(_) => return,
        };
        let mut tick: u64 = 0;
        loop {
            let (rules, reapply_ms, poll_ms) = {
                let state = app.state::<AppState>();
                let cfg = state.config.lock().unwrap();
                (
                    cfg.window_rules.clone(),
                    cfg.rule_reapply_interval_ms.max(200),
                    cfg.process_poll_interval_ms.max(200),
                )
            };

            // Frequent pass: persistent rules only.
            let errors = hmw_core::watcher::apply_rules_once(&rules, &payload, true);

            // Slower discovery pass: all rules (every `poll_ms`).
            if tick.is_multiple_of((poll_ms / reapply_ms).max(1)) {
                let more = hmw_core::watcher::apply_rules_once(&rules, &payload, false);
                if !more.is_empty() {
                    let _ = app.emit("rule-errors", more);
                }
            }

            if !errors.is_empty() {
                let _ = app.emit("rule-errors", errors);
            }

            tick = tick.wrapping_add(1);
            std::thread::sleep(Duration::from_millis(reapply_ms));
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
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
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
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
        })
        .build(app)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_path = Config::default_path();
    let config = Config::load(&config_path);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            config: Mutex::new(config),
            config_path,
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_config_dir,
            app_version,
            save_config,
            list_processes,
            list_windows,
            hide_process,
            hide_window,
            quick_launch,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            build_tray(&handle)?;

            // Apply initial self-visibility + autostart from saved config.
            let hide_self = {
                let state = handle.state::<AppState>();
                let cfg = state.config.lock().unwrap();
                cfg.hide_self
            };
            apply_self_visibility(&handle, hide_self);

            #[cfg(windows)]
            spawn_watcher(handle.clone());

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
