// Guided tour (NextStep) that walks through creating a rule.
import type { CardComponentProps, NextStepProps } from "nextstepjs";
import { useNextStep } from "nextstepjs";
import { RefObject, useCallback, useEffect } from "react";
import { useNavigate } from "react-router";

export const RULE_TOUR = "createRule";

/** Id of the NextStepViewport around the routed pages (inside the scrolling <main>). */
export const TOUR_VIEWPORT = "tour-viewport";

/** Element ids the tour points at. */
export const TOUR_IDS = {
  newRule: "tour-new-rule",
  ruleName: "tour-rule-name",
  trigger: "tour-trigger",
  chooseAircraft: "tour-choose-aircraft",
  programPath: "tour-program-path",
  programOptions: "tour-program-options",
  programTest: "tour-program-test",
} as const;

/** Step index of "New rule": the user clicks it instead of Next (see Rules page). */
export const NEW_RULE_STEP = 0;

// Steps where the user acts on the highlighted element instead of pressing Next.
const INTERACTIVE_STEPS = new Set([NEW_RULE_STEP]);

const sel = (id: string) => `#${id}`;
// scrollOffset keeps targets clear of the title bar and the bottom dock when scrolled into view.
const common = {
  pointerPadding: 8,
  pointerRadius: 12,
  scrollOffset: 120,
  viewportID: TOUR_VIEWPORT,
  selectorRetryAttempts: 10,
  selectorRetryDelay: 150,
};

// Tours used with a custom card (that type isn't exported by nextstepjs).
type CustomCardTours = Extract<NextStepProps, { cardComponent: unknown }>["steps"];

export const tours: CustomCardTours = [
  {
    tour: RULE_TOUR,
    steps: [
      {
        ...common,
        title: "Create a rule",
        content: (
          <>
            A rule says <i>when</i> to start your companion programs. Click <b>New rule</b> to make
            one.
          </>
        ),
        selector: sel(TOUR_IDS.newRule),
        side: "bottom",
      },
      {
        ...common,
        title: "Name it",
        content: (
          <>
            Give the rule a name you'll recognise, like the add-on it's for. It saves automatically
            as you go.
          </>
        ),
        selector: sel(TOUR_IDS.ruleName),
        side: "bottom",
      },
      {
        ...common,
        title: "Choose when it runs",
        content: (
          <>
            <b>With the sim</b> runs the programs whenever MSFS is open.{" "}
            <b>With specific aircraft</b> runs them only once you're in the cockpit of one of your
            chosen aircraft.
          </>
        ),
        selector: sel(TOUR_IDS.trigger),
        side: "bottom",
      },
      {
        ...common,
        title: "Pick your aircraft",
        content: (
          <>
            Click here to choose the aircraft from everything installed. Tick one or more, then
            press <b>Done</b>. Come back and press <b>Next</b> when you're ready.
          </>
        ),
        selector: sel(TOUR_IDS.chooseAircraft),
        side: "right",
      },
      {
        ...common,
        title: "Add the companion program",
        content: (
          <>
            <b>Browse</b> to the program's <code>.exe</code>, or a shortcut. Add arguments if the
            program needs any.
          </>
        ),
        selector: sel(TOUR_IDS.programPath),
        side: "top",
      },
      {
        ...common,
        title: "Closing it again",
        content: (
          <>
            By default the program is closed when the rule stops: when you leave the flight or quit
            MSFS. Untick it to leave the program open.
          </>
        ),
        selector: sel(TOUR_IDS.programOptions),
        side: "top",
      },
      {
        ...common,
        title: "Try it",
        content: (
          <>
            Use <b>Test</b> to check the program starts correctly, without loading a flight.
          </>
        ),
        selector: sel(TOUR_IDS.programTest),
        side: "left",
      },
      {
        title: "That's it!",
        content: (
          <>
            Leave MSFS Autostart running (it lives in the tray), start a flight, and your programs
            open by themselves. You can watch what it's doing on the <b>Activity</b> page.
          </>
        ),
      },
    ],
  },
];

