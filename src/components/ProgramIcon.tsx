import { AppWindow } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../types";

// Icons are looked up once per path for the whole session. Misses aren't cached,
// so a path that's still being typed (or a program installed later) gets retried.
const cache = new Map<string, string>();
const pending = new Map<string, Promise<string | null>>();

function loadIcon(path: string): Promise<string | null> {
  const key = path.trim().toLowerCase();
  const hit = cache.get(key);
  if (hit) return Promise.resolve(hit);
  let p = pending.get(key);
  if (!p) {
    p = api
      .getProgramIcon(path)
      .catch(() => null)
      .then((url) => {
        pending.delete(key);
        if (url) cache.set(key, url);
        return url;
      });
    pending.set(key, p);
  }
  return p;
}

/** The program's own Windows icon, or a generic app icon if it has none. */
export default function ProgramIcon({
  path,
  className = "size-5",
}: {
  path: string;
  className?: string;
}) {
  const [url, setUrl] = useState<string | null>(() => cache.get(path.trim().toLowerCase()) ?? null);

  useEffect(() => {
    if (!path.trim()) {
      setUrl(null);
      return;
    }
    let live = true;
    // Small delay so typing a path doesn't query every keystroke.
    const t = window.setTimeout(() => loadIcon(path).then((u) => live && setUrl(u)), 250);
    return () => {
      live = false;
      window.clearTimeout(t);
    };
  }, [path]);

  if (!url) return <AppWindow className={`${className} shrink-0 opacity-50`} aria-hidden="true" />;
  return (
    <img src={url} alt="" className={`${className} shrink-0 object-contain`} draggable={false} />
  );
}
