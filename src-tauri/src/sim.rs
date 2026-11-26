//! SimConnect client. `SimConnect.dll` is loaded at runtime (no SDK needed to build).
//! Runs on its own thread, reconnecting whenever the sim (re)starts, and reports
//! what it sees as [`SimEvent`]s.

use libloading::{Library, Symbol};
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum SimEvent {
    Connected { app_name: String, dll: String },
    Disconnected,
    /// Path from the `AircraftLoaded` system event/state.
    AircraftPath(String),
    /// `TITLE` simvar of the user aircraft.
    AircraftTitle(String),
    /// `LIVERY NAME` simvar (MSFS 2024).
    LiveryName(String),
    /// SimConnect "Sim" state: SimStart/SimStop events and the polled `Sim` system state.
    /// In MSFS 2024 this is also 1 in the main menu, so it isn't enough on its own.
    SimState(bool),
    /// Currently loaded .FLT file (`FlightLoaded` event/state).
    FlightFile(String),
    /// `CAMERA STATE` simvar.
    Camera(i32),
    /// Raw description of every message, for diagnostics.
    Debug(String),
    /// MSFS 2024 only: installed (aircraft title, livery) pairs.
    AircraftList(Vec<(String, String)>),
    Error(String),
}

type Handle = *mut c_void;
type HResult = i32;

type FnOpen = unsafe extern "system" fn(*mut Handle, *const c_char, *mut c_void, u32, *mut c_void, u32) -> HResult;
type FnClose = unsafe extern "system" fn(Handle) -> HResult;
type FnSubscribe = unsafe extern "system" fn(Handle, u32, *const c_char) -> HResult;
type FnRequestSystemState = unsafe extern "system" fn(Handle, u32, *const c_char) -> HResult;
type FnAddToDataDefinition =
    unsafe extern "system" fn(Handle, u32, *const c_char, *const c_char, u32, f32, u32) -> HResult;
type FnRequestDataOnSimObject = unsafe extern "system" fn(Handle, u32, u32, u32, u32, u32, u32, u32, u32) -> HResult;
type FnGetNextDispatch = unsafe extern "system" fn(Handle, *mut *mut Recv, *mut u32) -> HResult;
type FnEnumerateSimObjectsAndLiveries = unsafe extern "system" fn(Handle, u32, u32) -> HResult;

// SIMCONNECT_RECV_ID values (stable since FSX).
const RECV_EXCEPTION: u32 = 1;
const RECV_OPEN: u32 = 2;
const RECV_QUIT: u32 = 3;
const RECV_EVENT: u32 = 4;
const RECV_EVENT_FILENAME: u32 = 6;
const RECV_SIMOBJECT_DATA: u32 = 8;
const RECV_SYSTEM_STATE: u32 = 15;

const EV_SIM_START: u32 = 1;
const EV_SIM_STOP: u32 = 2;
const EV_AIRCRAFT_LOADED: u32 = 3;
const EV_FLIGHT_LOADED: u32 = 4;
const EV_SIM: u32 = 5;

const REQ_AIRCRAFT_STATE: u32 = 1;
const REQ_SIM_STATE: u32 = 2;
const REQ_TITLE: u32 = 3;
const REQ_ENUM_AIRCRAFT: u32 = 4;
const REQ_FLIGHT_STATE: u32 = 5;
const REQ_CAMERA: u32 = 6;
const REQ_LIVERY: u32 = 7;
const DEF_TITLE: u32 = 1;
const DEF_CAMERA: u32 = 2;
const DEF_LIVERY: u32 = 3;

const DATATYPE_INT32: u32 = 1;
const DATATYPE_STRING256: u32 = 9;
const PERIOD_SECOND: u32 = 4;
const DATA_REQUEST_FLAG_CHANGED: u32 = 1;
const OBJECT_ID_USER: u32 = 0;
const SIMOBJECT_TYPE_USER: u32 = 0;
const UNUSED: u32 = 0xFFFF_FFFF;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct Recv {
    size: u32,
    version: u32,
    id: u32,
}

