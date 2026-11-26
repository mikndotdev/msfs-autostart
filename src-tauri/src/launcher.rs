//! Launcher state machine: turns sim state + rules into running/closed programs.

use crate::catalog::{self, folder_from_loaded_path};
use crate::config::{wildcard_match, CloseMode, Config, Program, Rule, Trigger};
use crate::sim::SimEvent;
use crate::{log, AppState};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tauri::{AppHandle, Emitter, Manager};

const SIM_PROCESS_NAMES: [&str; 2] = ["flightsimulator.exe", "flightsimulator2024.exe"];

static SIM_PROCESS: AtomicBool = AtomicBool::new(false);

pub fn sim_process_running() -> bool {
    SIM_PROCESS.load(Ordering::Relaxed)
}

/// What a `CAMERA STATE` value says about being in a flight.
/// Values observed in MSFS 2024: 12 world map, 35/32/30 loading, 2 cockpit,
/// 29/34 pause/end-of-flight menus. Unknown values keep the previous state, so a
/// pause menu or an unusual camera doesn't end the flight.
#[derive(Debug, PartialEq)]
enum CameraKind {
    Flying,
    Menu,
    Unknown,
}

fn camera_kind(c: i32) -> CameraKind {
    match c {
        // Cockpit, external, drone, fixed, environment, 6DoF, gameplay, showcase,
        // drone aircraft, in-game RTC, replay, top-down drone, ground, follow traffic.
        2..=10 | 16..=19 | 24 | 25 => CameraKind::Flying,
        // Waiting/loading, world map, hangar/menu RTC, hangar, and 2024 loading screens.
        11..=15 | 21 | 30 | 32 | 35 => CameraKind::Menu,
        _ => CameraKind::Unknown,
    }
}

pub enum Msg {
    Sim(SimEvent),
    ConfigChanged,
    Launched { key: Key, result: Result<Option<u32>, String> },
}

/// (rule id, program id)
pub type Key = (String, String);

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub sim_running: bool,
    pub connected: bool,
    pub sim_name: String,
    pub dll: String,
    pub in_flight: bool,
    pub camera: Option<i32>,
    pub aircraft_path: String,
    pub aircraft_title: String,
    pub livery_name: String,
    /// Catalog ids matching the loaded aircraft.
    pub aircraft_ids: Vec<String>,
    pub active_rules: Vec<String>,
    pub programs: Vec<ProgramStatus>,
    pub last_error: String,
    pub paused: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProgramStatus {
    pub rule_id: String,
    pub program_id: String,
    pub exe: String,
    pub pid: Option<u32>,
    /// Number of live processes being tracked for this program.
    pub process_count: usize,
    /// Started by us (we only close what we started).
    pub owned: bool,
    pub alive: bool,
    /// "starting", "running", "exited", "stopping"
    pub state: String,
}

enum Slot {
    PendingStart {
        at: Instant,
    },
    Launching {
        since: u64,
    },
    Running {
        /// Every process we consider part of this program: the launched pid, its
        /// descendants, and processes of the same exe started after launch.
        pids: HashSet<u32>,
        owned: bool,
        exe: String,
        launched_at: u64,
        stop_at: Option<Instant>,
    },
}

struct Launcher {
    app: AppHandle,
    tx: Sender<Msg>,
    sys: System,
    last_proc_scan: Option<Instant>,
    sim_process: bool,
    connected: bool,
    sim_name: String,
    dll: String,
    sim_state: bool,
    connected_at: Option<Instant>,
    camera: Option<i32>,
    in_flight: bool,
    aircraft_path: String,
    aircraft_title: String,
    livery_name: String,
    last_error: String,
    slots: HashMap<Key, Slot>,
}

pub fn spawn(app: AppHandle, tx: Sender<Msg>, rx: Receiver<Msg>) {
    std::thread::Builder::new()
        .name("launcher".into())
        .spawn(move || {
            let mut l = Launcher {
                app,
                tx,
                sys: System::new().expect("init sysinfo"),
                last_proc_scan: None,
                sim_process: false,
                connected: false,
                sim_name: String::new(),
                dll: String::new(),
                sim_state: false,
                connected_at: None,
                camera: None,
                in_flight: false,
                aircraft_path: String::new(),
                aircraft_title: String::new(),
                livery_name: String::new(),
                last_error: String::new(),
                slots: HashMap::new(),
            };
            loop {
                match rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(msg) => l.handle(msg),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(_) => break,
                }
                while let Ok(msg) = rx.try_recv() {
                    l.handle(msg);
                }
                l.tick();
            }
        })
        .expect("spawn launcher thread");
}

