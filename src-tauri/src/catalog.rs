//! Installed aircraft catalog: scans MSFS 2020/2024 package folders for
//! `SimObjects/Airplanes/*/aircraft.cfg` and merges the live list that
//! MSFS 2024's SimConnect can enumerate.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Sim {
    #[serde(rename = "2020")]
    Msfs2020,
    #[serde(rename = "2024")]
    Msfs2024,
}

impl Sim {
    fn tag(self) -> &'static str {
        match self {
            Sim::Msfs2020 => "2020",
            Sim::Msfs2024 => "2024",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub aircraft: Vec<AircraftEntry>,
    /// Package roots that were scanned, for display/debugging.
    pub roots: Vec<String>,
    pub scanned_at: u64,
    /// Lower-case titles of AI-only variations (isAirTraffic / not user-selectable),
    /// used to filter AI traffic out of SimConnect's aircraft list.
    #[serde(default)]
    pub ai_titles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AircraftEntry {
    /// `"<sim>:<base folder>"` (lower-case) or `"<sim>:title:<title>"` for SimConnect-only entries.
    pub id: String,
    pub sim: Sim,
    pub manufacturer: String,
    pub name: String,
    pub package: String,
    /// All SimObject folder names (base + livery packages) belonging to this aircraft.
    pub folders: Vec<String>,
    pub liveries: Vec<Livery>,
    /// Where the entry came from: "files", "simconnect" or both.
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Livery {
    pub title: String,
    pub thumbnail: Option<String>,
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Locating installs

/// Where each store version keeps `UserCfg.opt`, labelled for the UI.
fn usercfg_candidates(sim: Sim) -> Vec<(&'static str, PathBuf)> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let roaming = std::env::var_os("APPDATA").map(PathBuf::from);
    let (store_pkg, steam_dir) = match sim {
        Sim::Msfs2020 => ("Microsoft.FlightSimulator_8wekyb3d8bbwe", "Microsoft Flight Simulator"),
        Sim::Msfs2024 => ("Microsoft.Limitless_8wekyb3d8bbwe", "Microsoft Flight Simulator 2024"),
    };
    let mut v = Vec::new();
    if let Some(l) = &local {
        v.push(("Microsoft Store", l.join("Packages").join(store_pkg).join("LocalCache").join("UserCfg.opt")));
    }
    if let Some(r) = &roaming {
        v.push(("Steam", r.join(steam_dir).join("UserCfg.opt")));
    }
    v
}

/// An MSFS install found on this PC.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Install {
    pub sim: Sim,
    /// "Steam" or "Microsoft Store".
    pub source: String,
    /// `InstalledPackagesPath` from UserCfg.opt.
    pub packages_path: String,
    #[serde(flatten)]
    pub check: PathCheck,
}

/// What a packages folder contains.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PathCheck {
    pub exists: bool,
    pub has_community: bool,
    pub has_official: bool,
}

pub fn check_packages_path(path: &Path) -> PathCheck {
    let mut c = PathCheck { exists: path.is_dir(), ..Default::default() };
    if let Ok(entries) = std::fs::read_dir(path) {
        for e in entries.flatten().filter(|e| e.path().is_dir()) {
            let name = e.file_name().to_string_lossy().to_lowercase();
            c.has_community |= name.starts_with("community");
            c.has_official |= name.starts_with("official") || name == "streamedpackages";
        }
    }
    c
}

/// Every install listed in a `UserCfg.opt`, for both sims and both stores.
pub fn detect_installs() -> Vec<Install> {
    let mut out: Vec<Install> = Vec::new();
    for sim in [Sim::Msfs2020, Sim::Msfs2024] {
        for (source, cfg) in usercfg_candidates(sim) {
            let Ok(text) = std::fs::read(&cfg) else { continue };
            let text = String::from_utf8_lossy(&text);
            for line in text.lines() {
                let Some(rest) = line.trim().strip_prefix("InstalledPackagesPath") else { continue };
                let p = rest.trim().trim_matches('"');
                if p.is_empty() || out.iter().any(|i| i.sim == sim && i.packages_path.eq_ignore_ascii_case(p)) {
                    continue;
                }
                out.push(Install {
                    sim,
                    source: source.to_string(),
                    packages_path: p.to_string(),
                    check: check_packages_path(Path::new(p)),
                });
            }
        }
    }
    out
}

/// Package directories (folders that contain `SimObjects`) under a packages path.
/// Handles `Community`, `Official\Steam\*`, `Official2024\*`, `StreamedPackages`, etc.
fn package_dirs(packages_path: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(top) = std::fs::read_dir(packages_path) else { return out };
    for top in top.flatten() {
        let top = top.path();
        if !top.is_dir() {
            continue;
        }
        // Depth 1 (Community\pkg) and depth 2 (Official\Steam\pkg).
        let Ok(children) = std::fs::read_dir(&top) else { continue };
        for child in children.flatten() {
            let child = child.path();
            if child.join("SimObjects").is_dir() {
                out.push(child);
            } else if child.is_dir()
                && let Ok(grand) = std::fs::read_dir(&child) {
                    for g in grand.flatten() {
                        let g = g.path();
                        if g.join("SimObjects").is_dir() {
                            out.push(g);
                        }
                    }
                }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// aircraft.cfg parsing

type Ini = Vec<(String, Vec<(String, String)>)>;

/// Minimal, forgiving INI parser: lower-cased section/key names, `;`/`//` comments
/// outside quotes, surrounding quotes stripped.
pub fn parse_ini(text: &str) -> Ini {
    let mut out: Ini = Vec::new();
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            if let Some(end) = line.find(']') {
                out.push((line[1..end].trim().to_lowercase(), Vec::new()));
            }
            continue;
        }
        if let Some((k, v)) = line.split_once('=')
            && let Some((_, kv)) = out.last_mut() {
                kv.push((k.trim().to_lowercase(), v.trim().trim_matches('"').trim().to_string()));
            }
    }
    out
}

fn strip_comment(line: &str) -> &str {
    let mut in_quotes = false;
    let bytes = line.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'"' => in_quotes = !in_quotes,
            b';' if !in_quotes => return &line[..i],
            b'/' if !in_quotes && bytes.get(i + 1) == Some(&b'/') => return &line[..i],
            _ => {}
        }
    }
    line
}

fn get<'a>(kv: &'a [(String, String)], key: &str) -> Option<&'a str> {
    kv.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// A localisation token like `TT:AIRCRAFT.X` or `$$:Generic` is useless as a name.