#[repr(C, packed)]
struct RecvEvent {
    hdr: Recv,
    group_id: u32,
    event_id: u32,
    data: u32,
}

#[repr(C, packed)]
struct RecvEventFilename {
    ev: RecvEvent,
    file_name: [c_char; 260],
    flags: u32,
}

#[repr(C, packed)]
struct RecvSystemState {
    hdr: Recv,
    request_id: u32,
    integer: u32,
    float: f32,
    string: [c_char; 260],
}

#[repr(C, packed)]
struct RecvOpen {
    hdr: Recv,
    app_name: [c_char; 256],
}

#[repr(C, packed)]
struct RecvSimObjectData {
    hdr: Recv,
    request_id: u32,
    object_id: u32,
    define_id: u32,
    flags: u32,
    entry_number: u32,
    out_of: u32,
    define_count: u32,
    // data follows
}

#[repr(C, packed)]
struct RecvListTemplate {
    hdr: Recv,
    request_id: u32,
    array_size: u32,
    entry_number: u32,
    out_of: u32,
    // entries follow
}

/// Size of one SIMCONNECT_ENUMERATE_SIMOBJECT_LIVERY entry: title[256] + livery[256].
const LIVERY_ENTRY_SIZE: usize = 512;

struct Api {
    _lib: Library,
    open: FnOpen,
    close: FnClose,
    subscribe: FnSubscribe,
    request_system_state: FnRequestSystemState,
    add_to_data_definition: FnAddToDataDefinition,
    request_data_on_sim_object: FnRequestDataOnSimObject,
    get_next_dispatch: FnGetNextDispatch,
    enumerate_liveries: Option<FnEnumerateSimObjectsAndLiveries>,
}

impl Api {
    fn load(path: &Path) -> Result<Self, String> {
        unsafe {
            let lib = Library::new(path).map_err(|e| format!("load {}: {e}", path.display()))?;
            fn sym<T: Copy>(lib: &Library, name: &[u8]) -> Result<T, String> {
                unsafe {
                    let s: Symbol<T> = lib
                        .get(name)
                        .map_err(|e| format!("missing {}: {e}", String::from_utf8_lossy(name)))?;
                    Ok(*s)
                }
            }
            Ok(Api {
                open: sym(&lib, b"SimConnect_Open\0")?,
                close: sym(&lib, b"SimConnect_Close\0")?,
                subscribe: sym(&lib, b"SimConnect_SubscribeToSystemEvent\0")?,
                request_system_state: sym(&lib, b"SimConnect_RequestSystemState\0")?,
                add_to_data_definition: sym(&lib, b"SimConnect_AddToDataDefinition\0")?,
                request_data_on_sim_object: sym(&lib, b"SimConnect_RequestDataOnSimObject\0")?,
                get_next_dispatch: sym(&lib, b"SimConnect_GetNextDispatch\0")?,
                enumerate_liveries: sym(&lib, b"SimConnect_EnumerateSimObjectsAndLiveries\0").ok(),
                _lib: lib,
            })
        }
    }
}

fn cstr(buf: &[c_char]) -> String {
    let bytes: Vec<u8> = buf.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
    String::from_utf8_lossy(&bytes).trim().to_string()
}

/// Where the bundled SimConnect.dll can be, in order. The DLL ships with the app
/// (`bundle.resources` -> `<resources>/lib/SimConnect.dll`); it isn't user-configurable.
pub fn dll_candidates(resource_dir: Option<PathBuf>) -> Vec<PathBuf> {
    let mut v = Vec::new();
    // Dev builds load the source copy, so the copy the build script puts next to the
    // exe isn't locked and rebuilding while the app runs keeps working.
    if cfg!(debug_assertions) {
        v.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib").join("SimConnect.dll"));
    }
    if let Some(r) = resource_dir {
        v.push(r.join("lib").join("SimConnect.dll"));
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
        v.push(dir.join("lib").join("SimConnect.dll"));
    }
    v.dedup();
    v
}

