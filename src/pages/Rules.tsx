import { ListChecks, Monitor, Plane, Plus } from "lucide-react";
import { useNextStep } from "nextstepjs";
import { Link, useNavigate } from "react-router";
import ProgramIcon from "../components/ProgramIcon";
import Thumb from "../components/Thumb";
import { useConfig } from "../state";
import { NEW_RULE_STEP, RULE_TOUR, TOUR_IDS } from "../tour";
import { Rule, fileName, newRule, thumbOf } from "../types";

export default function RulesPage() {
  const { config, updateConfig, status, catalog, addDraftRule } = useConfig();
  const navigate = useNavigate();
  const { currentTour, currentStep, setCurrentStep } = useNextStep();

  const addRule = () => {
    const r = newRule();
    addDraftRule(r);
    navigate(`/rules/${r.id}`);
    // The tour's first step asks the user to click here; advance once the editor opens.
    if (currentTour === RULE_TOUR && currentStep === NEW_RULE_STEP)
      setCurrentStep(NEW_RULE_STEP + 1, 300);
  };
  const setRule = (r: Rule) =>
    updateConfig({ ...config, rules: config.rules.map((x) => (x.id === r.id ? r : x)) });

  if (config.rules.length === 0) {
    return (
      <div className="hero min-h-full">
        <div className="hero-content text-center">
          <div className="max-w-md">
            <ListChecks className="mx-auto mb-4 size-12 text-primary" aria-hidden="true" />
            <h1 className="text-2xl">No rules yet</h1>
            <p className="py-4 text-base-content/70">
              A rule starts companion programs with the sim, or with specific aircraft, and closes
              them afterwards.
            </p>
            <button id={TOUR_IDS.newRule} className="btn btn-primary" onClick={addRule}>
              <Plus className="size-4" /> Create your first rule
            </button>
          </div>
        </div>
      </div>
    );
  }

  const byId = new Map(catalog.aircraft.map((a) => [a.id, a]));

  return (
    <div className="mx-auto max-w-5xl p-6">
      <div className="grid gap-4 md:grid-cols-2">
        {config.rules.map((r) => {
          const active = status.activeRules.includes(r.id);
          const aircraft = r.trigger.type === "aircraft" ? r.trigger.aircraft : [];
          return (
            <div
              key={r.id}
              className={`card card-border relative bg-base-200 transition-colors hover:border-primary/50 hover:bg-base-300/40 focus-within:border-primary ${r.enabled ? "" : "opacity-60"}`}
            >
              <div className="card-body gap-3 p-5">
                <div className="flex items-start gap-3">
                  {/* Stretched link: its ::after covers the whole card, so clicking anywhere opens the editor. */}
                  <Link
                    to={`/rules/${r.id}`}
                    className="min-w-0 flex-1 outline-none after:absolute after:inset-0 after:rounded-box after:content-['']"
                  >
                    <h2 className="card-title truncate text-lg">
                      {r.name || "Untitled rule"}
                      {active && (
                        <span className="badge badge-success badge-soft badge-sm border-success/30">
                          Active
                        </span>
                      )}
                    </h2>
                  </Link>
                  <input
                    type="checkbox"
                    className="toggle toggle-primary toggle-sm relative z-10"
                    checked={r.enabled}
                    onChange={(e) => setRule({ ...r, enabled: e.target.checked })}
                    aria-label={`Enable ${r.name}`}
                  />
                </div>

                <div className="flex items-center gap-2 text-sm text-base-content/70">
                  {r.trigger.type === "sim" ? (
                    <>
                      <Monitor className="size-4" aria-hidden="true" /> With the sim
                    </>
                  ) : (
                    <>
                      <Plane className="size-4" aria-hidden="true" />
                      {aircraft.length === 0
                        ? "No aircraft selected"
                        : aircraft.length === 1
                          ? aircraft[0].name
                          : `${aircraft.length} aircraft`}
                    </>
                  )}
                </div>

                {aircraft.length > 0 && (
                  <div className="flex gap-2">
                    {aircraft.slice(0, 4).map((a) => {
                      const e = byId.get(a.id);
                      return (
                        <Thumb
                          key={a.id}
                          path={e ? thumbOf(e) : null}
                          alt={a.name}
                          className="w-20 rounded-md"
                        />
                      );
                    })}
                    {aircraft.length > 4 && (
                      <div className="flex w-20 items-center justify-center rounded-md bg-base-300 text-sm">
                        +{aircraft.length - 4}
                      </div>
                    )}
                  </div>
                )}

                <div className="flex flex-wrap gap-1.5">
                  {r.programs.length === 0 && (
                    <span className="text-sm text-base-content/50">No programs</span>
                  )}
                  {r.programs.map((p) => {
                    const running = status.programs.find(
                      (s) => s.ruleId === r.id && s.programId === p.id,
                    )?.alive;
                    return (
                      <span
                        key={p.id}
                        className={`badge badge-sm gap-1.5 pl-1 ${running ? "badge-success badge-soft border-success/30" : "border-base-content/15 bg-base-300"}`}
                      >
                        <ProgramIcon path={p.exe} className="size-3.5" />
                        {p.exe ? fileName(p.exe) : "New program"}
                      </span>
                    );
                  })}
                </div>
              </div>
            </div>
          );
        })}

        <button
          id={TOUR_IDS.newRule}
          className="card min-h-48 cursor-pointer items-center justify-center gap-2 rounded-box border-2 border-dashed border-base-content/20 text-base-content/40 transition-colors hover:border-primary/60 hover:bg-primary/5 hover:text-primary"
          onClick={addRule}
          aria-label="New rule"
        >
          <Plus className="size-10" strokeWidth={1.5} aria-hidden="true" />
          <span className="text-sm font-medium">New rule</span>
        </button>
      </div>
    </div>
  );
}
