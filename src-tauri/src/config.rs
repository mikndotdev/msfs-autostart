//! Persistent user configuration (rules, programs, settings), stored as JSON.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
#[derive(Default)]
pub struct Config {
    pub rules: Vec<Rule>,
    pub settings: Settings,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Don't launch anything new while paused.
    pub paused: bool,
    /// Seconds to wait after a trigger ends before closing programs. Rides out flight
    /// restarts and short SimStop/SimStart blips.
    pub stop_grace_secs: u64,
    /// How long a graceful close (WM_CLOSE) may take before the process is killed.
    pub close_timeout_secs: u64,
    /// The setup wizard has been completed (it opens on launch until it is).
    pub setup_complete: bool,
    /// Packages folders added by hand in setup, scanned along with detected installs.
    pub extra_package_paths: Vec<ExtraPackagePath>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtraPackagePath {
    pub sim: crate::catalog::Sim,
    pub path: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            paused: false,
            stop_grace_secs: 10,
            close_timeout_secs: 10,
            setup_complete: false,
            extra_package_paths: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub trigger: Trigger,
    pub programs: Vec<Program>,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: String::new(),
            enabled: true,
            trigger: Trigger::Sim { aircraft: Vec::new(), patterns: Vec::new() },
            programs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Trigger {
    /// Active while MSFS is running. The aircraft selection is kept (unused) so
    /// switching the trigger back to `Aircraft` in the UI doesn't lose it.
    #[serde(rename_all = "camelCase")]
    Sim {
        #[serde(default)]
        aircraft: Vec<AircraftRef>,
        #[serde(default)]
        patterns: Vec<String>,
    },
    /// Active while in a flight with one of these aircraft loaded.
    #[serde(rename_all = "camelCase")]
    Aircraft {
        #[serde(default)]
        aircraft: Vec<AircraftRef>,
        /// Extra wildcard patterns (`*`, `?`, case-insensitive) matched against the
        /// aircraft title and the aircraft.cfg path.
        #[serde(default)]
        patterns: Vec<String>,
    },
}

/// Snapshot of a catalog entry, kept in the rule so it still matches if the
/// catalog is stale or the aircraft was removed and reinstalled.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AircraftRef {
    pub id: String,
    pub name: String,
    pub folders: Vec<String>,
    pub titles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Program {
    pub id: String,
    pub exe: String,
    /// Raw argument string, passed through to the process unchanged.
    pub args: String,
    /// Working directory; defaults to the exe's folder.
    pub cwd: String,
    /// Close the program when the trigger stops being active.
    pub close_on_stop: bool,
    pub close_mode: CloseMode,
    pub start_delay_secs: u64,
    /// Launch elevated (shows a UAC prompt).
    pub run_as_admin: bool,
}

impl Default for Program {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            exe: String::new(),
            args: String::new(),
            cwd: String::new(),
            close_on_stop: true,
            close_mode: CloseMode::Graceful,
            start_delay_secs: 0,
            run_as_admin: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CloseMode {
    /// Ask windows to close, kill after the timeout.
    Graceful,
    /// Terminate immediately.
    Kill,
}

pub fn config_path(dir: &Path) -> PathBuf {
    dir.join("config.json")
}

pub fn load(dir: &Path) -> Config {
    match std::fs::read_to_string(config_path(dir)) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            eprintln!("config parse error, using defaults: {e}");
            Config::default()
        }),
        Err(_) => Config::default(),
    }
}

pub fn save(dir: &Path, config: &Config) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join("config.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(config)?)?;
    std::fs::rename(tmp, config_path(dir))
}

/// Case-insensitive wildcard match supporting `*` and `?`.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard() {
        assert!(wildcard_match("*pmdg 737*", "PMDG 737-800 House"));
        assert!(wildcard_match("a?c", "ABC"));
        assert!(!wildcard_match("*777*", "PMDG 737-800"));
        assert!(wildcard_match("*", ""));
    }

    #[test]
    fn sim_trigger_keeps_aircraft_and_reads_old_configs() {
        // Configs written before the sim trigger kept a selection.
        let old: Trigger = serde_json::from_str(r#"{"type":"sim"}"#).unwrap();
        assert_eq!(old, Trigger::Sim { aircraft: vec![], patterns: vec![] });

        let kept = Trigger::Sim {
            aircraft: vec![AircraftRef { id: "2024:x".into(), ..Default::default() }],
            patterns: vec!["*md-11*".into()],
        };
        let back: Trigger = serde_json::from_str(&serde_json::to_string(&kept).unwrap()).unwrap();
        assert_eq!(back, kept);
    }

    #[test]
    fn trigger_json_roundtrip() {
        let r = Rule {
            trigger: Trigger::Aircraft { aircraft: vec![], patterns: vec!["*fenix*".into()] },
            ..Default::default()
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains(r#""type":"aircraft""#));
        let back: Rule = serde_json::from_str(&s).unwrap();
        assert_eq!(back, r);
    }
}