/// `should_connect` is polled before each connection attempt (e.g. "is the sim
/// process running?"), so we don't hammer SimConnect while the sim is closed.
/// `dll_paths` is re-evaluated before each attempt so settings changes apply.
pub fn spawn(
    tx: Sender<SimEvent>,
    should_connect: impl Fn() -> bool + Send + 'static,
    dll_paths: impl Fn() -> Vec<PathBuf> + Send + 'static,
) {
    let stop2 = Arc::new(AtomicBool::new(false));
    std::thread::Builder::new()
        .name("simconnect".into())
        .spawn(move || {
            let mut last_err = String::new();
            while !stop2.load(Ordering::Relaxed) {
                if should_connect() {
                    match connect_and_run(&tx, &dll_paths(), &stop2) {
                        Ok(()) => last_err.clear(),
                        // An empty error means "not ready yet" (SimConnect_Open refused).
                        Err(e) if e.is_empty() => {}
                        Err(e) => {
                            if e != last_err {
                                let _ = tx.send(SimEvent::Error(e.clone()));
                                last_err = e;
                            }
                        }
                    }
                }
                std::thread::sleep(Duration::from_secs(3));
            }
        })
        .expect("spawn simconnect thread");
}

/// Opens a connection with the first DLL that works. Returns Ok(()) after a
/// connection ended normally (sim quit).
fn connect_and_run(tx: &Sender<SimEvent>, dlls: &[PathBuf], stop: &AtomicBool) -> Result<(), String> {
    let existing: Vec<&PathBuf> = dlls.iter().filter(|p| p.is_file()).collect();
    if existing.is_empty() {
        return Err("The bundled SimConnect.dll is missing. Reinstall MSFS Autostart.".into());
    }
    let mut errors = Vec::new();
    for path in existing {
        let api = match Api::load(path) {
            Ok(a) => a,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };
        let mut h: Handle = std::ptr::null_mut();
        let name = CString::new("MSFS Autostart").unwrap();
        let hr = unsafe { (api.open)(&mut h, name.as_ptr(), std::ptr::null_mut(), 0, std::ptr::null_mut(), 0) };
        if hr < 0 || h.is_null() {
            // Not an error: the sim refuses connections until it has finished loading.
            continue;
        }
        let result = run(&api, h, tx, path, stop);
        unsafe { (api.close)(h) };
        let _ = tx.send(SimEvent::Disconnected);
        return result;
    }
    // Open failing is normal while the sim is still starting up; only report load errors.
    Err(errors.join("; "))
}

