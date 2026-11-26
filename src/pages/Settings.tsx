import { Compass, RefreshCw, Wand } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { useConfig } from "../state";
import { useStartRuleTour } from "../tour";
import mikandevLogo from "../assets/MDLogo.png";
import { Config, api } from "../types";

export default function SettingsPage() {
  const { config, updateConfig, catalog, scanning, rescan } = useConfig();
  const [autostart, setAutostartState] = useState<boolean | null>(null);
  const navigate = useNavigate();
  const startRuleTour = useStartRuleTour();
  const s = config.settings;
  const set = (patch: Partial<Config["settings"]>) =>
    updateConfig({ ...config, settings: { ...s, ...patch } });

  useEffect(() => {
    api.getAutostart().then(setAutostartState);
  }, []);

  const toggleAutostart = async (on: boolean) => {
    await api.setAutostart(on);
    setAutostartState(await api.getAutostart());
  };

  return (
    <div className="mx-auto max-w-5xl space-y-5 p-6">
      <div className="card card-border bg-base-200">
        <div className="card-body">
          <h2 className="card-title text-base">Startup</h2>
          <label className="label cursor-pointer justify-between gap-4 text-base-content">
            <span>
              Start with Windows
              <span className="block text-sm text-base-content/60">
                Starts hidden in the system tray.
              </span>
            </span>
            <input
              type="checkbox"
              className="toggle toggle-primary"
              checked={!!autostart}
              disabled={autostart === null}
              onChange={(e) => toggleAutostart(e.target.checked)}
            />
          </label>
          <p className="text-sm text-base-content/60">
            Closing this window keeps MSFS Autostart running in the tray. Use the tray menu to quit.
          </p>
        </div>
      </div>

      <div className="card card-border bg-base-200">
        <div className="card-body">
          <h2 className="card-title text-base">Closing programs</h2>
          <div className="grid gap-x-4 sm:grid-cols-2">
            <fieldset className="fieldset">
              <legend className="fieldset-legend">Wait before closing</legend>
              <label className="input w-full">
                <input
                  type="number"
                  min={0}
                  value={s.stopGraceSecs}
                  onChange={(e) => set({ stopGraceSecs: Math.max(0, Number(e.target.value) || 0) })}
                />
                <span className="label">seconds</span>
              </label>
              <p className="label whitespace-normal">
                Keeps programs open through a flight restart.
              </p>
            </fieldset>
            <fieldset className="fieldset">
              <legend className="fieldset-legend">Force close after</legend>
              <label className="input w-full">
                <input
                  type="number"
                  min={1}
                  value={s.closeTimeoutSecs}
                  onChange={(e) =>
                    set({ closeTimeoutSecs: Math.max(1, Number(e.target.value) || 1) })
                  }
                />
                <span className="label">seconds</span>
              </label>
              <p className="label whitespace-normal">
                How long a program gets to close on its own.
              </p>
            </fieldset>
          </div>
          <p className="text-sm text-base-content/60">
            Only programs MSFS Autostart started are closed. Programs that were already open stay
            open.
          </p>
        </div>
      </div>

      <div className="card card-border bg-base-200">
        <div className="card-body">
          <h2 className="card-title text-base">Aircraft catalog</h2>
          <p className="text-sm">
            <span className="font-semibold">{catalog.aircraft.length}</span> aircraft found
            {catalog.scannedAt ? (
              <span className="text-base-content/60">
                , last scanned {new Date(catalog.scannedAt * 1000).toLocaleString()}
              </span>
            ) : null}
            .
          </p>
          <ul className="space-y-1">
            {catalog.roots.map((r) => (
              <li key={r} className="font-mono text-xs text-base-content/70">
                {r}
              </li>
            ))}
          </ul>
          <div className="card-actions">
            <button className="btn btn-sm" onClick={rescan} disabled={scanning}>
              <RefreshCw className={`size-4 ${scanning ? "animate-spin" : ""}`} />
              {scanning ? "Scanning…" : "Rescan now"}
            </button>
          </div>
        </div>
      </div>

      <div className="card card-border bg-base-200">
        <div className="card-body">
          <h2 className="card-title text-base">Setup and help</h2>
          <p className="text-sm text-base-content/70">
            Go through the first-launch setup again to check where MSFS is installed, add a packages
            folder, or change how the app starts. Or replay the guided tour of creating a rule.
          </p>
          <div className="card-actions">
            <button className="btn btn-sm" onClick={() => navigate("/setup")}>
              <Wand className="size-4" /> Run setup again
            </button>
            <button className="btn btn-sm" onClick={startRuleTour}>
              <Compass className="size-4" /> Show the rule tour
            </button>
          </div>
        </div>
      </div>

      <footer className="mt-10 flex w-full flex-col items-center justify-center space-y-2">
        <button
          type="button"
          className="w-1/10 cursor-pointer rounded-box transition-opacity hover:opacity-85"
          onClick={() => api.openUrl("https://mikn.dev")}
          title="mikn.dev"
        >
          <img src={mikandevLogo} className="w-full" alt="MikanDev" draggable={false} />
        </button>
        <span className="text-sm text-base-content/60">
          &copy; 2020-{new Date().getFullYear()} MikanDev
        </span>
        <span className="text-sm text-base-content/60">
          This is a third-party app and is not affiliated with Microsoft/MSFS in any way.
        </span>
      </footer>
    </div>
  );
}