fn usable(s: Option<&str>) -> Option<String> {
    let s = s?.trim();
    if s.is_empty() || s.starts_with("TT:") || s.starts_with("$$") {
        None
    } else {
        Some(s.to_string())
    }
}

struct ParsedCfg {
    folder: String,
    base: String,
    manufacturer: Option<String>,
    ui_type: Option<String>,
    liveries: Vec<Livery>,
    ai_titles: Vec<String>,
}

fn read_ini(path: &Path) -> Option<Ini> {
    let bytes = std::fs::read(path).ok()?;
    Some(parse_ini(String::from_utf8_lossy(&bytes).trim_start_matches('\u{feff}')))
}

/// Parses a legacy `aircraft.cfg` or an MSFS 2024 preset's `config/aircraft.cfg`.
/// Returns `None` only when the file can't be read.
fn parse_aircraft_cfg(cfg_path: &Path) -> Option<ParsedCfg> {
    let bytes = std::fs::read(cfg_path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let ini = parse_ini(text.trim_start_matches('\u{feff}'));
    let dir = cfg_path.parent()?;
    let folder = dir.file_name()?.to_string_lossy().to_string();

    let base = ini
        .iter()
        .find(|(s, _)| s == "variation")
        .and_then(|(_, kv)| get(kv, "base_container"))
        .and_then(|b| {
            b.replace('/', "\\").trim_end_matches('\\').rsplit('\\').next().map(|s| s.to_string())
        })
        .filter(|s| !s.is_empty() && s != "..")
        .unwrap_or_else(|| folder.clone());

    let mut manufacturer = None;
    let mut ui_type = None;
    let mut liveries = Vec::new();
    let mut ai_titles = Vec::new();
    for (section, kv) in &ini {
        if !section.starts_with("fltsim.") {
            continue;
        }
        let flag = |k: &str| get(kv, k).map(|v| v.trim().starts_with('1'));
        if flag("isuserselectable") == Some(false) || flag("isairtraffic") == Some(true) {
            if let Some(t) = get(kv, "title").filter(|t| !t.is_empty()) {
                ai_titles.push(t.to_lowercase());
            }
            continue;
        }
        let Some(title) = usable(get(kv, "title")) else { continue };
        if manufacturer.is_none() {
            manufacturer = usable(get(kv, "ui_manufacturer"));
        }
        if ui_type.is_none() {
            ui_type = usable(get(kv, "ui_type"));
        }
        liveries.push(Livery { title, thumbnail: find_thumbnail(dir, kv) });
    }
    // Stock aircraft use localisation tokens; fall back to [GENERAL] icao_manufacturer.
    if manufacturer.is_none() {
        manufacturer = general_manufacturer(&ini);
    }
    Some(ParsedCfg { folder, base, manufacturer, ui_type, liveries, ai_titles })
}

fn general_manufacturer(ini: &Ini) -> Option<String> {
    let general = ini.iter().find(|(s, _)| s == "general").map(|(_, kv)| kv.as_slice()).unwrap_or(&[]);
    usable(get(general, "icao_manufacturer")).map(|m| title_case(&m))
}

/// Sub-directories two levels down: `<dir>/<creator>/<name>` (MSFS 2024 presets/liveries).
fn two_level_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(creators) = std::fs::read_dir(dir) else { return out };
    for c in creators.flatten() {
        if let Ok(items) = std::fs::read_dir(c.path()) {
            out.extend(items.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
        }
    }
    out.sort();
    out
}

/// MSFS 2024 `liveries/<creator>/<name>/livery.cfg` -> display name and thumbnail.
fn parse_livery(dir: &Path) -> Option<Livery> {
    let ini = read_ini(&dir.join("livery.cfg"))?;
    let section = |name: &str| ini.iter().find(|(s, _)| s == name).map(|(_, kv)| kv.as_slice()).unwrap_or(&[]);
    let fallback = dir.file_name()?.to_string_lossy().to_string();
    let title = usable(get(section("general"), "name"))
        .or_else(|| usable(get(section("fltsim"), "title")))
        .or_else(|| usable(get(section("fltsim"), "atc_id")))
        .unwrap_or(fallback);
    Some(Livery { title, thumbnail: thumbnail_in(&dir.join("thumbnail")) })
}

fn thumbnail_in(dir: &Path) -> Option<String> {
    ["thumbnail.jpg", "thumbnail.png", "thumbnail_small.jpg", "thumbnail_small.png"]
        .iter()
        .map(|n| dir.join(n))
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().to_string())
}