fn run(api: &Api, h: Handle, tx: &Sender<SimEvent>, dll: &Path, stop: &AtomicBool) -> Result<(), String> {
    let c = |s: &str| CString::new(s).unwrap();
    let send = |e: SimEvent| {
        let _ = tx.send(e);
    };
    let poll_state = || unsafe {
        (api.request_system_state)(h, REQ_AIRCRAFT_STATE, c("AircraftLoaded").as_ptr());
        (api.request_system_state)(h, REQ_SIM_STATE, c("Sim").as_ptr());
        (api.request_system_state)(h, REQ_FLIGHT_STATE, c("FlightLoaded").as_ptr());
    };
    unsafe {
        for (id, name) in [
            (EV_SIM_START, "SimStart"),
            (EV_SIM_STOP, "SimStop"),
            (EV_AIRCRAFT_LOADED, "AircraftLoaded"),
            (EV_FLIGHT_LOADED, "FlightLoaded"),
            (EV_SIM, "Sim"),
        ] {
            (api.subscribe)(h, id, c(name).as_ptr());
        }
        // Separate definitions so a simvar unknown to one sim version can't break the others.
        for (def, req, var, unit, ty) in [
            (DEF_TITLE, REQ_TITLE, "TITLE", "", DATATYPE_STRING256),
            (DEF_CAMERA, REQ_CAMERA, "CAMERA STATE", "Enum", DATATYPE_INT32),
            (DEF_LIVERY, REQ_LIVERY, "LIVERY NAME", "", DATATYPE_STRING256),
        ] {
            let unit_c = c(unit);
            let unit_ptr = if unit.is_empty() { std::ptr::null() } else { unit_c.as_ptr() };
            (api.add_to_data_definition)(h, def, c(var).as_ptr(), unit_ptr, ty, 0.0, UNUSED);
            (api.request_data_on_sim_object)(h, req, def, OBJECT_ID_USER, PERIOD_SECOND, DATA_REQUEST_FLAG_CHANGED, 0, 0, 0);
        }
        if let Some(f) = api.enumerate_liveries {
            f(h, REQ_ENUM_AIRCRAFT, SIMOBJECT_TYPE_USER);
        }
    }
    poll_state();

    let mut enum_pairs: Vec<(String, String)> = Vec::new();
    let mut enum_deadline: Option<Instant> = None;
    let mut last_msg = Instant::now();
    let mut last_poll = Instant::now();
    let data_offset = std::mem::size_of::<RecvSimObjectData>();

    while !stop.load(Ordering::Relaxed) {
        let mut got_any = false;
        loop {
            let mut p: *mut Recv = std::ptr::null_mut();
            let mut size: u32 = 0;
            let hr = unsafe { (api.get_next_dispatch)(h, &mut p, &mut size) };
            if hr < 0 || p.is_null() {
                break;
            }
            got_any = true;
            last_msg = Instant::now();
            let hdr = unsafe { std::ptr::read_unaligned(p) };
            let id = hdr.id;
            match id {
                RECV_OPEN => {
                    let o = unsafe { &*(p as *const RecvOpen) };
                    send(SimEvent::Connected { app_name: cstr(&o.app_name), dll: dll.display().to_string() });
                }
                RECV_QUIT => return Ok(()),
                RECV_EXCEPTION => {
                    let ex = unsafe { std::ptr::read_unaligned((p as *const u8).add(12) as *const [u32; 3]) };
                    send(SimEvent::Debug(format!("exception {} (send id {}, index {})", ex[0], ex[1], ex[2])));
                }
                RECV_EVENT_FILENAME => {
                    let e = unsafe { &*(p as *const RecvEventFilename) };
                    let file = cstr(&e.file_name);
                    let ev = e.ev.event_id;
                    send(SimEvent::Debug(format!("event#{ev} file={file}")));
                    match ev {
                        EV_AIRCRAFT_LOADED => send(SimEvent::AircraftPath(file)),
                        EV_FLIGHT_LOADED => send(SimEvent::FlightFile(file)),
                        _ => {}
                    }
                }
                RECV_EVENT => {
                    let e = unsafe { &*(p as *const RecvEvent) };
                    let (ev, data) = (e.event_id, e.data);
                    send(SimEvent::Debug(format!("event#{ev} data={data}")));
                    match ev {
                        EV_SIM_START => send(SimEvent::SimState(true)),
                        EV_SIM_STOP => send(SimEvent::SimState(false)),
                        EV_SIM => send(SimEvent::SimState(data == 1)),
                        _ => {}
                    }
                }
                RECV_SYSTEM_STATE => {
                    let s = unsafe { &*(p as *const RecvSystemState) };
                    match s.request_id {
                        REQ_AIRCRAFT_STATE => send(SimEvent::AircraftPath(cstr(&s.string))),
                        REQ_SIM_STATE => send(SimEvent::SimState(s.integer == 1)),
                        REQ_FLIGHT_STATE => send(SimEvent::FlightFile(cstr(&s.string))),
                        _ => {}
                    }
                }
                RECV_SIMOBJECT_DATA => {
                    let d = unsafe { &*(p as *const RecvSimObjectData) };
                    let req = d.request_id;
                    let avail = (size as usize).saturating_sub(data_offset);
                    let base = unsafe { (p as *const u8).add(data_offset) };
                    match req {
                        REQ_TITLE | REQ_LIVERY if avail >= 256 => {
                            let s = cstr(unsafe { std::slice::from_raw_parts(base as *const c_char, 256) });
                            send(if req == REQ_TITLE { SimEvent::AircraftTitle(s) } else { SimEvent::LiveryName(s) });
                        }
                        REQ_CAMERA if avail >= 4 => {
                            send(SimEvent::Camera(unsafe { std::ptr::read_unaligned(base as *const i32) }));
                        }
                        _ => {}
                    }
                }
                _ => {
                    // The 2024 livery list's numeric RECV id isn't fixed across SDK headers,
                    // so recognise it by request id + exact size.
                    if let Some(chunk) = parse_livery_list(p, size) {
                        enum_pairs.extend(chunk.pairs);
                        enum_deadline = Some(Instant::now() + Duration::from_secs(2));
                        if chunk.last {
                            send(SimEvent::AircraftList(std::mem::take(&mut enum_pairs)));
                            enum_deadline = None;
                        }
                    } else {
                        let words = unsafe {
                            std::ptr::read_unaligned((p as *const u8).add(12) as *const [u32; 6])
                        };
                        send(SimEvent::Debug(format!("unhandled recv id {id} size {size} words {words:?}")));
                    }
                }
            }
        }
        if enum_deadline.is_some_and(|d| Instant::now() > d) {
            send(SimEvent::AircraftList(std::mem::take(&mut enum_pairs)));
            enum_deadline = None;
        }
        // Polling catches missed events and doubles as a heartbeat.
        if last_poll.elapsed() > Duration::from_secs(3) {
            poll_state();
            last_poll = Instant::now();
        }
        if !got_any {
            if last_msg.elapsed() > Duration::from_secs(20) {
                return Err("SimConnect stopped responding".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    Ok(())
}

struct LiveryChunk {
    pairs: Vec<(String, String)>,
    last: bool,
}

fn parse_livery_list(p: *const Recv, size: u32) -> Option<LiveryChunk> {
    let hdr_size = std::mem::size_of::<RecvListTemplate>();
    if (size as usize) < hdr_size {
        return None;
    }
    let l = unsafe { std::ptr::read_unaligned(p as *const RecvListTemplate) };
    if l.request_id != REQ_ENUM_AIRCRAFT {
        return None;
    }
    let n = l.array_size as usize;
    if n == 0 || (size as usize) < hdr_size + n * LIVERY_ENTRY_SIZE {
        return None;
    }
    let base = unsafe { (p as *const u8).add(hdr_size) };
    let mut pairs = Vec::with_capacity(n);
    for i in 0..n {
        unsafe {
            let e = base.add(i * LIVERY_ENTRY_SIZE);
            let title = CStr::from_ptr(e as *const c_char).to_string_lossy().to_string();
            let livery = CStr::from_ptr(e.add(256) as *const c_char).to_string_lossy().to_string();
            pairs.push((title, livery));
        }
    }
    Some(LiveryChunk { pairs, last: l.entry_number + 1 >= l.out_of })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs src-tauri/lib/SimConnect.dll: `cargo test load_dll -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn load_dll() {
        let path = dll_candidates(None).into_iter().find(|p| p.is_file()).expect("no SimConnect.dll found");
        let api = Api::load(&path).expect("load SimConnect.dll");
        println!("loaded {} (2024 enumerate: {})", path.display(), api.enumerate_liveries.is_some());
    }

    #[test]
    fn struct_layout() {
        assert_eq!(std::mem::size_of::<Recv>(), 12);
        assert_eq!(std::mem::size_of::<RecvEvent>(), 24);
        assert_eq!(std::mem::size_of::<RecvSystemState>(), 12 + 12 + 260);
        assert_eq!(std::mem::size_of::<RecvSimObjectData>(), 40);
        assert_eq!(std::mem::size_of::<RecvListTemplate>(), 28);
    }
}
