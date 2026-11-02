import type * as React from "react";
import { BatteryFull, Circle, Search, Wifi } from "lucide-react";

import { cn } from "@starter/ui/lib/utils";

import { formatClock } from "../lib/format";
import { useEngine } from "../lib/engine";
import { MockScreen } from "./mock-screen";

/** Height of the fake macOS menu bar. Floating surfaces anchor below it. */
export const MENUBAR_H = 28;

/**
 * SOURCE OF TRUTH KEYWORDS: DesktopBackdrop, fake desktop, mac menu bar, status item, prototype
 * WHAT:  A fake Mac desktop with a menu bar and Cortex's status item, used behind
 *        floating surfaces (panel, HUD, toasts, countdown, region picker).
 * WHY:   Floating surfaces only make sense in context: over other apps, under the menu bar.
 *        The status item shows the recording clock like the real NSStatusItem will.
 * WHERE: prototype shell only.
 */
export function DesktopBackdrop({
  children,
  onStatusItem,
  statusItemActive,
}: {
  children?: React.ReactNode;
  onStatusItem: () => void;
  statusItemActive: boolean;
}) {
  const e = useEngine();
  const recording = e.status === "recording" || e.status === "paused";
  return (
    <div className="relative size-full overflow-hidden">
      <MockScreen variant="desktop" className="absolute inset-0" />
      <div
        className="absolute inset-x-0 top-0 z-40 flex items-center gap-4 bg-card/75 px-4 text-[13px] text-foreground backdrop-blur-xl"
        style={{ height: MENUBAR_H }}
      >
        <span className="font-semibold">Figma</span>
        <span className="text-muted-foreground">File</span>
        <span className="text-muted-foreground">Edit</span>
        <span className="text-muted-foreground">View</span>
        <div className="ml-auto flex items-center gap-3">
          <button
            type="button"
            onClick={onStatusItem}
            aria-label="Cortex menu bar item"
            className={cn(
              "flex h-[22px] items-center gap-1.5 rounded-[5px] px-1.5 transition-colors hover:bg-foreground/10",
              statusItemActive && "bg-foreground/15",
            )}
          >
            {recording ? (
              <>
                <span className={cn("size-2 rounded-full", e.status === "paused" ? "bg-warn" : "bg-rec")} />
                <span className="tabular text-[12.5px] font-medium">{formatClock(e.elapsedSecs)}</span>
              </>
            ) : (
              <Circle className="size-3.5" strokeWidth={2.6} />
            )}
          </button>
          <Wifi className="size-4" />
          <BatteryFull className="size-4" />
          <Search className="size-3.5" />
          <span className="tabular">Mon 9:41</span>
        </div>
      </div>
      {children}
    </div>
  );
}