/// "MD-11 GE", "MD-11 PW", "MD-11F GE" -> "MD-11"
fn common_name(names: &[String]) -> Option<String> {
    let first = names.first()?;
    if names.iter().all(|n| n == first) {
        return Some(first.clone());
    }
    let mut prefix: Vec<char> = first.chars().collect();
    for n in &names[1..] {
        let len = prefix.iter().zip(n.chars()).take_while(|(a, b)| **a == *b).count();
        prefix.truncate(len);
    }
    let s: String = prefix.into_iter().collect();
    // If the prefix ends mid-word ("Draco X: Pa"), cut back to the last word boundary.
    let mid_word = s.chars().last().is_some_and(char::is_alphanumeric)
        && first[s.len()..].chars().next().is_some_and(char::is_alphanumeric);
    let s = match s.rfind([' ', '|', '(', '/']) {
        Some(i) if mid_word => s[..i].to_string(),
        _ => s,
    };
    let s = s.trim_end_matches([' ', '-', '|', '(', '/', ',', ':']).trim().to_string();
    (s.chars().count() >= 3).then_some(s)
}

/// "Asobo_Baron_G58" -> "Baron G58"
fn pretty_folder(s: &str) -> String {
    let s = s.strip_prefix("Asobo_").or_else(|| s.strip_prefix("Microsoft_")).unwrap_or(s);
    s.replace(['_', '-'], " ").trim().to_string()
}

