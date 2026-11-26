//! System tray icon and menu.

use crate::launcher::Status;
use crate::AppState;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

const TRAY_ID: &str = "main";

static PAUSE_ITEM: std::sync::OnceLock<CheckMenuItem<tauri::Wry>> = std::sync::OnceLock::new();

pub fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let paused = app.state::<AppState>().config.lock().unwrap().settings.paused;
    let open = MenuItem::with_id(app, "open", "Open settings", true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, "pause", "Pause launching", true, paused, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &pause, &PredefinedMenuItem::separator(app)?, &quit])?;
    let _ = PAUSE_ITEM.set(pause.clone());

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().cloned().expect("app icon"))
        .tooltip("MSFS Autostart — waiting for MSFS")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "open" => show_window(app),
            "pause" => {
                let state = app.state::<AppState>();
                let paused = {
                    let mut cfg = state.config.lock().unwrap();
                    cfg.settings.paused = !cfg.settings.paused;
                    cfg.settings.paused
                };
                let _ = pause.set_checked(paused);
                crate::persist_config(app);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn update_tooltip(app: &AppHandle, s: &Status) {
    let text = if !s.sim_running {
        "waiting for MSFS".to_string()
    } else if !s.connected {
        "MSFS starting…".to_string()
    } else if s.in_flight && !s.aircraft_title.is_empty() {
        s.aircraft_title.clone()
    } else {
        "in menus".to_string()
    };
    let running = s.programs.iter().filter(|p| p.alive).count();
    let mut tip = format!("MSFS Autostart — {text}");
    if running > 0 {
        tip.push_str(&format!(" ({running} running)"));
    }
    if s.paused {
        tip.push_str(" [paused]");
    }
    if let Some(t) = app.tray_by_id(TRAY_ID) {
        // Windows limits tray tooltips to 127 chars.
        let _ = t.set_tooltip(Some(tip.chars().take(120).collect::<String>()));
    }
}

/// Keeps the tray's Pause checkbox in sync when settings change from the UI.
pub fn sync_pause(paused: bool) {
    if let Some(item) = PAUSE_ITEM.get() {
        let _ = item.set_checked(paused);
    }
}