/**
 * Keeps the spotlight and card on their target while the user scrolls `scroller`.
 * NextStep only recalculates positions on window resize, so while a tour is showing,
 * each scroll (at most once per frame) fires a synthetic resize.
 */
export function useTourFollowsScroll(scroller: RefObject<HTMLElement | null>) {
  const { isNextStepVisible } = useNextStep();
  useEffect(() => {
    const el = scroller.current;
    if (!isNextStepVisible || !el) return;
    let frame = 0;
    const onScroll = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => window.dispatchEvent(new Event("resize")));
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      el.removeEventListener("scroll", onScroll);
      cancelAnimationFrame(frame);
    };
  }, [isNextStepVisible, scroller]);
}

const ruleSteps = tours[0].steps;

// Steps that only apply to some rules: skipped while their target isn't on the page.
// "Pick your aircraft" disappears when the rule runs "With the sim".
const OPTIONAL_STEPS = new Set(
  ruleSteps.flatMap((s, i) => (s.selector === sel(TOUR_IDS.chooseAircraft) ? [i] : [])),
);

/** True when step `i` should be skipped right now. Only judged on the rule editor,
 *  where all of these targets live (on other pages they're absent anyway). */
function isSkipped(i: number) {
  if (!OPTIONAL_STEPS.has(i) || !document.getElementById(TOUR_IDS.trigger)) return false;
  const selector = ruleSteps[i]?.selector;
  return !!selector && !document.querySelector(selector);
}

/** Opens the Rules page and starts the rule tour. */
export function useStartRuleTour() {
  const { startNextStep } = useNextStep();
  const navigate = useNavigate();
  return useCallback(() => {
    navigate("/rules");
    // Let the Rules page render before the tour looks for its first element.
    window.setTimeout(() => startNextStep(RULE_TOUR), 300);
  }, [navigate, startNextStep]);
}

/** Tour card in the app's daisyUI style. */
export function TourCard({
  step,
  currentStep,
  totalSteps,
  nextStep,
  prevStep,
  skipTour,
  arrow,
}: CardComponentProps) {
  const { setCurrentStep } = useNextStep();
  const interactive = INTERACTIVE_STEPS.has(currentStep);
  const last = currentStep === totalSteps - 1;

  // Move by one, jumping over steps that don't apply to this rule.
  const go = (dir: 1 | -1) => {
    let i = currentStep + dir;
    while (i > 0 && i < totalSteps - 1 && isSkipped(i)) i += dir;
    if (i === currentStep + dir) (dir === 1 ? nextStep : prevStep)();
    else setCurrentStep(i);
  };

  // Counter that leaves out skipped steps.
  const skippedBefore = Array.from({ length: currentStep }, (_, i) => i).filter(isSkipped).length;
  const skippedTotal = Array.from({ length: totalSteps }, (_, i) => i).filter(isSkipped).length;
  return (
    <div className="card w-80 border border-base-content/15 bg-base-200 shadow-2xl">
      <div className="card-body gap-3 p-5">
        <div className="flex items-baseline justify-between gap-3">
          <h3 className="card-title text-base">{step.title}</h3>
          <span className="shrink-0 text-xs text-base-content/50">
            {currentStep + 1 - skippedBefore} / {totalSteps - skippedTotal}
          </span>
        </div>
        <div className="text-sm leading-relaxed text-base-content/80">{step.content}</div>
        {arrow}
        <div className="mt-1 flex items-center gap-2">
          {skipTour && !last && (
            <button
              className="btn btn-ghost btn-sm mr-auto px-2 text-base-content/60"
              onClick={skipTour}
            >
              Skip tour
            </button>
          )}
          <div className="ml-auto flex gap-2">
            {currentStep > 0 && (
              <button className="btn btn-sm" onClick={() => go(-1)}>
                Back
              </button>
            )}
            {interactive ? (
              <span className="self-center text-xs text-primary">Click the highlighted item</span>
            ) : (
              <button className="btn btn-primary btn-sm" onClick={() => go(1)}>
                {last ? "Finish" : "Next"}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
