import {
  ArrowLeft,
  FolderOpen,
  Monitor,
  Pencil,
  Plane,
  Play,
  Plus,
  Trash,
  TriangleExclamationPoint,
  X,
} from "lucide-react";
import { useRef, useState } from "react";
import { Link, Navigate, useNavigate, useParams } from "react-router";
import AircraftPicker from "../components/AircraftPicker";
import ProgramIcon from "../components/ProgramIcon";
import Thumb from "../components/Thumb";
import { TOUR_IDS } from "../tour";
import { useConfig } from "../state";
import {
  AircraftRef,
  Program,
  ProgramStatus,
  Rule,
  api,
  fileName,
  newProgram,
  thumbOf,
} from "../types";

export default function RuleEditor() {
  const { id } = useParams();
  const { config, updateConfig, status, catalog } = useConfig();
  const navigate = useNavigate();
  const [picking, setPicking] = useState(false);
  const rule = config.rules.find((r) => r.id === id);
  if (!rule) return <Navigate to="/rules" replace />;

  const set = (patch: Partial<Rule>) =>
    updateConfig({
      ...config,
      rules: config.rules.map((r) => (r.id === rule.id ? { ...rule, ...patch } : r)),
    });
  const del = () => {
    updateConfig({
      ...config,
      rules: config.rules.filter((r) => r.id !== rule.id),
    });
    navigate("/rules");
  };

  const trigger = rule.trigger;
  // Kept for both trigger types, so switching tabs never loses the selection.
  const aircraft = trigger.aircraft ?? [];
  const patterns = trigger.patterns ?? [];
  const setAircraft = (a: AircraftRef[], p: string[] = patterns) =>
    set({ trigger: { type: "aircraft", aircraft: a, patterns: p } });
  const active = status.activeRules.includes(rule.id);
  const entryFor = (aid: string) => catalog.aircraft.find((a) => a.id === aid);
  const updateProgram = (i: number, p: Program) => {
    const programs = [...rule.programs];
    programs[i] = p;
    set({ programs });
  };

  return (
    <div className="mx-auto max-w-4xl space-y-5 p-6">
      {/* Header */}
      <div className="flex flex-wrap items-center gap-3">
        <Link to="/rules" className="btn btn-ghost btn-sm btn-square" aria-label="Back to rules">
          <ArrowLeft className="size-5" />
        </Link>
        <label
          id={TOUR_IDS.ruleName}
          className="input input-lg group min-w-48 flex-1 border-base-content/20 bg-base-200 transition-colors hover:border-base-content/40"
          title="Rename rule"
        >
          <input
            className="text-xl font-bold"
            value={rule.name}
            onChange={(e) => set({ name: e.target.value })}
            placeholder="Rule name"
            aria-label="Rule name"
          />
          <Pencil
            className="size-4 shrink-0 opacity-40 transition-opacity group-hover:opacity-70 group-focus-within:text-primary group-focus-within:opacity-100"
            aria-hidden="true"
          />
        </label>
        {active && <span className="badge badge-success badge-soft">Active now</span>}
        <label className="label cursor-pointer gap-2">
          <input
            type="checkbox"
            className="toggle toggle-primary"
            checked={rule.enabled}
            onChange={(e) => set({ enabled: e.target.checked })}
          />
          Enabled
        </label>
        <DeleteButton rule={rule} onConfirm={del} />
      </div>

      {/* Trigger */}
      <div className="space-y-3">
        <div className="flex h-8 items-center">
          <h2 className="text-sm uppercase tracking-wide text-base-content/60">When</h2>
        </div>
        <div className="card card-border bg-base-200">
          <div className="card-body gap-4">
            <div id={TOUR_IDS.trigger} role="tablist" className="tabs tabs-box w-fit">
              <button
                role="tab"
                className={`tab gap-2 ${trigger.type === "sim" ? "tab-active" : ""}`}
                aria-selected={trigger.type === "sim"}
                onClick={() => set({ trigger: { type: "sim", aircraft, patterns } })}
              >
                <Monitor className="size-4" /> With the sim
              </button>
              <button
                role="tab"
                className={`tab gap-2 ${trigger.type === "aircraft" ? "tab-active" : ""}`}
                aria-selected={trigger.type === "aircraft"}
                onClick={() => setAircraft(aircraft, patterns)}
              >
                <Plane className="size-4" /> With specific aircraft
              </button>
            </div>
            <p className="text-sm text-base-content/70">
              {trigger.type === "sim"
                ? "Programs start when MSFS launches and close when it exits."
                : "Programs start once you're in the cockpit with one of these aircraft, and close when you switch aircraft, return to the menu, or exit the sim."}
            </p>

            {trigger.type === "aircraft" && (
              <>
                <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
                  {aircraft.map((r) => {
                    const e = entryFor(r.id);
                    return (
                      <div key={r.id} className="card card-sm overflow-hidden bg-base-300">
                        <figure className="relative">
                          <Thumb path={e ? thumbOf(e) : null} alt={r.name} className="w-full" />
                          <button
                            className="btn btn-circle btn-xs absolute top-1.5 right-1.5 opacity-80 hover:opacity-100"
                            aria-label={`Remove ${r.name}`}
                            onClick={() => setAircraft(aircraft.filter((x) => x.id !== r.id))}
                          >
                            <X className="size-3" />
                          </button>
                        </figure>
                        <div className="card-body gap-1 p-2.5">
                          <span
                            className="line-clamp-2 text-sm font-medium"
                            title={r.folders.join(", ")}
                          >
                            {r.name}
                          </span>
                          {e && <span className="badge badge-outline badge-xs">MSFS {e.sim}</span>}
                        </div>
                      </div>
                    );
                  })}
                  <button
                    id={TOUR_IDS.chooseAircraft}
                    className="card card-sm min-h-36 cursor-pointer items-center justify-center gap-2 rounded-box border-2 border-dashed border-base-content/20 text-base-content/40 transition-colors hover:border-primary/60 hover:bg-primary/5 hover:text-primary"
                    onClick={() => setPicking(true)}
                  >
                    <Plus className="size-8" strokeWidth={1.5} aria-hidden="true" />
                    <span className="text-sm font-medium">
                      {aircraft.length === 0 ? "Choose aircraft" : "Add or remove aircraft"}
                    </span>
                  </button>
                </div>
                <div className="collapse-arrow collapse border border-base-300 bg-base-100">
                  <input type="checkbox" aria-label="Show pattern matching" />
                  <div className="collapse-title text-sm font-medium">
                    Advanced: match by pattern
                  </div>
                  <div className="collapse-content">
                    <fieldset className="fieldset">
                      <textarea
                        className="textarea w-full font-mono"
                        rows={3}
                        value={patterns.join("\n")}
                        onChange={(e) => setAircraft(aircraft, e.target.value.split("\n"))}
                        placeholder="*Fenix*"
                        aria-label="Patterns"
                      />
                      <p className="label whitespace-normal">
                        One wildcard pattern per line (* and ?), matched against the aircraft title,
                        livery and aircraft.cfg path. Example: *PMDG 737*
                      </p>
                    </fieldset>
                  </div>
                </div>
              </>
            )}
          </div>
        </div>
      </div>

      {/* Programs */}
      <div className="space-y-3">
        <div className="flex h-8 items-center justify-between">
          <h2 className="text-sm uppercase tracking-wide text-base-content/60">Programs</h2>
          <button
            className="btn btn-sm"
            onClick={() => set({ programs: [...rule.programs, newProgram()] })}
          >
            <Plus className="size-4" /> Add program
          </button>
        </div>
        {rule.programs.length === 0 && (
          <div className="rounded-box border border-dashed border-base-300 p-6 text-center text-sm text-base-content/60">
            No programs yet. Add the companion app this rule should start.
          </div>
        )}
        {rule.programs.map((p, i) => (
          <ProgramCard
            key={p.id}
            program={p}
            tourTarget={i === 0}
            running={status.programs.find((s) => s.ruleId === rule.id && s.programId === p.id)}
            onChange={(np) => updateProgram(i, np)}
            onRemove={() => set({ programs: rule.programs.filter((_, j) => j !== i) })}
          />
        ))}
      </div>

      {picking && (
        <AircraftPicker
          selected={aircraft}
          onCancel={() => setPicking(false)}
          onDone={(refs) => {
            setAircraft(refs);
            setPicking(false);
          }}
        />
      )}
    </div>
  );
}

