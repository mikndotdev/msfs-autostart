import { Activity, ListChecks, Settings as SettingsIcon } from "lucide-react";
import { NextStepViewport } from "nextstepjs";
import { useRef } from "react";
import { Navigate, NavLink, Route, Routes, useLocation } from "react-router";
import TitleBar from "./components/TitleBar";
import ActivityPage from "./pages/Activity";
import RuleEditor from "./pages/RuleEditor";
import RulesPage from "./pages/Rules";
import SettingsPage from "./pages/Settings";
import SetupPage from "./pages/Setup";
import { useApp } from "./state";
import { TOUR_VIEWPORT, useTourFollowsScroll } from "./tour";

export default function App() {
  const { config } = useApp();
  const { pathname } = useLocation();
  const mainRef = useRef<HTMLElement>(null);
  useTourFollowsScroll(mainRef);
  if (!config) {
    return (
      <div className="flex h-full items-center justify-center">
        <span className="loading loading-spinner loading-lg text-primary" />
      </div>
    );
  }
  const inSetup = pathname === "/setup";
  // First launch: the setup wizard comes first.
  if (!config.settings.setupComplete && !inSetup) return <Navigate to="/setup" replace />;

  return (
    <div className="flex h-full flex-col">
      <TitleBar />
      <main ref={mainRef} className="flex-1 overflow-y-auto">
        {/* The tour measures and scrolls targets within this, since <main> is the scroll container. */}
        <NextStepViewport id={TOUR_VIEWPORT}>
          {/* Room for the fixed dock. Inside the viewport, so the tour's dimmed overlay
              (drawn within the viewport) also covers this space when scrolled down. */}
          <div className={inSetup ? "" : "pb-24"}>
            <Routes>
              <Route path="/" element={<Navigate to="/rules" replace />} />
              <Route path="/rules" element={<RulesPage />} />
              <Route path="/rules/:id" element={<RuleEditor />} />
              <Route path="/activity" element={<ActivityPage />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="/setup" element={<SetupPage />} />
              <Route path="*" element={<Navigate to="/rules" replace />} />
            </Routes>
          </div>
        </NextStepViewport>
      </main>
      {!inSetup && <Dock />}
    </div>
  );
}

function Dock() {
  const items = [
    { to: "/rules", label: "Rules", Icon: ListChecks },
    { to: "/activity", label: "Activity", Icon: Activity },
    { to: "/settings", label: "Settings", Icon: SettingsIcon },
  ];
  return (
    <nav className="dock dock-md border-t border-base-300 bg-base-200" aria-label="Main">
      {items.map(({ to, label, Icon }) => (
        <NavLink
          key={to}
          to={to}
          className={({ isActive }) => (isActive ? "dock-active text-primary" : "")}
        >
          <Icon className="size-5" aria-hidden="true" />
          <span className="dock-label">{label}</span>
        </NavLink>
      ))}
    </nav>
  );
}