/// "CESSNA" -> "Cessna"; leaves mixed-case input alone.
fn title_case(s: &str) -> String {
    if s.chars().any(|c| c.is_lowercase()) {
        return s.to_string();
    }
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c.flat_map(|x| x.to_lowercase())).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn find_thumbnail(dir: &Path, kv: &[(String, String)]) -> Option<String> {
    let mut candidates = Vec::new();
    if let Some(t) = get(kv, "ui_thumbnailfile").filter(|s| !s.is_empty()) {
        candidates.push(dir.join(t));
    }
    let tex = get(kv, "texture").unwrap_or("");
    let tex_dir = if tex.is_empty() { dir.join("texture") } else { dir.join(format!("texture.{tex}")) };
    // MSFS 2024 presets keep it in `<preset>/thumbnail/`, next to `<preset>/config/`.
    let preset_thumb = dir.parent().map(|p| p.join("thumbnail"));
    for d in [Some(tex_dir), preset_thumb].into_iter().flatten() {
        for name in ["thumbnail.jpg", "thumbnail.png", "thumbnail_small.jpg"] {
            candidates.push(d.join(name));
        }
    }
    candidates.into_iter().find(|p| p.is_file()).map(|p| p.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------
// Scanning

/// Scans all installs of both sims. Takes a few seconds on large Community folders.
///
/// Handles both layouts:
/// - legacy: `SimObjects/Airplanes/<A>/aircraft.cfg` (livery packages point at their
///   base aircraft with `[VARIATION] base_container`)
/// - MSFS 2024 modular: `SimObjects/Airplanes/<A>/presets/<creator>/<preset>/config/aircraft.cfg`
///   plus `SimObjects/Airplanes/<A>/liveries/<creator>/<livery>/livery.cfg`, often in
///   separate packages that share the `<A>` folder name.
pub fn scan(extra: &[(Sim, PathBuf)]) -> Catalog {
    let mut roots = Vec::new();
    // (sim, base folder lower) -> entry
    let mut groups: BTreeMap<(Sim, String), AircraftEntry> = BTreeMap::new();
    // Name info from the base aircraft's own cfg wins over livery packages.
    let mut base_named: HashSet<(Sim, String)> = HashSet::new();
    // Preset ui_types per modular group, used to derive a common name.
    let mut preset_types: HashMap<(Sim, String), Vec<String>> = HashMap::new();
    let mut ai_titles: HashSet<String> = HashSet::new();

    fn entry<'a>(
        groups: &'a mut BTreeMap<(Sim, String), AircraftEntry>,
        sim: Sim,
        base: &str,
        package: &str,
    ) -> &'a mut AircraftEntry {
        let key = (sim, base.to_lowercase());
        groups.entry(key.clone()).or_insert_with(|| AircraftEntry {
            id: format!("{}:{}", sim.tag(), key.1),
            sim,
            manufacturer: String::new(),
            name: String::new(),
            package: package.to_string(),
            folders: Vec::new(),
            liveries: Vec::new(),
            source: "files".into(),
        })
    }
    fn add_folder(e: &mut AircraftEntry, folder: &str) {
        if !e.folders.iter().any(|f| f.eq_ignore_ascii_case(folder)) {
            e.folders.push(folder.to_string());
        }
    }
    fn add_livery(e: &mut AircraftEntry, l: Livery) {
        if !e.liveries.iter().any(|x| x.title == l.title) {
            e.liveries.push(l);
        }
    }

    // Detected installs plus folders the user added in setup.
    let mut sources: Vec<(Sim, PathBuf)> = detect_installs()
        .into_iter()
        .filter(|i| i.check.exists)
        .map(|i| (i.sim, PathBuf::from(i.packages_path)))
        .collect();
    for (sim, p) in extra {
        if p.is_dir() && !sources.iter().any(|(s2, q)| s2 == sim && same_dir(q, p)) {
            sources.push((*sim, p.clone()));
        }
    }

    for (sim, pp) in sources {
        roots.push(format!("{}: {}", sim.tag(), pp.display()));
        for pkg in package_dirs(&pp) {
            let package = pkg.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let Ok(planes) = std::fs::read_dir(pkg.join("SimObjects").join("Airplanes")) else { continue };
            for plane in planes.flatten() {
                let dir = plane.path();
                let Some(folder) = dir.file_name().map(|s| s.to_string_lossy().to_string()) else { continue };

                // Legacy layout.
                if let Some(parsed) = parse_aircraft_cfg(&dir.join("aircraft.cfg")) {
                    ai_titles.extend(parsed.ai_titles);
                    if !parsed.liveries.is_empty() {
                        let key = (sim, parsed.base.to_lowercase());
                        let is_base = parsed.folder.eq_ignore_ascii_case(&parsed.base);
                        let e = entry(&mut groups, sim, &parsed.base, &package);
                        if (is_base && base_named.insert(key)) || e.name.is_empty() {
                            if let Some(m) = parsed.manufacturer {
                                e.manufacturer = m;
                            }
                            e.name = parsed.ui_type.unwrap_or_else(|| pretty_folder(&parsed.base));
                            if is_base {
                                e.package = package.clone();
                            }
                        }
                        add_folder(e, &parsed.folder);
                        for l in parsed.liveries {
                            add_livery(e, l);
                        }
                    }
                }

                // MSFS 2024 presets.
                for preset in two_level_dirs(&dir.join("presets")) {
                    let Some(parsed) = parse_aircraft_cfg(&preset.join("config").join("aircraft.cfg")) else {
                        continue;
                    };
                    ai_titles.extend(parsed.ai_titles);
                    if parsed.liveries.is_empty() {
                        continue;
                    }
                    let key = (sim, folder.to_lowercase());
                    if let Some(t) = &parsed.ui_type {
                        preset_types.entry(key.clone()).or_default().push(t.clone());
                    }
                    let e = entry(&mut groups, sim, &folder, &package);
                    e.package = package.clone();
                    if e.manufacturer.is_empty()
                        && let Some(m) = parsed.manufacturer.clone().or_else(|| {
                            read_ini(&dir.join("common").join("config").join("aircraft.cfg"))
                                .and_then(|ini| general_manufacturer(&ini))
                        }) {
                            e.manufacturer = m;
                        }
                    add_folder(e, &folder);
                    for l in parsed.liveries {
                        add_livery(e, l);
                    }
                }

                // MSFS 2024 liveries (often shipped in their own packages).
                for livery_dir in two_level_dirs(&dir.join("liveries")) {
                    if let Some(l) = parse_livery(&livery_dir) {
                        let e = entry(&mut groups, sim, &folder, &package);
                        add_folder(e, &folder);
                        add_livery(e, l);
                    }
                }
            }
        }
    }

    for (key, e) in groups.iter_mut() {
        if let Some(types) = preset_types.get(key)
            && let Some(n) = common_name(types) {
                e.name = n;
            }
        if e.name.is_empty() {
            e.name = pretty_folder(&e.folders.first().cloned().unwrap_or_else(|| key.1.clone()));
        }
        // Thumbnails first so the picker card shows one.
        e.liveries.sort_by_key(|l| l.thumbnail.is_none());
    }

    let mut aircraft: Vec<AircraftEntry> = groups.into_values().collect();
    sort_entries(&mut aircraft);
    let mut ai_titles: Vec<String> = ai_titles.into_iter().collect();
    ai_titles.sort();
    Catalog { aircraft, roots, scanned_at: now_secs(), ai_titles }
}

