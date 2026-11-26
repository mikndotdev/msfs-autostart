import { invoke, isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { makeMockApi } from "./mock";

export type CloseMode = "graceful" | "kill";

export interface Program {
  id: string;
  exe: string;
  args: string;
  cwd: string;
  closeOnStop: boolean;
  closeMode: CloseMode;
  startDelaySecs: number;
  runAsAdmin: boolean;
}

export interface AircraftRef {
  id: string;
  name: string;
  folders: string[];
  titles: string[];
}

export type Trigger =
  // The selection is kept on "sim" rules so switching back doesn't lose it.
  | { type: "sim"; aircraft?: AircraftRef[]; patterns?: string[] }
  | { type: "aircraft"; aircraft: AircraftRef[]; patterns: string[] };

export interface Rule {
  id: string;
  name: string;
  enabled: boolean;
  trigger: Trigger;
  programs: Program[];
}

export interface Settings {
  paused: boolean;
  stopGraceSecs: number;
  closeTimeoutSecs: number;
  /** The setup wizard has been finished once. */
  setupComplete: boolean;
  /** Packages folders added by hand, scanned along with detected installs. */
  extraPackagePaths: { sim: SimVersion; path: string }[];
}

export interface PathCheck {
  exists: boolean;
  hasCommunity: boolean;
  hasOfficial: boolean;
}

/** An MSFS install found through its UserCfg.opt. */
export interface Install extends PathCheck {
  sim: SimVersion;
  source: string;
  packagesPath: string;
}

export const isPackagesFolder = (c: PathCheck) => c.exists && (c.hasCommunity || c.hasOfficial);

export interface Config {
  rules: Rule[];
  settings: Settings;
}

export type SimVersion = "2020" | "2024";

export interface Livery {
  title: string;
  thumbnail: string | null;
}

export interface AircraftEntry {
  id: string;
  sim: SimVersion;
  manufacturer: string;
  name: string;
  package: string;
  folders: string[];
  liveries: Livery[];
  source: string;
}

export interface Catalog {
  aircraft: AircraftEntry[];
  roots: string[];
  scannedAt: number;
}

export interface ProgramStatus {
  ruleId: string;
  programId: string;
  exe: string;
  pid: number | null;
  processCount: number;
  owned: boolean;
  alive: boolean;
  state: "starting" | "running" | "exited" | "stopping";
}

export interface Status {
  simRunning: boolean;
  connected: boolean;
  simName: string;
  dll: string;
  inFlight: boolean;
  camera: number | null;
  aircraftPath: string;
  aircraftTitle: string;
  liveryName: string;
  aircraftIds: string[];
  activeRules: string[];
  programs: ProgramStatus[];
  lastError: string;
  paused: boolean;
}

export interface LogLine {
  ts: number;
  msg: string;
}

const tauriApi = {
  getConfig: () => invoke<Config>("get_config"),
  saveConfig: (config: Config) => invoke<void>("save_config", { config }),
  getStatus: () => invoke<Status>("get_status"),
  getLog: () => invoke<LogLine[]>("get_log"),
  getCatalog: () => invoke<{ catalog: Catalog; scanning: boolean }>("get_catalog"),
  rescanCatalog: () => invoke<void>("rescan_catalog"),
  testLaunch: (program: Program) => invoke<number | null>("test_launch", { program }),
  pickFile: (kind: "exe" | "folder") => invoke<string | null>("pick_file", { kind }),
  getProgramIcon: (path: string) => invoke<string | null>("get_program_icon", { path }),
  detectInstalls: () => invoke<Install[]>("detect_installs"),
  checkPackagesPath: (path: string) => invoke<PathCheck>("check_packages_path", { path }),
  /** Opens a link in the default browser (only URLs allowed by the opener capability). */
  openUrl: (url: string) => openUrl(url),
  getAutostart: () => invoke<boolean>("get_autostart"),
  setAutostart: (enabled: boolean) => invoke<void>("set_autostart", { enabled }),
};

export type Api = typeof tauriApi;

/** True inside the Tauri app; false when the UI is opened in a plain browser. */
export const inTauri = isTauri();

// In a plain browser during development, use a mock backend so the UI can be
// previewed. `import.meta.env.DEV` is false in production builds, so the mock
// is tree-shaken out.
export const api: Api = import.meta.env.DEV && !inTauri ? makeMockApi() : tauriApi;
export const newId = () => crypto.randomUUID();

export const newProgram = (): Program => ({
  id: newId(),
  exe: "",
  args: "",
  cwd: "",
  closeOnStop: true,
  closeMode: "graceful",
  startDelaySecs: 0,
  runAsAdmin: false,
});

export const newRule = (): Rule => ({
  id: newId(),
  name: "New rule",
  enabled: true,
  trigger: { type: "aircraft", aircraft: [], patterns: [] },
  programs: [newProgram()],
});

export const refFromEntry = (a: AircraftEntry): AircraftRef => ({
  id: a.id,
  name: a.manufacturer ? `${a.manufacturer} ${a.name}` : a.name,
  folders: a.folders,
  titles: a.liveries.map((l) => l.title),
});

export const fileName = (p: string) => p.split(/[\\/]/).pop() || p;

export const thumbOf = (a: AircraftEntry) => a.liveries.find((l) => l.thumbnail)?.thumbnail ?? null;
