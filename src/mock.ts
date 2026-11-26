// Dev-only fake backend, used when the UI runs in a plain browser (no Tauri).
// Lets the UI be previewed and iterated on without the app or the sim running.
import type { Api, AircraftEntry, Catalog, Config, LogLine, Status } from "./types";

const ac = (
  id: string,
  sim: "2020" | "2024",
  manufacturer: string,
  name: string,
  pkg: string,
  liveries: string[],
): AircraftEntry => ({
  id,
  sim,
  manufacturer,
  name,
  package: pkg,
  folders: [id.split(":")[1]],
  liveries: liveries.map((title) => ({ title, thumbnail: null })),
  source: "files",
});

/** Built lazily so production bundles (which never call it) drop this module. */
export function makeMockApi(): Api {
  const catalog: Catalog = {
    aircraft: [
      ac("2024:pmdg 737-800", "2024", "Boeing", "737-800", "pmdg-aircraft-738", [
        "PMDG House",
        "KLM",
      ]),
      ac("2024:pmdg 777-300er", "2024", "Boeing", "777-300ER", "pmdg-aircraft-77w", ["PMDG House"]),
      ac("2024:bbs-757-200-pw", "2024", "Boeing", "757-200 PW", "bluebirdsim-aircraft-757-200", [
        "House",
        "Delta",
        "Condor",
      ]),
      ac(
        "2024:tfdi_design_md-11",
        "2024",
        "McDonnell Douglas",
        "MD-11",
        "tfdidesign-aircraft-md11",
        ["Federal Express N602FE", "Finnair 'Moomins' (OH-LGC)", "KLM PH-KCA"],
      ),
      ac("2024:inibuilds-a300", "2024", "Airbus", "A300-600R", "inibuilds-a300", [
        "American Airlines (N33069)",
      ]),
      ac("2020:fnx_320_cfm", "2020", "Airbus", "A320 CFM", "fnx-aircraft-319-321", [
        "Fenix House",
        "easyJet",
      ]),
      ac("2020:asobo_c172sp_as1000", "2020", "", "C172sp AS1000", "asobo-aircraft-c172sp-as1000", [
        "Asobo",
        "Livery 01",
      ]),
      ac("2024:just flight f100", "2024", "Just Flight", "F100", "justflight-aircraft-f100", [
        "Just Flight House",
      ]),
    ],
    roots: ["2020: W:\\msfs2020", "2024: X:\\msfs2024"],
    scannedAt: Math.floor(Date.now() / 1000) - 3600,
  };

  let config: Config = {
    rules: [
      {
        id: "r1",
        name: "MD-11 companion",
        enabled: true,
        trigger: {
          type: "aircraft",
          aircraft: [
            {
              id: "2024:tfdi_design_md-11",
              name: "McDonnell Douglas MD-11",
              folders: ["TFDi_Design_MD-11"],
              titles: [],
            },
          ],
          patterns: [],
        },
        programs: [
          {
            id: "p1",
            exe: "C:\\Program Files\\TFDi Design\\MD-11\\TFDi.MD11.Companion.exe",
            args: "",
            cwd: "",
            closeOnStop: true,
            closeMode: "graceful",
            startDelaySecs: 0,
            runAsAdmin: false,
          },
        ],
      },
      {
        id: "r2",
        name: "Always with the sim",
        enabled: true,
        trigger: { type: "sim" },
        programs: [
          {
            id: "p2",
            exe: "C:\\Program Files\\Navigraph\\Simlink\\Navigraph Simlink.exe",
            args: "--minimized",
            cwd: "",
            closeOnStop: true,
            closeMode: "graceful",
            startDelaySecs: 5,
            runAsAdmin: false,
          },
          {
            id: "p3",
            exe: "C:\\Tools\\FSUIPC7\\FSUIPC7.exe",
            args: "",
            cwd: "",
            closeOnStop: false,
            closeMode: "kill",
            startDelaySecs: 0,
            runAsAdmin: true,
          },
        ],
      },
    ],
    // setupComplete: false, so the setup wizard shows in the browser preview.
    settings: {
      paused: false,
      stopGraceSecs: 10,
      closeTimeoutSecs: 10,
      setupComplete: false,
      extraPackagePaths: [],
    },
  };

  const status: Status = {
    simRunning: true,
    connected: true,
    simName: "SunRise",
    dll: "C:\\…\\lib\\SimConnect.dll",
    inFlight: true,
    camera: 2,
    aircraftPath:
      "SimObjects\\Airplanes\\TFDi_Design_MD-11\\presets\\tfdi-design\\TFDi_Design_MD-11 GE\\config\\aircraft.CFG",
    aircraftTitle: "TFDi Design MD-11 GE CF6-80c",
    liveryName: "Finnair 'Moomins' (OH-LGC)",
    aircraftIds: ["2024:tfdi_design_md-11"],
    activeRules: ["r1", "r2"],
    programs: [
      {
        ruleId: "r1",
        programId: "p1",
        exe: config.rules[0].programs[0].exe,
        pid: 4242,
        processCount: 2,
        owned: true,
        alive: true,
        state: "running",
      },
      {
        ruleId: "r2",
        programId: "p2",
        exe: config.rules[1].programs[0].exe,
        pid: 5150,
        processCount: 1,
        owned: true,
        alive: true,
        state: "running",
      },
      {
        ruleId: "r2",
        programId: "p3",
        exe: config.rules[1].programs[1].exe,
        pid: 6000,
        processCount: 1,
        owned: false,
        alive: true,
        state: "running",
      },
    ],
    lastError: "",
    paused: false,
  };

  const now = Math.floor(Date.now() / 1000);
  const log: LogLine[] = [
    "Aircraft cache: 276 aircraft, 3483 AI titles",
    "MSFS process detected",
    "Connected to SunRise",
    "SimConnect listed 6234 aircraft/liveries (0 new aircraft added)",
    "Aircraft: SimObjects\\Airplanes\\TFDi_Design_MD-11\\presets\\…\\aircraft.CFG",
    "Flight started (camera 2)",
    "Started TFDi.MD11.Companion.exe (pid 4242)",
  ].map((msg, i) => ({ ts: now - 120 + i * 9, msg }));

  const wait = <T>(v: T) => new Promise<T>((r) => setTimeout(() => r(structuredClone(v)), 50));

  return {
    getConfig: () => wait(config),
    saveConfig: async (c) => {
      config = structuredClone(c);
    },
    getStatus: () => wait({ ...status, paused: config.settings.paused }),
    getLog: () => wait(log),
    getCatalog: () => wait({ catalog, scanning: false }),
    rescanCatalog: async () => {},
    testLaunch: () => wait(1234),
    pickFile: () => wait("C:\\Program Files\\Example\\Companion.exe"),
    // A coloured tile per program, so the icon layout can be previewed in a browser.
    getProgramIcon: (path) => {
      if (!path.trim()) return wait(null);
      const hue = [...path].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7);
      const letter = (path.split(/[\\/]/).pop() ?? "?").charAt(0).toUpperCase();
      const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" rx="7" fill="hsl(${hue} 60% 45%)"/><text x="16" y="22" font-size="17" font-family="sans-serif" font-weight="700" text-anchor="middle" fill="#fff">${letter}</text></svg>`;
      return wait(`data:image/svg+xml,${encodeURIComponent(svg)}`);
    },
    detectInstalls: () =>
      wait([
        {
          sim: "2020",
          source: "Steam",
          packagesPath: "W:\\msfs2020",
          exists: true,
          hasCommunity: true,
          hasOfficial: true,
        },
        {
          sim: "2024",
          source: "Steam",
          packagesPath: "X:\\msfs2024",
          exists: true,
          hasCommunity: true,
          hasOfficial: true,
        },
      ]),
    checkPackagesPath: (path) =>
      wait({
        exists: !!path.trim(),
        hasCommunity: /msfs|community|packages/i.test(path),
        hasOfficial: false,
      }),
    openUrl: async (url) => {
      window.open(url, "_blank");
    },
    getAutostart: () => wait(false),
    setAutostart: async () => {},
  };
}