fn same_dir(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase();
    norm(a) == norm(b)
}

fn sort_entries(v: &mut [AircraftEntry]) {
    v.sort_by(|a, b| {
        (a.manufacturer.to_lowercase(), a.name.to_lowercase(), a.sim)
            .cmp(&(b.manufacturer.to_lowercase(), b.name.to_lowercase(), b.sim))
    });
}

/// SimConnect's list also contains AI traffic models; skip those.
fn is_ai_title(title_lower: &str, ai: &HashSet<&str>) -> bool {
    ai.contains(title_lower)
        || title_lower.starts_with("fsltl")
        || title_lower.contains("passive")
        || title_lower.ends_with("-stub")
        || title_lower.ends_with(" stub")
}

/// Merges MSFS 2024's SimConnect enumeration (`(aircraft title, livery name)` pairs).
/// Titles already known from the file scan and AI traffic are skipped; the rest
/// become new entries. Returns how many entries were added.
pub fn merge_simconnect(catalog: &mut Catalog, pairs: &[(String, String)]) -> usize {
    let known: HashSet<String> = catalog
        .aircraft
        .iter()
        .filter(|a| a.sim == Sim::Msfs2024)
        .flat_map(|a| a.liveries.iter().map(|l| l.title.to_lowercase()))
        .collect();
    let ai: HashSet<&str> = catalog.ai_titles.iter().map(|s| s.as_str()).collect();
    let mut by_title: BTreeMap<String, AircraftEntry> = BTreeMap::new();
    for (title, livery) in pairs {
        let title = title.trim();
        let lower = title.to_lowercase();
        if title.is_empty() || known.contains(&lower) || is_ai_title(&lower, &ai) {
            continue;
        }
        let id = format!("2024:title:{}", title.to_lowercase());
        if catalog.aircraft.iter().any(|a| a.id == id) {
            continue;
        }
        let e = by_title.entry(title.to_lowercase()).or_insert_with(|| AircraftEntry {
            id,
            sim: Sim::Msfs2024,
            manufacturer: String::new(),
            name: title.to_string(),
            package: String::new(),
            folders: Vec::new(),
            liveries: vec![Livery { title: title.to_string(), thumbnail: None }],
            source: "simconnect".into(),
        });
        let livery = livery.trim();
        if !livery.is_empty() && !e.liveries.iter().any(|l| l.title == livery) {
            e.liveries.push(Livery { title: livery.to_string(), thumbnail: None });
        }
    }
    let added = by_title.len();
    if added > 0 {
        catalog.aircraft.extend(by_title.into_values());
        sort_entries(&mut catalog.aircraft);
    }
    added
}

