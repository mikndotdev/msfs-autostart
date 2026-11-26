import { convertFileSrc } from "@tauri-apps/api/core";
import { Plane } from "lucide-react";
import { useState } from "react";
import { inTauri } from "../types";

/** Aircraft thumbnail from a local file path, with a plane placeholder. */
export default function Thumb({
  path,
  alt,
  className = "",
}: {
  path: string | null;
  alt: string;
  className?: string;
}) {
  const [failed, setFailed] = useState(false);
  if (!path || failed || !inTauri) {
    return (
      <div
        className={`flex aspect-video items-center justify-center bg-base-300 text-base-content/30 ${className}`}
        role="img"
        aria-label={alt}
      >
        <Plane className="size-1/3 max-h-10 max-w-10" aria-hidden="true" />
      </div>
    );
  }
  return (
    <img
      className={`aspect-video object-cover ${className}`}
      src={convertFileSrc(path)}
      alt={alt}
      loading="lazy"
      onError={() => setFailed(true)}
    />
  );
}
