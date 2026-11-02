import { Radio } from "lucide-react";

import { cn } from "@starter/ui/lib/utils";

import type { EngineStatus } from "../../lib/types";

/** True while a file is being written (recording or paused). */
export function isCapturing(status: EngineStatus) {
  return status === "recording" || status === "paused";
}

/**
 * SOURCE OF TRUTH KEYWORDS: RecStatus, REC dot, PAUSED, status tag, recording indicator, floating surfaces
 * WHAT:  The small "● REC" / "● PAUSED" / "SAVING…" / "READY" tag used on floating surfaces.
 * WHY:   The panel, HUD and countdown must name the engine state with the same words and
 *        colours (red only while recording, amber when paused), matching Rec's toolbar pill.
 * WHERE: MenuBarPanel, HudSurface, CountdownSurface.
 */
export function RecStatus({ status, className }: { status: EngineStatus; className?: string }) {
  const label =
    status === "recording"
      ? "Rec"
      : status === "paused"
        ? "Paused"
        : status === "finalizing"
          ? "Saving…"
          : status === "countdown"
            ? "Starting"
            : "Ready";
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 text-[10.5px] font-semibold tracking-[0.08em] uppercase",
        status === "recording" ? "text-rec" : status === "paused" ? "text-warn" : "text-muted-foreground",
        className,
      )}
    >
      <span
        aria-hidden
        className={cn(
          "size-1.5 rounded-full",
          status === "recording" && "animate-rec-pulse bg-rec",
          status === "paused" && "bg-warn",
          status !== "recording" && status !== "paused" && "bg-muted-foreground/50",
        )}
      />
      {label}
    </span>
  );
}

/**
 * SOURCE OF TRUTH KEYWORDS: LiveBadge, LIVE pill, streaming indicator, go live, STR-09
 * WHAT:  A compact "LIVE" pill shown while streaming.
 * WHY:   Streaming is a separate state from recording; it must be visible on every
 *        floating surface without borrowing the record red.
 * WHERE: MenuBarPanel header, HudSurface status line.
 */
export function LiveBadge({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        "inline-flex h-[18px] items-center gap-1 rounded-full bg-accent-tint px-1.5 text-[10px] font-semibold tracking-[0.08em] text-accent-text uppercase",
        className,
      )}
    >
      <Radio className="size-3" aria-hidden />
      Live
    </span>
  );
}
