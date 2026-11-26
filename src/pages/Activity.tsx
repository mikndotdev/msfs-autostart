import { useEffect, useRef } from "react";
import { useConfig } from "../state";
import { ProgramStatus, fileName } from "../types";

const stateBadge: Record<ProgramStatus["state"], string> = {
  starting: "badge-warning",
  running: "badge-success",
  stopping: "badge-warning",
  exited: "border-base-content/15 bg-base-300",
};

export default function ActivityPage() {
  const { status, log, config } = useConfig();
  const logRef = useRef<HTMLDivElement>(null);
  // Keep the log scrolled to the newest line (only the log box, not the page).
  useEffect(() => {
    const el = logRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [log.length]);
  const ruleName = (id: string) => config.rules.find((r) => r.id === id)?.name ?? "(deleted rule)";

  const simState = status.connected
    ? status.inFlight
      ? "In flight"
      : "In menus"
    : status.simRunning
      ? "Starting"
      : "Not running";

  return (
    <div className="mx-auto max-w-5xl space-y-5 p-6">
      <div className="stats stats-vertical w-full border border-base-300 bg-base-200 sm:stats-horizontal">
        <div className="stat">
          <div className="stat-title">Sim</div>
          <div className="stat-value text-2xl">{simState}</div>
          <div className="stat-desc">{status.simName || "—"}</div>
        </div>
        <div className="stat min-w-0">
          <div className="stat-title">Aircraft</div>
          <div className="stat-value truncate text-lg" title={status.aircraftTitle}>
            {status.aircraftTitle || "—"}
          </div>
          <div className="stat-desc truncate" title={status.liveryName}>
            {status.liveryName || "No livery reported"}
          </div>
        </div>
        <div className="stat">
          <div className="stat-title">Camera state</div>
          <div className="stat-value text-2xl">{status.camera ?? "—"}</div>
          <div className="stat-desc">Used to tell flights from menus</div>
        </div>
      </div>

      <div className="card card-border bg-base-200">
        <div className="card-body">
          <h2 className="card-title text-base">Managed programs</h2>
          {status.programs.length === 0 ? (
            <p className="text-sm text-base-content/60">Nothing running.</p>
          ) : (
            <div className="overflow-x-auto">
              <table className="table table-sm">
                <thead>
                  <tr>
                    <th>Program</th>
                    <th>Rule</th>
                    <th>State</th>
                    <th>PID</th>
                  </tr>
                </thead>
                <tbody>
                  {status.programs.map((p) => (
                    <tr key={p.ruleId + p.programId}>
                      <td className="font-medium">{fileName(p.exe)}</td>
                      <td>{ruleName(p.ruleId)}</td>
                      <td>
                        <span
                          className={`badge badge-soft badge-sm capitalize ${stateBadge[p.state]}`}
                        >
                          {p.state}
                        </span>
                        {!p.owned && p.state !== "starting" && (
                          <span className="ml-2 text-xs text-base-content/60">
                            already open, won't be closed
                          </span>
                        )}
                      </td>
                      <td className="font-mono text-xs">
                        {p.pid ?? "—"}
                        {p.processCount > 1 && (
                          <span className="text-base-content/50"> +{p.processCount - 1}</span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>

      <div className="card card-border bg-base-200">
        <div className="card-body">
          <h2 className="card-title text-base">Log</h2>
          <div
            ref={logRef}
            className="max-h-80 overflow-y-auto rounded-box bg-base-300 p-3 font-mono text-xs leading-relaxed"
          >
            {log.length === 0 && <span className="text-base-content/50">No activity yet.</span>}
            {log.map((l, i) => (
              <div key={i} className="break-all">
                <span className="text-base-content/40">
                  {new Date(l.ts * 1000).toLocaleTimeString()}
                </span>{" "}
                {l.msg}
              </div>
            ))}
          </div>
          <p className="text-xs text-base-content/50">
            Also saved to activity.log in %APPDATA%\dev.mikn.msfs-autostart (cleared on each start).
          </p>
        </div>
      </div>
    </div>
  );
}
