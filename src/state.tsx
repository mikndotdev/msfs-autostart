import { listen } from "@tauri-apps/api/event";
import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import { useLocation } from "react-router";
import { Catalog, Config, LogLine, Rule, Status, api, inTauri } from "./types";

export const emptyStatus: Status = {
  simRunning: false,
  connected: false,
  simName: "",
  dll: "",
  inFlight: false,
  camera: null,
  aircraftPath: "",
  aircraftTitle: "",
  liveryName: "",
  aircraftIds: [],
  activeRules: [],
  programs: [],
  lastError: "",
  paused: false,
};

interface AppState {
  config: Config | null;
  /** Updates the config immediately and saves it (debounced). */
  updateConfig: (next: Config) => void;
  status: Status;
  catalog: Catalog;
  scanning: boolean;
  rescan: () => void;
  log: LogLine[];
  /** Adds a new rule as a draft: it's discarded if left blank when the editor closes. */
  addDraftRule: (rule: Rule) => void;
}

/** A rule the user hasn't given an aircraft, pattern or program yet. */
export function isBlankRule(r: Rule): boolean {
  const aircraft = r.trigger.aircraft ?? [];
  const patterns = (r.trigger.patterns ?? []).filter((p) => p.trim());
  return aircraft.length === 0 && patterns.length === 0 && !r.programs.some((p) => p.exe.trim());
}

const Ctx = createContext<AppState | null>(null);

export function useApp(): AppState {
  const v = useContext(Ctx);
  if (!v) throw new Error("useApp outside AppProvider");
  return v;
}

/** Like useApp, but for components that need the config loaded. */
export function useConfig() {
  const app = useApp();
  if (!app.config) throw new Error("config not loaded");
  return { ...app, config: app.config };
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [config, setConfig] = useState<Config | null>(null);
  const [status, setStatus] = useState<Status>(emptyStatus);
  const [catalog, setCatalog] = useState<Catalog>({ aircraft: [], roots: [], scannedAt: 0 });
  const [scanning, setScanning] = useState(false);
  const [log, setLog] = useState<LogLine[]>([]);
  const saveTimer = useRef<number | undefined>(undefined);
  const configRef = useRef<Config | null>(null);
  // Ids of rules created in this session that may still be blank drafts.
  const drafts = useRef(new Set<string>());

  const loadCatalog = useCallback(async () => {
    const r = await api.getCatalog();
    setCatalog(r.catalog);
    setScanning(r.scanning);
  }, []);

  useEffect(() => {
    api.getConfig().then((c) => {
      configRef.current = c;
      setConfig(c);
    });
    api.getStatus().then(setStatus);
    api.getLog().then(setLog);
    loadCatalog();
    if (!inTauri) return;
    const unlisten = [
      listen<Status>("status", (e) => setStatus(e.payload)),
      listen<LogLine>("log", (e) => setLog((l) => [...l.slice(-299), e.payload])),
      listen("catalog-changed", () => loadCatalog()),
      // Changes made from the tray (Pause).
      listen<Config>("config-changed", (e) =>
        setConfig((c) => {
          if (!c || c.settings.paused === e.payload.settings.paused) return c;
          const next = { ...c, settings: e.payload.settings };
          configRef.current = next;
          return next;
        }),
      ),
    ];
    return () => unlisten.forEach((p) => p.then((f) => f()));
  }, [loadCatalog]);

  const updateConfig = useCallback((next: Config) => {
    configRef.current = next;
    setConfig(next);
    window.clearTimeout(saveTimer.current);
    // Blank drafts are never written to disk, so the backend never sees them either.
    const toSave = {
      ...next,
      rules: next.rules.filter((r) => !(drafts.current.has(r.id) && isBlankRule(r))),
    };
    saveTimer.current = window.setTimeout(() => api.saveConfig(toSave), 400);
  }, []);

  const addDraftRule = useCallback(
    (rule: Rule) => {
      const c = configRef.current;
      if (!c) return;
      drafts.current.add(rule.id);
      updateConfig({ ...c, rules: [...c.rules, rule] });
    },
    [updateConfig],
  );

  // Leaving a draft's editor (by any route change) drops it if it's still blank.
  const { pathname } = useLocation();
  const prevPath = useRef(pathname);
  useEffect(() => {
    const prev = prevPath.current;
    prevPath.current = pathname;
    const left = /^\/rules\/([^/]+)$/.exec(prev)?.[1];
    if (!left || pathname === prev || !drafts.current.has(left)) return;
    drafts.current.delete(left);
    const c = configRef.current;
    const rule = c?.rules.find((r) => r.id === left);
    if (c && rule && isBlankRule(rule))
      updateConfig({ ...c, rules: c.rules.filter((r) => r.id !== left) });
  }, [pathname, updateConfig]);

  const rescan = useCallback(() => {
    setScanning(true);
    api.rescanCatalog();
  }, []);

  return (
    <Ctx.Provider
      value={{ config, updateConfig, status, catalog, scanning, rescan, log, addDraftRule }}
    >
      {children}
    </Ctx.Provider>
  );
}
