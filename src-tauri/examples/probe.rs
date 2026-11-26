//! Prints every SimConnect signal the app sees, with timestamps.
//! `cargo run --example probe`
use msfs_autostart_lib::sim::{self, SimEvent};
use std::sync::mpsc::channel;
use std::time::Instant;

fn main() {
    let (tx, rx) = channel();
    sim::spawn(tx, || true, || sim::dll_candidates(None));
    let t0 = Instant::now();
    let mut last_camera = None;
    for ev in rx {
        let t = t0.elapsed().as_secs_f32();
        match &ev {
            // Camera is polled with CHANGED, but dedupe anyway; skip noisy duplicates of state polls.
            SimEvent::Camera(c) => {
                if last_camera == Some(*c) { continue; }
                last_camera = Some(*c);
            }
            SimEvent::AircraftList(v) => {
                println!("[{t:7.1}] AircraftList: {} entries", v.len());
                if let Ok(p) = std::env::var("PROBE_LIST") {
                    let _ = std::fs::write(p, v.iter().map(|(a, b)| format!("{a}	{b}
")).collect::<String>());
                }
                continue;
            }
            _ => {}
        }
        println!("[{t:7.1}] {ev:?}");
    }
}