impl Launcher {
    fn state(&self) -> tauri::State<'_, AppState> {
        self.app.state::<AppState>()
    }

    fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Sim(ev) => self.on_sim(ev),
            Msg::ConfigChanged => {}
            Msg::Launched { key, result } => {
                let exe = self.program(&key).map(|p| p.exe).unwrap_or_default();
                let launched_at = match self.slots.get(&key) {
                    Some(Slot::Launching { since }) => *since,
                    _ => catalog::now_secs(),
                };
                let slot = match result {
                    Ok(pid) => {
                        log(
                            &self.app,
                            format!("Started {} (pid {})", file_name(&exe), pid.map_or("?".into(), |p| p.to_string())),
                        );
                        Slot::Running { pids: pid.into_iter().collect(), owned: true, exe, launched_at, stop_at: None }
                    }
                    Err(e) => {
                        log(&self.app, format!("Failed to start: {e}"));
                        // Keep a dead slot so we don't retry in a loop.
                        Slot::Running { pids: HashSet::new(), owned: false, exe, launched_at, stop_at: None }
                    }
                };
                if self.slots.contains_key(&key) {
                    self.slots.insert(key, slot);
                }
                // Pick up child processes right away.
                self.refresh_processes(true);
            }
        }
    }

    fn on_sim(&mut self, ev: SimEvent) {
        match ev {
            SimEvent::Connected { app_name, dll } => {
                log(&self.app, format!("Connected to {app_name}"));
                self.connected = true;
                self.connected_at = Some(Instant::now());
                self.sim_name = app_name;
                self.dll = dll;
                self.last_error.clear();
            }
            SimEvent::Disconnected => {
                log(&self.app, "Disconnected from sim".to_string());
                self.connected = false;
                self.sim_state = false;
                self.connected_at = None;
                self.camera = None;
                self.set_in_flight(false, "disconnected");
                self.aircraft_path.clear();
                self.aircraft_title.clear();
                self.livery_name.clear();
            }
            SimEvent::AircraftPath(p) => {
                if p != self.aircraft_path {
                    log(&self.app, format!("Aircraft: {p}"));
                    self.aircraft_path = p;
                }
            }
            SimEvent::AircraftTitle(t) => {
                if t != self.aircraft_title {
                    log(&self.app, format!("Aircraft title: {t}"));
                    self.aircraft_title = t;
                }
            }
            SimEvent::LiveryName(l) => self.livery_name = l,
            SimEvent::SimState(s) => {
                self.sim_state = s;
                if !s {
                    // SimStop only happens outside a flight (menus/loading).
                    self.set_in_flight(false, "SimStop");
                }
                // SimStart alone isn't trusted: MSFS 2024 reports it in the main menu.
                // The camera decides; see `tick` for the no-camera fallback.
            }
            SimEvent::Camera(c) => {
                if self.camera != Some(c) {
                    self.camera = Some(c);
                    match camera_kind(c) {
                        CameraKind::Flying if self.sim_state => self.set_in_flight(true, &format!("camera {c}")),
                        CameraKind::Menu => self.set_in_flight(false, &format!("camera {c}")),
                        _ => {}
                    }
                }
            }
            SimEvent::FlightFile(_) | SimEvent::Debug(_) => {}
            SimEvent::AircraftList(pairs) => {
                let st = self.state();
                let mut cat = st.catalog.lock().unwrap();
                let added = catalog::merge_simconnect(&mut cat, &pairs);
                if added > 0 {
                    if let Err(e) = catalog::save_cache(&st.data_dir, &cat) {
                        log(&self.app, format!("Couldn't save aircraft cache: {e}"));
                    }
                    *st.index.lock().unwrap() = catalog::Index::build(&cat);
                }
                drop(cat);
                if added > 0 {
                    let _ = self.app.emit("catalog-changed", ());
                }
                log(
                    &self.app,
                    format!("SimConnect listed {} aircraft/liveries ({added} new aircraft added)", pairs.len()),
                );
            }
            SimEvent::Error(e) => {
                if e != self.last_error {
                    log(&self.app, format!("SimConnect: {e}"));
                    self.last_error = e;
                }
            }
        }
    }

    fn set_in_flight(&mut self, f: bool, why: &str) {
        if f != self.in_flight {
            self.in_flight = f;
            log(&self.app, if f { format!("Flight started ({why})") } else { format!("Left flight ({why})") });
        }
    }

    fn program(&self, key: &Key) -> Option<Program> {
        let st = self.state();
        let cfg = st.config.lock().unwrap();
        cfg.rules.iter().find(|r| r.id == key.0)?.programs.iter().find(|p| p.id == key.1).cloned()
    }

    fn sim_running(&self) -> bool {
        self.sim_process || self.connected
    }

    fn titles(&self) -> Vec<&str> {
        [self.aircraft_title.as_str(), self.livery_name.as_str()].into_iter().filter(|s| !s.is_empty()).collect()
    }

    fn aircraft_ids(&self) -> HashSet<String> {
        let folder = folder_from_loaded_path(&self.aircraft_path);
        let st = self.state();
        let idx = st.index.lock().unwrap();
        let mut ids = idx.ids_for(folder.as_deref(), None);
        for t in self.titles() {
            ids.extend(idx.ids_for(None, Some(t)));
        }
        ids
    }

    fn rule_active(&self, rule: &Rule, ids: &HashSet<String>) -> bool {
        if !rule.enabled {
            return false;
        }
        match &rule.trigger {
            Trigger::Sim { .. } => self.sim_running(),
            Trigger::Aircraft { aircraft, patterns } => {
                if !(self.connected && self.in_flight) {
                    return false;
                }
                let folder = folder_from_loaded_path(&self.aircraft_path).unwrap_or_default();
                let titles = self.titles();
                let ci = |a: &str, b: &str| !a.is_empty() && a.eq_ignore_ascii_case(b);
                aircraft.iter().any(|r| {
                    ids.contains(&r.id)
                        || r.folders.iter().any(|f| ci(&folder, f))
                        || r.titles.iter().any(|t| titles.iter().any(|x| ci(x, t)))
                }) || patterns.iter().map(|p| p.trim()).filter(|p| !p.is_empty()).any(|p| {
                    titles.iter().any(|t| wildcard_match(p, t)) || wildcard_match(p, &self.aircraft_path)
                })
            }
        }
    }

    fn refresh_processes(&mut self, force: bool) {
        if !force && self.last_proc_scan.is_some_and(|t| t.elapsed() < Duration::from_secs(2)) {
            return;
        }
        self.last_proc_scan = Some(Instant::now());
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
        let running = self.sys.processes().values().any(|p| {
            let n = p.name().to_string_lossy().to_lowercase();
            SIM_PROCESS_NAMES.contains(&n.as_str())
        });
        if running != self.sim_process {
            log(&self.app, if running { "MSFS process detected".into() } else { "MSFS process exited".to_string() });
        }
        self.sim_process = running;
        SIM_PROCESS.store(running, Ordering::Relaxed);
        self.track_processes();
    }

    /// Extends each owned slot's pid set with child processes and processes of the
    /// same exe started after launch. Handles launcher stubs, shortcuts and apps
    /// that hand off to a new process.
    fn track_processes(&mut self) {
        let procs = self.sys.processes();
        for slot in self.slots.values_mut() {
            let Slot::Running { pids, owned: true, exe, launched_at, .. } = slot else { continue };
            pids.retain(|p| procs.contains_key(&Pid::from_u32(*p)));
            let want = Path::new(exe.as_str());
            let is_exe = exe.to_lowercase().ends_with(".exe");
            loop {
                let before = pids.len();
                for (pid, p) in procs {
                    let id = pid.as_u32();
                    if pids.contains(&id) || p.start_time() + 2 < *launched_at {
                        continue;
                    }
                    let child = p.parent().is_some_and(|pp| pids.contains(&pp.as_u32()));
                    let same_exe = is_exe && p.exe().is_some_and(|e| same_path(e, want));
                    if child || same_exe {
                        pids.insert(id);
                    }
                }
                if pids.len() == before {
                    break;
                }
            }
        }
    }

    fn alive_pids(&self, pids: &HashSet<u32>) -> Vec<u32> {
        let mut v: Vec<u32> = pids.iter().copied().filter(|p| self.sys.process(Pid::from_u32(*p)).is_some()).collect();
        v.sort();
        v
    }

    fn exe_running(&self, exe: &str) -> Option<u32> {
        if !exe.to_lowercase().ends_with(".exe") {
            return None;
        }
        let want = Path::new(exe);
        self.sys.processes().iter().find_map(|(pid, p)| {
            let path = p.exe()?;
            same_path(path, want).then(|| pid.as_u32())
        })
    }

    fn tick(&mut self) {
        self.refresh_processes(false);
        // Fallback for sims that never report CAMERA STATE: trust the sim state, but
        // only after giving the camera a few seconds to show up after connecting.
        if self.connected
            && self.sim_state
            && self.camera.is_none()
            && self.connected_at.is_some_and(|t| t.elapsed() > Duration::from_secs(5))
        {
            self.set_in_flight(true, "SimStart, no camera data");
        }
        let cfg: Config = self.state().config.lock().unwrap().clone();
        let ids = self.aircraft_ids();
        let now = Instant::now();
        let grace = Duration::from_secs(cfg.settings.stop_grace_secs);

        let mut desired: HashMap<Key, Program> = HashMap::new();
        let mut active_rules = Vec::new();
        for rule in &cfg.rules {
            if self.rule_active(rule, &ids) {
                active_rules.push(rule.id.clone());
                for p in &rule.programs {
                    if !p.exe.trim().is_empty() {
                        desired.insert((rule.id.clone(), p.id.clone()), p.clone());
                    }
                }
            }
        }

        // Start what should be running.
        for (key, prog) in &desired {
            match self.slots.get_mut(key) {
                None => {
                    if !cfg.settings.paused {
                        self.slots.insert(
                            key.clone(),
                            Slot::PendingStart { at: now + Duration::from_secs(prog.start_delay_secs) },
                        );
                    }
                }
                Some(Slot::Running { stop_at, exe, .. }) if stop_at.is_some() => {
                    log(&self.app, format!("Keeping {} open (rule active again)", file_name(exe)));
                    *stop_at = None;
                }
                _ => {}
            }
        }

        // Schedule stops / drop pending starts for what shouldn't run anymore.
        let mut forget = Vec::new();
        let mut scheduled = Vec::new();
        for (key, slot) in self.slots.iter() {
            if desired.contains_key(key) {
                continue;
            }
            match slot {
                Slot::PendingStart { .. } => forget.push(key.clone()),
                Slot::Launching { .. } => {}
                Slot::Running { pids, owned, stop_at, exe, .. } => {
                    let prog = cfg
                        .rules
                        .iter()
                        .find(|r| r.id == key.0)
                        .and_then(|r| r.programs.iter().find(|p| p.id == key.1));
                    let alive = !self.alive_pids(pids).is_empty();
                    match prog {
                        Some(p) if p.close_on_stop && *owned && alive => {
                            if stop_at.is_none() {
                                scheduled.push((key.clone(), exe.clone()));
                            }
                        }
                        _ => forget.push(key.clone()),
                    }
                }
            }
        }
        for (k, exe) in scheduled {
            log(&self.app, format!("Rule stopped; closing {} in {}s", file_name(&exe), grace.as_secs()));
            if let Some(Slot::Running { stop_at, .. }) = self.slots.get_mut(&k) {
                *stop_at = Some(now + grace);
            }
        }
        for k in forget {
            self.slots.remove(&k);
        }

        // Fire due actions.
        let keys: Vec<Key> = self.slots.keys().cloned().collect();
        for key in keys {
            let due_start = matches!(self.slots.get(&key), Some(Slot::PendingStart { at }) if *at <= now);
            if due_start {
                let Some(prog) = desired.get(&key).cloned() else { continue };
                if cfg.settings.paused {
                    self.slots.remove(&key);
                    continue;
                }
                self.refresh_processes(true);
                if let Some(pid) = self.exe_running(&prog.exe) {
                    log(
                        &self.app,
                        format!(
                            "{} is already running (pid {pid}); not starting another, and it won't be closed",
                            file_name(&prog.exe)
                        ),
                    );
                    self.slots.insert(
                        key,
                        Slot::Running {
                            pids: [pid].into(),
                            owned: false,
                            exe: prog.exe,
                            launched_at: catalog::now_secs(),
                            stop_at: None,
                        },
                    );
                } else {
                    self.slots.insert(key.clone(), Slot::Launching { since: catalog::now_secs() });
                    let tx = self.tx.clone();
                    std::thread::spawn(move || {
                        let result = launch(&prog);
                        let _ = tx.send(Msg::Launched { key, result });
                    });
                }
                continue;
            }
            let due_stop = matches!(self.slots.get(&key), Some(Slot::Running { stop_at: Some(t), .. }) if *t <= now);
            if due_stop && let Some(Slot::Running { pids, exe, .. }) = self.slots.remove(&key) {
                let alive = self.alive_pids(&pids);
                if alive.is_empty() {
                    continue;
                }
                let mode = self.program(&key).map(|p| p.close_mode).unwrap_or(CloseMode::Graceful);
                let timeout = Duration::from_secs(cfg.settings.close_timeout_secs);
                log(&self.app, format!("Closing {} (pids {alive:?})", file_name(&exe)));
                let app = self.app.clone();
                std::thread::spawn(move || match close_processes(&alive, mode, timeout) {
                    Ok(how) => log(&app, format!("Closed {} ({how})", file_name(&exe))),
                    Err(e) => log(&app, format!("Couldn't close {}: {e}", file_name(&exe))),
                });
            }
        }

        self.publish(&cfg, ids, active_rules);
    }

    fn publish(&self, cfg: &Config, ids: HashSet<String>, active_rules: Vec<String>) {
        let mut programs: Vec<ProgramStatus> = self
            .slots
            .iter()
            .map(|(k, s)| {
                let exe = cfg
                    .rules
                    .iter()
                    .find(|r| r.id == k.0)
                    .and_then(|r| r.programs.iter().find(|p| p.id == k.1))
                    .map(|p| p.exe.clone())
                    .unwrap_or_default();
                let (pid, count, owned, alive, state) = match s {
                    Slot::PendingStart { .. } | Slot::Launching { .. } => (None, 0, true, false, "starting"),
                    Slot::Running { pids, owned, stop_at, .. } => {
                        let alive = self.alive_pids(pids);
                        let st = if stop_at.is_some() {
                            "stopping"
                        } else if !alive.is_empty() {
                            "running"
                        } else {
                            "exited"
                        };
                        (alive.first().copied(), alive.len(), *owned, !alive.is_empty(), st)
                    }
                };
                ProgramStatus {
                    rule_id: k.0.clone(),
                    program_id: k.1.clone(),
                    exe,
                    pid,
                    process_count: count,
                    owned,
                    alive,
                    state: state.into(),
                }
            })
            .collect();
        programs.sort_by(|a, b| (&a.rule_id, &a.program_id).cmp(&(&b.rule_id, &b.program_id)));
        let mut aircraft_ids: Vec<String> = ids.into_iter().collect();
        aircraft_ids.sort();

        let status = Status {
            sim_running: self.sim_running(),
            connected: self.connected,
            sim_name: self.sim_name.clone(),
            dll: self.dll.clone(),
            in_flight: self.in_flight,
            camera: self.camera,
            aircraft_path: self.aircraft_path.clone(),
            aircraft_title: self.aircraft_title.clone(),
            livery_name: self.livery_name.clone(),
            aircraft_ids,
            active_rules,
            programs,
            last_error: self.last_error.clone(),
            paused: cfg.settings.paused,
        };
        let st = self.state();
        let mut cur = st.status.lock().unwrap();
        if *cur != status {
            *cur = status.clone();
            drop(cur);
            crate::tray::update_tooltip(&self.app, &status);
            let _ = self.app.emit("status", status);
        }
    }
}

