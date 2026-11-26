mod catalog;
mod config;
mod icons;
mod launcher;
pub mod sim;
mod tray;

use catalog::Catalog;
use config::{Config, Program};
use launcher::{Msg, Status};
use serde::Serialize;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_dialog::DialogExt as _;

pub struct AppState {
    pub config: Mutex<Config>,
    pub catalog: Mutex<Catalog>,
    pub index: Mutex<catalog::Index>,
    pub status: Mutex<Status>,
    pub log: Mutex<VecDeque<LogLine>>,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub launcher_tx: Mutex<Sender<Msg>>,
    pub scanning: std::sync::atomic::AtomicBool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    pub ts: u64,
    pub msg: String,
}

pub fn log(app: &AppHandle, msg: String) {
    eprintln!("{msg}");
    let line = LogLine { ts: catalog::now_secs(), msg };
    if let Some(st) = app.try_state::<AppState>() {
        let mut l = st.log.lock().unwrap();
        l.push_back(line.clone());
        while l.len() > 300 {
            l.pop_front();
        }
        drop(l);
        // Also keep a file (activity.log in the app data folder) for troubleshooting.
        use std::io::Write as _;
        if let Ok(mut f) =
            std::fs::OpenOptions::new().create(true).append(true).open(st.data_dir.join("activity.log"))
        {
            let _ = writeln!(f, "{} {}", line.ts, line.msg);
        }
    }
    let _ = app.emit("log", line);
}

pub fn persist_config(app: &AppHandle) {
    let st = app.state::<AppState>();
    let cfg = st.config.lock().unwrap().clone();
    if let Err(e) = config::save(&st.config_dir, &cfg) {
        log(app, format!("Failed to save config: {e}"));
    }
    tray::sync_pause(cfg.settings.paused);
    let _ = st.launcher_tx.lock().unwrap().send(Msg::ConfigChanged);
    let _ = app.emit("config-changed", cfg);
}

// ---------------------------------------------------------------------------
// Commands

#[tauri::command]
fn get_config(state: tauri::State<AppState>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn save_config(app: AppHandle, config: Config) {
    *app.state::<AppState>().config.lock().unwrap() = config;
    persist_config(&app);
}

#[tauri::command]
fn get_status(state: tauri::State<AppState>) -> Status {
    state.status.lock().unwrap().clone()
}

#[tauri::command]
fn get_log(state: tauri::State<AppState>) -> Vec<LogLine> {
    state.log.lock().unwrap().iter().cloned().collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogResponse {
    catalog: Catalog,
    scanning: bool,
}

#[tauri::command]
fn get_catalog(state: tauri::State<AppState>) -> CatalogResponse {
    CatalogResponse {
        catalog: state.catalog.lock().unwrap().clone(),
        scanning: state.scanning.load(std::sync::atomic::Ordering::Relaxed),
    }
}

#[tauri::command]
fn rescan_catalog(app: AppHandle) {
    start_scan(app);
}

fn start_scan(app: AppHandle) {
    use std::sync::atomic::Ordering;
    if app.state::<AppState>().scanning.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = app.emit("catalog-changed", ());
    log(&app, "Scanning installed aircraft…".into());
    let extra: Vec<(catalog::Sim, PathBuf)> = app
        .state::<AppState>()
        .config
        .lock()
        .unwrap()
        .settings
        .extra_package_paths
        .iter()
        .map(|e| (e.sim, PathBuf::from(e.path.trim())))
        .collect();
    std::thread::spawn(move || {
        let t = std::time::Instant::now();
        let mut fresh = catalog::scan(&extra);
        let st = app.state::<AppState>();
        {
            // Keep entries that only SimConnect knows about (MSFS 2024 streamed aircraft).
            let old = st.catalog.lock().unwrap();
            let pairs: Vec<(String, String)> = old
                .aircraft
                .iter()
                .filter(|a| a.source == "simconnect")
                .flat_map(|a| a.liveries.iter().map(|l| (a.name.clone(), l.title.clone())))
                .collect();
            catalog::merge_simconnect(&mut fresh, &pairs);
        }
        if let Err(e) = catalog::save_cache(&st.data_dir, &fresh) {
            log(&app, format!("Couldn't save aircraft cache: {e}"));
        }
        log(&app, format!("Found {} aircraft in {:.1}s", fresh.aircraft.len(), t.elapsed().as_secs_f32()));
        *st.index.lock().unwrap() = catalog::Index::build(&fresh);
        *st.catalog.lock().unwrap() = fresh;
        st.scanning.store(false, Ordering::SeqCst);
        let _ = app.emit("catalog-changed", ());
    });
}

#[tauri::command]
async fn test_launch(program: Program) -> Result<Option<u32>, String> {
    tauri::async_runtime::spawn_blocking(move || launcher::launch(&program))
        .await
        .map_err(|e| e.to_string())?
}

/// MSFS installs found through their UserCfg.opt (Steam and Microsoft Store).
#[tauri::command]
async fn detect_installs() -> Vec<catalog::Install> {
    tauri::async_runtime::spawn_blocking(catalog::detect_installs).await.unwrap_or_default()
}

/// Whether a folder looks like an MSFS packages folder.
#[tauri::command]
fn check_packages_path(path: String) -> catalog::PathCheck {
    catalog::check_packages_path(std::path::Path::new(path.trim()))
}

/// The program's icon as a PNG data URL (None if the file is missing or has no icon).
#[tauri::command]
async fn get_program_icon(path: String) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || icons::icon_data_url(&path)).await.ok().flatten()
}