// ---------------------------------------------------------------------------
// Lookup index used for rule matching

#[derive(Default)]
pub struct Index {
    by_folder: HashMap<String, Vec<String>>,
    by_title: HashMap<String, Vec<String>>,
}

impl Index {
    pub fn build(c: &Catalog) -> Self {
        let mut idx = Index::default();
        for a in &c.aircraft {
            for f in &a.folders {
                idx.by_folder.entry(f.to_lowercase()).or_default().push(a.id.clone());
            }
            for l in &a.liveries {
                idx.by_title.entry(l.title.to_lowercase()).or_default().push(a.id.clone());
            }
        }
        idx
    }

    /// Catalog ids the currently loaded aircraft belongs to.
    pub fn ids_for(&self, folder: Option<&str>, title: Option<&str>) -> HashSet<String> {
        let mut out = HashSet::new();
        if let Some(v) = folder.and_then(|f| self.by_folder.get(&f.to_lowercase())) {
            out.extend(v.iter().cloned());
        }
        if let Some(v) = title.and_then(|t| self.by_title.get(&t.to_lowercase())) {
            out.extend(v.iter().cloned());
        }
        out
    }
}

/// SimObject folder name from an `AircraftLoaded` path. Examples:
/// - `SimObjects\Airplanes\PMDG 737-800\aircraft.CFG` -> `PMDG 737-800`
/// - `SimObjects\Airplanes\TFDi_Design_MD-11\presets\tfdi-design\TFDi_Design_MD-11 GE\config\aircraft.CFG`
///   -> `TFDi_Design_MD-11` (MSFS 2024)
/// - `x:\msfs2024\community2024\pkg\simobjects\airplanes\tfdi_design_md-11\presets\...` (MSFS 2024, absolute)
pub fn folder_from_loaded_path(path: &str) -> Option<String> {
    let p = path.replace('/', "\\");
    let parts: Vec<&str> = p.split('\\').filter(|s| !s.is_empty()).collect();
    // The folder right after SimObjects\<category>.
    if let Some(i) = parts.iter().rposition(|s| s.eq_ignore_ascii_case("simobjects"))
        && let Some(f) = parts.get(i + 2)
            && (!f.contains('.') || parts.len() > i + 3) {
                return Some(f.to_string());
            }
    let mut parts = parts;
    if parts.last().is_some_and(|l| l.to_lowercase().ends_with(".cfg") || l.to_lowercase().ends_with(".air")) {
        parts.pop();
    }
    parts.last().map(|s| s.to_string())
}