fn file_name(p: &str) -> String {
    Path::new(p).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| p.to_string())
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_start_matches("\\\\?\\").to_lowercase();
    norm(a) == norm(b)
}

// ---------------------------------------------------------------------------
// Win32 process control

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Launches via ShellExecuteEx so `.exe`, `.lnk`, `.bat` and elevation all work the
/// same way they do from Explorer. Returns the pid when Windows gives us a handle.
pub fn launch(p: &Program) -> Result<Option<u32>, String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
    use windows::Win32::System::Threading::GetProcessId;
    use windows::Win32::UI::Shell::{
        ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let exe = p.exe.trim().trim_matches('"');
    if !Path::new(exe).exists() {
        return Err(format!("{exe} does not exist"));
    }
    let cwd = if p.cwd.trim().is_empty() {
        Path::new(exe).parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default()
    } else {
        p.cwd.trim().to_string()
    };
    let file = wide(exe);
    let params = wide(p.args.trim());
    let dir = wide(&cwd);
    let verb = wide(if p.run_as_admin { "runas" } else { "open" });

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI,
            lpVerb: PCWSTR(verb.as_ptr()),
            lpFile: PCWSTR(file.as_ptr()),
            lpParameters: if p.args.trim().is_empty() { PCWSTR::null() } else { PCWSTR(params.as_ptr()) },
            lpDirectory: if cwd.is_empty() { PCWSTR::null() } else { PCWSTR(dir.as_ptr()) },
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        ShellExecuteExW(&mut info).map_err(|e| format!("{exe}: {}", e.message()))?;
        if info.hProcess.is_invalid() {
            return Ok(None);
        }
        let pid = GetProcessId(info.hProcess);
        let _ = CloseHandle(info.hProcess);
        Ok((pid != 0).then_some(pid))
    }
}

