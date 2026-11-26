import { Check, RefreshCw, Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useApp } from "../state";
import { AircraftEntry, AircraftRef, SimVersion, refFromEntry, thumbOf } from "../types";
import Thumb from "./Thumb";

interface Props {
  selected: AircraftRef[];
  onDone: (refs: AircraftRef[]) => void;
  onCancel: () => void;
}

export default function AircraftPicker({ selected, onDone, onCancel }: Props) {
  const { catalog, scanning, rescan } = useApp();
  const [query, setQuery] = useState("");
  const [sim, setSim] = useState<"all" | SimVersion>("all");
  const [picked, setPicked] = useState<Map<string, AircraftRef>>(
    () => new Map(selected.map((r) => [r.id, r])),
  );
  const dialog = useRef<HTMLDialogElement>(null);

  // Native <dialog>: focus trapping, Esc to close, top layer.
  useEffect(() => {
    dialog.current?.showModal();
  }, []);

  const groups = useMemo(() => {
    const q = query.trim().toLowerCase();
    const match = (a: AircraftEntry) =>
      (sim === "all" || a.sim === sim) &&
      (!q ||
        a.name.toLowerCase().includes(q) ||
        a.manufacturer.toLowerCase().includes(q) ||
        a.package.toLowerCase().includes(q) ||
        a.liveries.some((l) => l.title.toLowerCase().includes(q)));
    const map = new Map<string, AircraftEntry[]>();
    for (const a of catalog.aircraft) {
      if (!match(a)) continue;
      const key = a.manufacturer || "Other";
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push(a);
    }
    return [...map.entries()].sort(([a], [b]) =>
      a === "Other" ? 1 : b === "Other" ? -1 : a.localeCompare(b),
    );
  }, [catalog, query, sim]);

  const packageSize = useMemo(() => {
    const m = new Map<string, number>();
    for (const a of catalog.aircraft)
      if (a.package) m.set(`${a.sim}|${a.package}`, (m.get(`${a.sim}|${a.package}`) ?? 0) + 1);
    return m;
  }, [catalog]);

  const toggle = (a: AircraftEntry) =>
    setPicked((prev) => {
      const next = new Map(prev);
      if (next.has(a.id)) next.delete(a.id);
      else next.set(a.id, refFromEntry(a));
      return next;
    });

  const selectPackage = (a: AircraftEntry) =>
    setPicked((prev) => {
      const next = new Map(prev);
      for (const b of catalog.aircraft)
        if (b.sim === a.sim && b.package === a.package) next.set(b.id, refFromEntry(b));
      return next;
    });

  const total = groups.reduce((n, [, v]) => n + v.length, 0);

  return (
    <dialog ref={dialog} className="modal" onClose={onCancel} aria-label="Choose aircraft">
      <div className="modal-box flex h-[min(46rem,calc(100vh-3rem))] w-11/12 max-w-5xl flex-col gap-0 p-0">
        <div className="flex flex-wrap items-center gap-3 border-b border-base-300 p-4">
          <h2 className="mr-2 text-lg">Choose aircraft</h2>
          <label className="input input-sm min-w-48 flex-1">
            <Search className="size-4 opacity-50" aria-hidden="true" />
            <input
              autoFocus
              type="search"
              placeholder="Search aircraft, manufacturer, livery…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </label>
          <div className="join">
            {(["all", "2020", "2024"] as const).map((s) => (
              <button
                key={s}
                className={`btn btn-sm join-item ${sim === s ? "btn-primary" : ""}`}
                onClick={() => setSim(s)}
              >
                {s === "all" ? "All" : `MSFS ${s}`}
              </button>
            ))}
          </div>
          <button
            className="btn btn-ghost btn-sm"
            onClick={rescan}
            disabled={scanning}
            title="Rescan installed aircraft"
          >
            <RefreshCw className={`size-4 ${scanning ? "animate-spin" : ""}`} />
            {scanning ? "Scanning…" : "Rescan"}
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-4 pb-4">
          {catalog.aircraft.length === 0 && (
            <div className="py-16 text-center text-base-content/60">
              {scanning ? (
                <span className="loading loading-dots loading-md" />
              ) : (
                "No aircraft found. Check that MSFS is installed, then rescan."
              )}
            </div>
          )}
          {catalog.aircraft.length > 0 && total === 0 && (
            <p className="py-16 text-center text-base-content/60">No aircraft match “{query}”.</p>
          )}
          {groups.map(([manufacturer, list]) => (
            <section key={manufacturer}>
              <h3 className="sticky top-0 z-10 bg-base-100 pt-4 pb-2 text-xs uppercase tracking-wide text-base-content/60">
                {manufacturer}
              </h3>
              <div className="grid grid-cols-[repeat(auto-fill,minmax(11rem,1fr))] gap-3">
                {list.map((a) => {
                  const on = picked.has(a.id);
                  const pkgCount = packageSize.get(`${a.sim}|${a.package}`) ?? 0;
                  return (
                    <div key={a.id} className="flex flex-col gap-1">
                      <button
                        className={`card card-sm overflow-hidden border-2 bg-base-200 text-left transition-colors ${
                          on ? "border-primary" : "border-transparent hover:border-base-content/20"
                        }`}
                        onClick={() => toggle(a)}
                        aria-pressed={on}
                        title={a.folders.join(", ")}
                      >
                        <figure className="relative">
                          <Thumb path={thumbOf(a)} alt="" className="w-full" />
                          <span
                            className={`absolute top-2 right-2 flex size-6 items-center justify-center rounded-full border-2 border-white/80 ${
                              on ? "bg-primary text-primary-content" : "bg-black/40"
                            }`}
                            aria-hidden="true"
                          >
                            {on && <Check className="size-3.5" strokeWidth={3} />}
                          </span>
                        </figure>
                        <div className="card-body gap-1 p-2.5">
                          <span className="line-clamp-2 text-sm font-medium">{a.name}</span>
                          <span className="flex flex-wrap items-center gap-1.5 text-xs text-base-content/60">
                            <span className="badge badge-outline badge-xs">{a.sim}</span>
                            {a.liveries.length} {a.liveries.length === 1 ? "livery" : "liveries"}
                            {a.source === "simconnect" && (
                              <span className="badge badge-xs border-base-content/15 bg-base-300">
                                SimConnect
                              </span>
                            )}
                          </span>
                        </div>
                      </button>
                      {pkgCount > 1 && (
                        <button
                          className="link link-primary self-start pl-1 text-xs"
                          onClick={() => selectPackage(a)}
                          title={a.package}
                        >
                          Select whole package ({pkgCount})
                        </button>
                      )}
                    </div>
                  );
                })}
              </div>
            </section>
          ))}
        </div>

        <div className="modal-action m-0 items-center border-t border-base-300 p-4">
          <span className="mr-auto text-sm text-base-content/60">
            {picked.size} selected · {catalog.aircraft.length} installed
          </span>
          <button className="btn btn-ghost" onClick={() => dialog.current?.close()}>
            Cancel
          </button>
          <button className="btn btn-primary" onClick={() => onDone([...picked.values()])}>
            Done
          </button>
        </div>
      </div>
      <form method="dialog" className="modal-backdrop">
        <button aria-label="Close">close</button>
      </form>
    </dialog>
  );
}