pub fn cache_path(dir: &Path) -> PathBuf {
    dir.join("catalog.json")
}

pub fn load_cache(dir: &Path) -> Option<Catalog> {
    serde_json::from_slice(&std::fs::read(cache_path(dir)).ok()?).ok()
}

pub fn save_cache(dir: &Path, c: &Catalog) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let bytes = serde_json::to_vec(c).map_err(|e| format!("serialize catalog: {e}"))?;
    let path = cache_path(dir);
    std::fs::write(&path, bytes).map_err(|e| format!("write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ini_parsing() {
        let ini = parse_ini(
            "[VARIATION]\nbase_container =\"..\\inibuilds-aircraft-a306r\" ; c\n[FLTSIM.0]\nTitle=\"A300; GE\" // x\nisUserSelectable=1\n",
        );
        assert_eq!(get(&ini[0].1, "base_container"), Some("..\\inibuilds-aircraft-a306r"));
        assert_eq!(get(&ini[1].1, "title"), Some("A300; GE"));
    }

    #[test]
    fn loaded_path_folder() {
        assert_eq!(
            folder_from_loaded_path("SimObjects\\Airplanes\\PMDG 737-800\\aircraft.CFG").as_deref(),
            Some("PMDG 737-800")
        );
        assert_eq!(folder_from_loaded_path("SimObjects/Airplanes/C172/").as_deref(), Some("C172"));
        // MSFS 2024 paths seen from SimConnect.
        assert_eq!(
            folder_from_loaded_path(
                "SimObjects\\Airplanes\\TFDi_Design_MD-11\\presets\\tfdi-design\\TFDi_Design_MD-11 GE\\config\\aircraft.CFG"
            )
            .as_deref(),
            Some("TFDi_Design_MD-11")
        );
        assert_eq!(
            folder_from_loaded_path(
                "x:\\msfs2024\\community2024\\tfdidesign-aircraft-md11\\simobjects\\airplanes\\tfdi_design_md-11\\presets\\tfdi-design\\tfdi_design_md-11 ge\\config\\aircraft.cfg"
            )
            .as_deref(),
            Some("tfdi_design_md-11")
        );
    }

    #[test]
    fn common_names() {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(common_name(&v(&["MD-11 GE", "MD-11 PW", "MD-11F GE"])).as_deref(), Some("MD-11"));
        assert_eq!(common_name(&v(&["737-800"])).as_deref(), Some("737-800"));
        assert_eq!(common_name(&v(&["Draco X: Passengers", "Draco X: Cargo"])).as_deref(), Some("Draco X"));
        assert_eq!(common_name(&v(&["A", "B"])), None);
    }

    /// `cargo test detect_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn detect_real() {
        for i in detect_installs() {
            println!("{:?}", i);
        }
        println!("{:?}", check_packages_path(Path::new(r"C:\Windows")));
        println!("scan: {} aircraft", scan(&[]).aircraft.len());
    }

    #[test]
    #[ignore]
    fn cache_roundtrip_real() {
        let dir = std::env::temp_dir().join("msfs-autostart-cache-test");
        let _ = std::fs::remove_dir_all(&dir);
        let c = scan(&[]);
        save_cache(&dir, &c).unwrap();
        let back = load_cache(&dir).expect("cache should load");
        println!("saved {} / loaded {} aircraft, {} ai titles", c.aircraft.len(), back.aircraft.len(), back.ai_titles.len());
        assert_eq!(c.aircraft.len(), back.aircraft.len());
    }

    /// Runs against the real installs on this machine: `cargo test scan_real -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn scan_real() {
        let t = std::time::Instant::now();
        let c = scan(&[]);
        println!("roots: {:#?}", c.roots);
        println!("{} aircraft in {:?}", c.aircraft.len(), t.elapsed());
        for a in c.aircraft.iter().take(400) {
            println!(
                "[{}] {} | {} | {} liveries | thumb: {} | folders: {:?}",
                a.sim.tag(),
                a.manufacturer,
                a.name,
                a.liveries.len(),
                a.liveries.iter().filter(|l| l.thumbnail.is_some()).count(),
                a.folders.iter().take(3).collect::<Vec<_>>()
            );
        }
    }
}