/// Graceful: post WM_CLOSE to the processes' visible top-level windows, wait up to
/// `timeout`, then terminate whatever is left. Returns a short description.
pub fn close_processes(pids: &[u32], mode: CloseMode, timeout: Duration) -> Result<String, String> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WPARAM};
    use windows::Win32::System::Threading::{
        OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE,
    };

    struct Target<'a> {
        pids: &'a [u32],
        posted: u32,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let target = &mut *(lparam.0 as *mut Target);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if target.pids.contains(&pid)
                && IsWindowVisible(hwnd).as_bool()
                && PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)).is_ok()
            {
                target.posted += 1;
            }
            BOOL(1)
        }
    }

    unsafe {
        let mut handles: Vec<(u32, HANDLE)> = Vec::new();
        let mut errors = Vec::new();
        for &pid in pids {
            match OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, false, pid) {
                Ok(h) => handles.push((pid, h)),
                Err(e) => errors.push(format!("pid {pid}: {} (running as admin?)", e.message())),
            }
        }
        if handles.is_empty() {
            return Err(errors.join("; "));
        }

        let mut graceful = 0;
        if mode == CloseMode::Graceful {
            let mut t = Target { pids, posted: 0 };
            let _ = EnumWindows(Some(enum_proc), LPARAM(&mut t as *mut _ as isize));
            if t.posted > 0 {
                let deadline = Instant::now() + timeout;
                for (_, h) in &handles {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if WaitForSingleObject(*h, left.as_millis() as u32) == WAIT_OBJECT_0 {
                        graceful += 1;
                    }
                }
            }
        }
        let mut killed = 0;
        for (pid, h) in &handles {
            if WaitForSingleObject(*h, 0) != WAIT_OBJECT_0 {
                match TerminateProcess(*h, 1) {
                    Ok(()) => killed += 1,
                    Err(e) => errors.push(format!("terminate pid {pid}: {}", e.message())),
                }
            }
            let _ = CloseHandle(*h);
        }
        if !errors.is_empty() && graceful + killed == 0 {
            return Err(errors.join("; "));
        }
        Ok(format!("{graceful} closed, {killed} force-closed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_kinds_from_2024_probe() {
        assert_eq!(camera_kind(12), CameraKind::Menu); // world map
        assert_eq!(camera_kind(35), CameraKind::Menu); // loading
        assert_eq!(camera_kind(32), CameraKind::Menu);
        assert_eq!(camera_kind(30), CameraKind::Menu);
        assert_eq!(camera_kind(2), CameraKind::Flying); // cockpit
        assert_eq!(camera_kind(29), CameraKind::Unknown); // pause menu: keep state
        assert_eq!(camera_kind(34), CameraKind::Unknown);
    }
}