function DeleteButton({ rule, onConfirm }: { rule: Rule; onConfirm: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const programs = rule.programs.filter((p) => p.exe.trim()).length;
  return (
    <>
      <button
        className="btn btn-ghost btn-sm text-error"
        onClick={() => dialog.current?.showModal()}
      >
        <Trash className="size-4" /> Delete
      </button>
      <dialog ref={dialog} className="modal" aria-labelledby="delete-rule-title">
        <div className="modal-box max-w-md">
          <div className="flex items-start gap-4">
            <div className="flex size-10 shrink-0 items-center justify-center rounded-full bg-error/15 text-error">
              <Trash className="size-5" aria-hidden="true" />
            </div>
            <div className="min-w-0">
              <h3 id="delete-rule-title" className="text-lg">
                Delete rule?
              </h3>
              <p className="py-2 text-base-content/70">
                <span className="font-medium text-base-content">
                  {rule.name || "Untitled rule"}
                </span>
                {programs > 0 && ` and its ${programs} ${programs === 1 ? "program" : "programs"}`}{" "}
                will be removed. This can't be undone.
              </p>
            </div>
          </div>
          <div className="modal-action">
            <form method="dialog" className="flex gap-2">
              {/* Focused first, so Enter doesn't delete by accident. */}
              <button className="btn btn-ghost" autoFocus>
                Cancel
              </button>
              <button type="button" className="btn btn-error" onClick={onConfirm}>
                <Trash className="size-4" /> Delete rule
              </button>
            </form>
          </div>
        </div>
        <form method="dialog" className="modal-backdrop">
          <button aria-label="Close">close</button>
        </form>
      </dialog>
    </>
  );
}

const stateBadge: Record<ProgramStatus["state"], string> = {
  starting: "badge-warning",
  running: "badge-success",
  stopping: "badge-warning",
  exited: "border-base-content/15 bg-base-300",
};

function ProgramCard({
  program,
  tourTarget,
  running,
  onChange,
  onRemove,
}: {
  program: Program;
  /** The rule tour points at the first program card. */
  tourTarget?: boolean;
  running?: ProgramStatus;
  onChange: (p: Program) => void;
  onRemove: () => void;
}) {
  const [testMsg, setTestMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const set = (patch: Partial<Program>) => onChange({ ...program, ...patch });

  const browse = async () => {
    const f = await api.pickFile("exe");
    if (f) set({ exe: f });
  };
  const browseCwd = async () => {
    const f = await api.pickFile("folder");
    if (f) set({ cwd: f });
  };
  const test = async () => {
    setTestMsg({ ok: true, text: "Starting…" });
    try {
      const pid = await api.testLaunch(program);
      setTestMsg({ ok: true, text: pid ? `Started (pid ${pid})` : "Started" });
    } catch (e) {
      setTestMsg({ ok: false, text: String(e) });
    }
  };

  return (
    <div className="card card-border bg-base-200">
      <div className="card-body gap-3 p-5">
        <div className="flex items-center gap-2">
          <ProgramIcon path={program.exe} className="size-6" />
          <h3 className="card-title min-w-0 flex-1 truncate text-base">
            {program.exe ? fileName(program.exe) : "New program"}
          </h3>
          {running && (
            <span className={`badge badge-soft badge-sm capitalize ${stateBadge[running.state]}`}>
              {running.state}
            </span>
          )}
          <button
            id={tourTarget ? TOUR_IDS.programTest : undefined}
            className="btn btn-ghost btn-sm"
            onClick={test}
            disabled={!program.exe}
          >
            <Play className="size-4" /> Test
          </button>
          <button
            className="btn btn-ghost btn-sm btn-square text-error"
            onClick={onRemove}
            aria-label="Remove program"
          >
            <Trash className="size-4" />
          </button>
        </div>

        {testMsg && (
          <div
            role="status"
            className={`alert alert-soft py-2 text-sm ${testMsg.ok ? "alert-info" : "alert-error"}`}
          >
            {testMsg.text}
          </div>
        )}

        <fieldset id={tourTarget ? TOUR_IDS.programPath : undefined} className="fieldset">
          <legend className="fieldset-legend">Program</legend>
          <div className="join w-full">
            <input
              className="input join-item w-full font-mono text-sm"
              value={program.exe}
              onChange={(e) => set({ exe: e.target.value })}
              placeholder="C:\Path\To\Companion.exe (or .lnk / .bat)"
              spellCheck={false}
            />
            <button className="btn join-item" onClick={browse}>
              <FolderOpen className="size-4" /> Browse
            </button>
          </div>
        </fieldset>

        <div className="grid gap-x-4 sm:grid-cols-[1fr_1fr_8rem]">
          <fieldset className="fieldset">
            <legend className="fieldset-legend">Arguments</legend>
            <input
              className="input w-full font-mono text-sm"
              value={program.args}
              onChange={(e) => set({ args: e.target.value })}
              spellCheck={false}
            />
          </fieldset>
          <fieldset className="fieldset">
            <legend className="fieldset-legend">Working folder</legend>
            <div className="join w-full">
              <input
                className="input join-item w-full font-mono text-sm"
                value={program.cwd}
                onChange={(e) => set({ cwd: e.target.value })}
                placeholder="Program's folder"
                spellCheck={false}
              />
              <button
                className="btn join-item btn-square"
                onClick={browseCwd}
                aria-label="Choose working folder"
              >
                <FolderOpen className="size-4" />
              </button>
            </div>
          </fieldset>
          <fieldset className="fieldset">
            <legend className="fieldset-legend">Start delay</legend>
            <label className="input w-full">
              <input
                type="number"
                min={0}
                value={program.startDelaySecs}
                onChange={(e) =>
                  set({
                    startDelaySecs: Math.max(0, Number(e.target.value) || 0),
                  })
                }
              />
              <span className="label">s</span>
            </label>
          </fieldset>
        </div>

        <div
          id={tourTarget ? TOUR_IDS.programOptions : undefined}
          className="flex flex-wrap items-center gap-x-6 gap-y-2"
        >
          <label className="label cursor-pointer gap-2 text-base-content">
            <input
              type="checkbox"
              className="checkbox checkbox-primary checkbox-sm"
              checked={program.closeOnStop}
              onChange={(e) => set({ closeOnStop: e.target.checked })}
            />
            Close when the rule stops
          </label>
          {program.closeOnStop && (
            <select
              className="select select-sm w-auto"
              value={program.closeMode}
              onChange={(e) => set({ closeMode: e.target.value as Program["closeMode"] })}
              aria-label="How to close"
            >
              <option value="graceful">Ask it to close, then force</option>
              <option value="kill">Force close immediately</option>
            </select>
          )}
          <label
            className="label cursor-pointer gap-2 text-base-content"
            title="Shows a UAC prompt each time"
          >
            <input
              type="checkbox"
              className="checkbox checkbox-primary checkbox-sm"
              checked={program.runAsAdmin}
              onChange={(e) => set({ runAsAdmin: e.target.checked })}
            />
            Run as administrator
          </label>
        </div>

        {program.runAsAdmin && (
          <div role="alert" className="alert alert-warning alert-soft text-sm">
            <TriangleExclamationPoint className="size-4" />
            <span>
              Windows will ask for permission each time this starts, and MSFS Autostart can't close
              it unless it also runs as administrator.
            </span>
          </div>
        )}
      </div>
    </div>
  );
}