#[tauri::command]
async fn pick_file(app: AppHandle, kind: String) -> Option<String> {
    let mut d = app.dialog().file();
    d = match kind.as_str() {
        "folder" => return app.dialog().file().blocking_pick_folder().map(|p| p.to_string()),
        _ => d.add_filter("Programs", &["exe", "bat", "cmd", "lnk"]),
    };
    d.blocking_pick_file().map(|p| p.to_string())
}

#[tauri::command]
fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let a = app.autolaunch();
    if enabled { a.enable() } else { a.disable() }.map_err(|e| e.to_string())
}


// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_window(app)))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            let config = config::load(&config_dir);
            let cached = catalog::load_cache(&data_dir);
            // Caches from older versions lack the AI-title list (and 2024 modular aircraft).
            let need_scan = cached.as_ref().is_none_or(|c| c.ai_titles.is_empty());
            // Start each session with a fresh log file.
            let _ = std::fs::create_dir_all(&data_dir);
            let _ = std::fs::write(data_dir.join("activity.log"), "");
            let cat = cached.unwrap_or_default();
            let (tx, rx) = channel::<Msg>();

            app.manage(AppState {
                config: Mutex::new(config),
                index: Mutex::new(catalog::Index::build(&cat)),
                catalog: Mutex::new(cat),
                status: Mutex::new(Status::default()),
                log: Mutex::new(VecDeque::new()),
                config_dir,
                data_dir,
                launcher_tx: Mutex::new(tx.clone()),
                scanning: Default::default(),
            });

            {
                let st = handle.state::<AppState>();
                let c = st.catalog.lock().unwrap();
                let msg = format!(
                    "Aircraft cache: {} aircraft, {} AI titles ({}){}",
                    c.aircraft.len(),
                    c.ai_titles.len(),
                    catalog::cache_path(&st.data_dir).display(),
                    if need_scan { "; rescanning" } else { "" }
                );
                drop(c);
                log(&handle, msg);
            }
            tray::create(&handle)?;
            launcher::spawn(handle.clone(), tx.clone(), rx);

            let (sim_tx, sim_rx) = channel();
            std::thread::spawn(move || {
                for ev in sim_rx {
                    if tx.send(Msg::Sim(ev)).is_err() {
                        break;
                    }
                }
            });
            let h2 = handle.clone();
            sim::spawn(sim_tx, launcher::sim_process_running, move || {
                sim::dll_candidates(h2.path().resource_dir().ok())
            });

            if need_scan {
                start_scan(handle.clone());
            }
            if !std::env::args().any(|a| a == "--hidden") {
                tray::show_window(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window just hides it; the app keeps running in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            get_status,
            get_log,
            get_catalog,
            rescan_catalog,
            test_launch,
            pick_file,
            get_program_icon,
            detect_installs,
            check_packages_path,
            get_autostart,
            set_autostart,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
