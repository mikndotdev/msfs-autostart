import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Pause, Plane, Play, Square, X } from "lucide-react";
import { useEffect, useState } from "react";
import appIcon from "../../src-tauri/icons/app-icon.svg";
import { useApp } from "../state";
import { Status, inTauri } from "../types";

/**
 * Replaces the native Windows title bar (`decorations: false`): app icon, sim status,
 * pause, and window controls. The whole bar is a drag region (double-click maximizes);
 * buttons inside it stay clickable.
 */
export default function TitleBar() {
  const { status, config, updateConfig } = useApp();
  const maximized = useMaximized();
  if (!config) return null;
  const paused = config.settings.paused;

  const pill = pillState(status);
  const running = status.programs.filter((p) => p.alive).length;
  const win = inTauri ? getCurrentWindow() : null;

  return (
    <header
      data-tauri-drag-region="deep"
      className="flex h-12 shrink-0 select-none items-stretch border-b border-base-300 bg-base-200"
    >
      <div className="flex shrink-0 items-center gap-3 pr-1 pl-3">
        <img src={appIcon} alt="" className="size-6 rounded-md" draggable={false} />
        <span className="hidden text-sm font-bold whitespace-nowrap text-base-content/90 md:inline">
          MSFS Autostart
        </span>
      </div>

      {/* Status pill spanning the bar: its colour is the sim status. Running count on the
          left, status in the middle, pause on the right. */}
      <div className="flex min-w-0 flex-1 items-center px-2">
        <div
          role="status"
          title={pill.title}
          className={`grid h-8 w-full grid-cols-[minmax(0,1fr)_minmax(0,auto)_minmax(0,1fr)] items-center gap-2 rounded-full border pr-1 pl-4 text-sm font-medium transition-colors duration-300 ${pill.tone}`}
        >
          <span className="truncate text-xs opacity-75">
            {running > 0 ? `${running} running` : ""}
          </span>
          <span className="flex min-w-0 items-center gap-2">
            {pill.icon}
            <span className="truncate">{pill.text}</span>
          </span>
          <button
            className={`btn btn-xs justify-self-end rounded-full ${
              paused ? "btn-warning" : "btn-ghost text-current hover:bg-current/10"
            }`}
            onClick={() =>
              updateConfig({ ...config, settings: { ...config.settings, paused: !paused } })
            }
            title={paused ? "Resume launching programs" : "Stop launching programs until resumed"}
          >
            {paused ? <Play className="size-3.5" /> : <Pause className="size-3.5" />}
            {paused ? "Resume" : "Pause"}
          </button>
        </div>
      </div>

      {/* Windows-style caption buttons, flush with the top-right corner. */}
      <div className="flex">
        <CaptionButton label="Minimize" onClick={() => win?.minimize()}>
          <Minus className="size-4" strokeWidth={1.5} />
        </CaptionButton>
        <CaptionButton
          label={maximized ? "Restore" : "Maximize"}
          onClick={() => win?.toggleMaximize()}
        >
          {maximized ? (
            <Copy className="size-3.5 -scale-x-100" strokeWidth={1.5} />
          ) : (
            <Square className="size-3.5" strokeWidth={1.5} />
          )}
        </CaptionButton>
        {/* Closing hides to the tray (handled by the backend's CloseRequested hook). */}
        <CaptionButton label="Close to tray" onClick={() => win?.close()} danger>
          <X className="size-4" strokeWidth={1.5} />
        </CaptionButton>
      </div>
    </header>
  );
}

// Full class strings so Tailwind picks them up.
const tones = {
  idle: "border-base-content/10 bg-base-300 text-base-content/60",
  starting: "border-warning/30 bg-warning/15 text-warning",
  error: "border-error/30 bg-error/15 text-error",
  menus: "border-info/30 bg-info/15 text-info",
  flying: "border-success/30 bg-success/15 text-success",
};

/** SimConnect's application name is the sim's internal codename. */
function simLabel(name: string) {
  if (/sunrise/i.test(name)) return "MSFS 2024";
  if (/kittyhawk/i.test(name)) return "MSFS 2020";
  return name || "MSFS";
}

function pillState(s: Status): {
  tone: string;
  text: string;
  title?: string;
  icon?: React.ReactNode;
} {
  if (s.connected && s.inFlight) {
    return {
      tone: tones.flying,
      text: s.aircraftTitle || "In flight",
      title: [simLabel(s.simName), s.aircraftTitle, s.liveryName].filter(Boolean).join(" · "),
      icon: <Plane className="size-4 shrink-0" aria-hidden="true" />,
    };
  }
  if (s.connected) return { tone: tones.menus, text: `${simLabel(s.simName)} · In menus` };
  if (s.lastError) return { tone: tones.error, text: s.lastError, title: s.lastError };
  if (s.simRunning) return { tone: tones.starting, text: "MSFS is starting…" };
  return { tone: tones.idle, text: "Waiting for MSFS" };
}

function CaptionButton({
  label,
  onClick,
  danger,
  children,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className={`flex w-12 items-center justify-center text-base-content/70 transition-colors focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-primary ${
        danger
          ? "hover:bg-error hover:text-error-content"
          : "hover:bg-base-content/10 hover:text-base-content"
      }`}
    >
      {children}
    </button>
  );
}

/** Tracks whether the window is maximized, to swap the maximize/restore icon. */
function useMaximized() {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    if (!inTauri) return;
    const win = getCurrentWindow();
    const update = () => win.isMaximized().then(setMaximized);
    update();
    const unlisten = win.onResized(update);
    return () => {
      unlisten.then((f) => f());
    };
  }, []);
  return maximized;
}
