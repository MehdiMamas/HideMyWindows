//! Native right-click protection controls. Menu changes run on the UI thread.
use crate::{show_main_window, AppState};
use hmw_core::launch_protection::{now_ms, TrayProtectionAction, TrayProtectionState};
use serde_json::Value;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder, Submenu, SubmenuBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

struct TrayMenu {
    show: MenuItem<Wry>,
    status: MenuItem<Wry>,
    toggle: MenuItem<Wry>,
    pause: Submenu<Wry>,
    durations: Vec<(u64, MenuItem<Wry>)>,
    resume: MenuItem<Wry>,
    quit: MenuItem<Wry>,
    previous: Mutex<Option<(TrayProtectionState, String)>>,
}

fn translations(language: &str) -> &'static Value {
    static LANGUAGES: OnceLock<[Value; 5]> = OnceLock::new();
    let all = LANGUAGES.get_or_init(|| {
        [
            include_str!("../../src/lib/locales/en.json"),
            include_str!("../../src/lib/locales/fr.json"),
            include_str!("../../src/lib/locales/it.json"),
            include_str!("../../src/lib/locales/ro.json"),
            include_str!("../../src/lib/locales/pl.json"),
        ]
        .map(|text| serde_json::from_str(text).expect("bundled locale JSON"))
    });
    &all[match language {
        "fr" => 1,
        "it" => 2,
        "ro" => 3,
        "pl" => 4,
        _ => 0,
    }]
}

fn text(language: &str, path: &str) -> String {
    translations(language)
        .pointer(path)
        .and_then(Value::as_str)
        .or_else(|| translations("en").pointer(path).and_then(Value::as_str))
        .unwrap_or(path)
        .to_owned()
}

fn refresh(app: &AppHandle) -> tauri::Result<()> {
    let state = app.state::<AppState>();
    let now = now_ms();
    let (view, language) = {
        let config = state.config.lock().unwrap();
        let applied = state.launch_protection_status.lock().unwrap();
        // No app-state lock is retained during native menu updates.
        (
            TrayProtectionState::new(&config, &applied, now),
            config.language.clone().unwrap_or_else(|| "en".into()),
        )
    };
    let menu = app.state::<TrayMenu>();
    let mut previous = menu.previous.lock().unwrap();
    if previous.as_ref() == Some(&(view.clone(), language.clone())) {
        return Ok(());
    }
    let mut status = text(
        &language,
        &format!("/launchProtection/status/{}", view.phase),
    );
    if view.phase == "paused" {
        status.push(' ');
        status.push_str(
            &text(&language, "/tray/remaining")
                .replace("{minutes}", &view.remaining_minutes.to_string()),
        );
    }
    menu.show.set_text(text(&language, "/tray/show"))?;
    menu.quit.set_text(text(&language, "/tray/quit"))?;
    menu.status.set_text(&status)?;
    menu.toggle.set_text(text(
        &language,
        if view.enabled {
            "/tray/disable"
        } else {
            "/tray/enable"
        },
    ))?;
    menu.pause.set_text(text(&language, "/tray/pause"))?;
    menu.pause.set_enabled(view.enabled)?;
    for (minutes, item) in &menu.durations {
        item.set_text(
            text(&language, "/launchProtection/duration")
                .replace("{minutes}", &minutes.to_string()),
        )?;
    }
    menu.resume
        .set_text(text(&language, "/launchProtection/resume"))?;
    menu.resume.set_enabled(view.resume_enabled)?;
    if let Some(tray) = app.tray_by_id("main") {
        tray.set_tooltip(Some(format!("HideMyWindows — {status}")))?;
    }
    *previous = Some((view, language));
    Ok(())
}

fn apply_action(app: &AppHandle, action: TrayProtectionAction) -> Result<(), String> {
    let state = app.state::<AppState>();
    let next = {
        let mut current = state.config.lock().unwrap();
        let mut next = current.clone();
        if !action.apply(&mut next, now_ms()) {
            return Ok(());
        }
        // Commit only after persistence succeeds. Mutate the latest config so
        // tray actions retain current rules and other settings.
        next.save(&state.config_path)
            .map_err(|error| error.to_string())?;
        *current = next.clone();
        next
    };
    let _ = state.rules_wake.send(());
    let _ = app.emit("config-changed", next);
    Ok(())
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id("show", "Show HideMyWindows").build(app)?;
    let status = MenuItemBuilder::with_id("protection-status", "Applying protection settings…")
        .enabled(false)
        .build(app)?;
    let toggle =
        MenuItemBuilder::with_id("protection-toggle", "Disable protection before apps appear")
            .build(app)?;
    let mut durations = Vec::new();
    for minutes in [15, 30, 60, 120] {
        let item = MenuItemBuilder::with_id(
            format!("protection-pause-{minutes}"),
            format!("{minutes} minutes"),
        )
        .build(app)?;
        durations.push((minutes, item));
    }
    let mut pause = SubmenuBuilder::new(app, "Pause protection before apps appear");
    for (_, item) in &durations {
        pause = pause.item(item);
    }
    let pause = pause.build()?;
    let resume = MenuItemBuilder::with_id("protection-resume", "Resume now")
        .enabled(false)
        .build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&show)
        .separator()
        .items(&[&status, &toggle, &pause, &resume])
        .separator()
        .item(&quit)
        .build()?;

    app.manage(TrayMenu {
        show,
        status,
        toggle,
        pause,
        durations,
        resume,
        quit,
        previous: Mutex::new(None),
    });
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("HideMyWindows")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            id => {
                if let Some(action) = TrayProtectionAction::from_menu_id(id) {
                    if let Err(error) = apply_action(app, action) {
                        show_main_window(app);
                        let _ = app.emit("tray-action-error", error);
                    }
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    refresh(app)?;
    let app = app.clone();
    std::thread::spawn(move || loop {
        let handle = app.clone();
        if app
            .run_on_main_thread(move || {
                let _ = refresh(&handle);
            })
            .is_err()
        {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    });
    Ok(())
}
