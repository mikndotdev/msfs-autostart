import {
  Check,
  Compass,
  FolderOpen,
  Plus,
  Power,
  Trash,
  TriangleExclamationPoint,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import appIcon from "../../src-tauri/icons/app-icon.svg";
import { useConfig } from "../state";
import { useStartRuleTour } from "../tour";
import { Install, PathCheck, Settings, SimVersion, api, isPackagesFolder } from "../types";

type Extra = Settings["extraPackagePaths"][number] & { check?: PathCheck };

const STEPS = ["Welcome", "MSFS", "Startup", "Done"];

/**
 * First-launch setup: confirms where MSFS is installed (with a manual fallback) and
 * whether to start with Windows. Re-runnable from Settings.
 */
export default function SetupPage() {
  const { config, updateConfig, rescan } = useConfig();
  const startRuleTour = useStartRuleTour();
  const navigate = useNavigate();
  const rerun = config.settings.setupComplete;
  const [step, setStep] = useState(0);
  const [installs, setInstalls] = useState<Install[] | null>(null);
  const [extra, setExtra] = useState<Extra[]>(config.settings.extraPackagePaths);
  // Recommended on first run; a re-run starts from the current setting.
  const [autostart, setAutostart] = useState(true);
  const [finishing, setFinishing] = useState(false);

  useEffect(() => {
    api.detectInstalls().then(setInstalls);
    if (rerun) api.getAutostart().then(setAutostart);
    // Re-check folders added on a previous run.
    config.settings.extraPackagePaths.forEach((e, i) =>
      api
        .checkPackagesPath(e.path)
        .then((check) => setExtra((x) => x.map((y, j) => (j === i ? { ...y, check } : y)))),
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const addFolder = async () => {
    const path = await api.pickFile("folder");
    if (!path || extra.some((e) => e.path.toLowerCase() === path.toLowerCase())) return;
    const check = await api.checkPackagesPath(path);
    const sim: SimVersion = /2020/.test(path) ? "2020" : "2024";
    setExtra((x) => [...x, { sim, path, check }]);
  };

  const finish = async (thenTour: boolean) => {
    setFinishing(true);
    const extraPackagePaths = extra.map(({ sim, path }) => ({ sim, path }));
    const pathsChanged =
      JSON.stringify(extraPackagePaths) !== JSON.stringify(config.settings.extraPackagePaths);
    const next = {
      ...config,
      settings: { ...config.settings, setupComplete: true, extraPackagePaths },
    };
    try {
      await api.setAutostart(autostart);
    } catch {
      // Not fatal: it can be changed later in Settings.
    }
    updateConfig(next);
    // Save right away (not debounced) so the rescan sees the new folders.
    await api.saveConfig(next);
    if (pathsChanged || !rerun) rescan();
    if (thenTour) startRuleTour();
    else navigate(rerun ? "/settings" : "/rules");
  };

  const usable =
    (installs ?? []).filter((i) => isPackagesFolder(i)).length +
    extra.filter((e) => e.check && isPackagesFolder(e.check)).length;

  return (
    <div className="mx-auto flex min-h-full max-w-3xl flex-col gap-6 p-6">
      <div className="flex items-center gap-4">
        <ul className="steps flex-1">
          {STEPS.map((s, i) => (
            <li key={s} className={`step text-sm ${i <= step ? "step-primary" : ""}`}>
              {s}
            </li>
          ))}
        </ul>
        {rerun && (
          <button
            className="btn btn-ghost btn-sm btn-square"
            onClick={() => navigate("/settings")}
            aria-label="Close setup"
          >
            <X className="size-5" />
          </button>
        )}
      </div>

      <div className="card card-border bg-base-200">
        <div className="card-body gap-5 p-8">
          {step === 0 && (
            <div className="flex flex-col items-center gap-4 py-4 text-center">
              <img src={appIcon} alt="" className="size-20 rounded-2xl" />
              <h1 className="text-2xl">{rerun ? "Setup" : "Welcome to MSFS Autostart"}</h1>
              <p className="max-w-lg text-base-content/70">
                MSFS Autostart opens your add-ons' companion programs when you start a flight, or
                when the sim launches, and closes them again afterwards.
              </p>
              <p className="max-w-lg text-base-content/70">
                This takes a minute: we'll check where Microsoft Flight Simulator is installed, then
                decide how the app starts.
              </p>
            </div>
          )}

          {step === 1 && (
            <>
              <div>
                <h2 className="text-xl">Where's MSFS installed?</h2>
                <p className="mt-1 text-sm text-base-content/70">
                  Used to list your installed aircraft. These were found automatically; add a folder
                  only if an install is missing.
                </p>
              </div>

              {installs === null ? (
                <div className="flex items-center gap-3 text-base-content/60">
                  <span className="loading loading-spinner loading-sm" /> Looking for MSFS…
                </div>
              ) : installs.length === 0 ? (
                <div role="alert" className="alert alert-warning alert-soft">
                  <TriangleExclamationPoint className="size-5" />
                  <span>
                    No MSFS install was found automatically. Add your packages folder below: it's
                    the folder you chose when installing MSFS, and it contains the Community and
                    Official folders.
                  </span>
                </div>
              ) : (
                <ul className="space-y-2">
                  {installs.map((i) => (
                    <InstallRow
                      key={i.sim + i.packagesPath}
                      title={`MSFS ${i.sim}`}
                      subtitle={i.source}
                      path={i.packagesPath}
                      check={i}
                    />
                  ))}
                </ul>
              )}

              {extra.length > 0 && (
                <div className="space-y-2">
                  <h3 className="text-sm uppercase tracking-wide text-base-content/60">
                    Added by you
                  </h3>
                  <ul className="space-y-2">
                    {extra.map((e, i) => (
                      <InstallRow
                        key={e.path}
                        path={e.path}
                        check={e.check}
                        title={
                          <select
                            className="select select-xs w-auto"
                            value={e.sim}
                            aria-label="Sim version"
                            onChange={(ev) =>
                              setExtra((x) =>
                                x.map((y, j) =>
                                  j === i ? { ...y, sim: ev.target.value as SimVersion } : y,
                                ),
                              )
                            }
                          >
                            <option value="2020">MSFS 2020</option>
                            <option value="2024">MSFS 2024</option>
                          </select>
                        }
                        onRemove={() => setExtra((x) => x.filter((_, j) => j !== i))}
                      />
                    ))}
                  </ul>
                </div>
              )}

              <button className="btn btn-sm self-start" onClick={addFolder}>
                <Plus className="size-4" /> Add packages folder
              </button>
            </>
          )}

          {step === 2 && (
            <>
              <div>
                <h2 className="text-xl">How should the app start?</h2>
                <p className="mt-1 text-sm text-base-content/70">
                  The app has to be running to notice when you start MSFS.
                </p>
              </div>
              <label
                className={`flex cursor-pointer items-start gap-4 rounded-box border p-4 transition-colors ${
                  autostart ? "border-primary bg-primary/5" : "border-base-300"
                }`}
              >
                <Power
                  className={`mt-0.5 size-6 shrink-0 ${autostart ? "text-primary" : "text-base-content/40"}`}
                />
                <span className="flex-1">
                  <span className="flex flex-wrap items-center gap-2 font-bold">
                    Start with Windows
                    <span className="badge badge-primary badge-soft badge-sm border-primary/30">
                      Recommended
                    </span>
                  </span>
                  <span className="mt-1 block text-sm text-base-content/70">
                    Starts quietly in the system tray when you sign in, so your programs are always
                    ready when you launch MSFS. It uses almost no resources while waiting.
                  </span>
                </span>
                <input
                  type="checkbox"
                  className="toggle toggle-primary mt-0.5"
                  checked={autostart}
                  onChange={(e) => setAutostart(e.target.checked)}
                />
              </label>
              {!autostart && (
                <p className="text-sm text-base-content/60">
                  You'll need to open MSFS Autostart yourself before starting the sim. You can
                  change this later in Settings.
                </p>
              )}
            </>
          )}

          {step === 3 && (
            <div className="flex flex-col items-center gap-4 py-4 text-center">
              <div className="flex size-16 items-center justify-center rounded-full bg-success/15 text-success">
                <Check className="size-8" strokeWidth={2.5} />
              </div>
              <h2 className="text-2xl">You're all set</h2>
              <ul className="space-y-1 text-sm text-base-content/70">
                <li>
                  {usable} MSFS {usable === 1 ? "install" : "installs"} will be scanned for aircraft
                </li>
                <li>
                  {autostart ? "Starts with Windows, in the tray" : "Doesn't start with Windows"}
                </li>
              </ul>
              <p className="max-w-lg text-base-content/70">
                Want a quick tour of creating a rule? It shows where to pick the aircraft and the
                companion program. It's optional, and you can replay it from Settings.
              </p>
            </div>
          )}
        </div>
      </div>

      <div className="flex items-center justify-between">
        <button
          className="btn btn-ghost"
          onClick={() => setStep((s) => s - 1)}
          disabled={step === 0 || finishing}
        >
          Back
        </button>
        {step < STEPS.length - 1 ? (
          <button className="btn btn-primary" onClick={() => setStep((s) => s + 1)}>
            {step === 0 ? "Get started" : "Next"}
          </button>
        ) : (
          <div className="flex gap-2">
            <button className="btn btn-ghost" onClick={() => finish(false)} disabled={finishing}>
              {rerun ? "Finish" : "Skip the tour"}
            </button>
            <button className="btn btn-primary" onClick={() => finish(true)} disabled={finishing}>
              <Compass className="size-4" /> Show me how to create a rule
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

function InstallRow({
  title,
  subtitle,
  path,
  check,
  onRemove,
}: {
  title: React.ReactNode;
  subtitle?: string;
  path: string;
  check?: PathCheck;
  onRemove?: () => void;
}) {
  const ok = check ? isPackagesFolder(check) : null;
  return (
    <li className="flex items-center gap-3 rounded-box border border-base-300 bg-base-100 p-3">
      <FolderOpen className="size-5 shrink-0 text-base-content/40" aria-hidden="true" />
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2 text-sm font-bold">
          {title}
          {subtitle && <span className="font-normal text-base-content/60">{subtitle}</span>}
        </div>
        <div className="truncate font-mono text-xs text-base-content/70" title={path}>
          {path}
        </div>
      </div>
      {ok === null ? (
        <span className="loading loading-spinner loading-xs" />
      ) : ok ? (
        <span className="badge badge-success badge-soft badge-sm border-success/30 gap-1">
          <Check className="size-3" /> Found
        </span>
      ) : (
        <span
          className="badge badge-warning badge-soft badge-sm border-warning/30"
          title={check?.exists ? "No Community or Official folder in here" : "Folder doesn't exist"}
        >
          {check?.exists ? "Not a packages folder" : "Missing"}
        </span>
      )}
      {onRemove && (
        <button
          className="btn btn-ghost btn-xs btn-square text-error"
          onClick={onRemove}
          aria-label="Remove folder"
        >
          <Trash className="size-4" />
        </button>
      )}
    </li>
  );
}
