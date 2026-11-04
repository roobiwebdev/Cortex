import type * as React from "react";
import { AppWindow, LayoutGrid, Monitor, SquareDashed } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Kbd } from "@starter/ui/components/kbd";
import { Segmented } from "@starter/ui/components/segmented";
import { cn } from "@starter/ui/lib/utils";

export type PickMode = "area" | "window" | "app" | "display";

const MODES: ReadonlyArray<{ value: PickMode; label: React.ReactNode; title: string }> = [
  {
    value: "area",
    label: (
      <>
        <SquareDashed />
        Area
      </>
    ),
    title: "Draw an area",
  },
  {
    value: "window",
    label: (
      <>
        <AppWindow />
        Window
      </>
    ),
    title: "Record one window",
  },
  {
    value: "app",
    label: (
      <>
        <LayoutGrid />
        App
      </>
    ),
    title: "Record every window of an app",
  },
  {
    value: "display",
    label: (
      <>
        <Monitor />
        Full screen
      </>
    ),
    title: "Record the whole display",
  },
];

/**
 * SOURCE OF TRUTH KEYWORDS: RegionToolbar, picker toolbar, capture mode, Area Window App Full screen, confirm region
 * WHAT:  The floating bottom-centre shell of the region picker: mode switch, Cancel/confirm,
 *        a contextual options tier (children) and a one-line hint.
 * WHY:   One anchored place for every decision keeps the desktop itself free for selecting.
 *        It fades while dragging so it never hides what is being selected.
 * WHERE: RegionPicker.
 */
export function RegionToolbar({
  mode,
  modes,
  onMode,
  primaryLabel,
  canConfirm,
  onConfirm,
  onCancel,
  hint,
  receded,
  onPointerEnter,
  children,
}: {
  mode: PickMode;
  /** The modes this engine can record; the others are not offered. */
  modes: readonly PickMode[];
  onMode: (m: PickMode) => void;
  primaryLabel: string;
  canConfirm: boolean;
  onConfirm: () => void;
  onCancel: () => void;
  hint: string;
  receded: boolean;
  /** Pointer moved onto the chrome: the overlay hides its crosshair and hover outline. */
  onPointerEnter: () => void;
  children: React.ReactNode;
}) {
  return (
    <div
      className={cn(
        "pointer-events-none absolute inset-x-0 bottom-6 flex flex-col items-center gap-2 px-4 transition-opacity duration-200 ease-out-quick",
        receded && "opacity-25",
      )}
    >
      <div
        role="toolbar"
        aria-label="Choose what to record"
        // Clicks on the toolbar must never start a selection underneath it.
        onPointerDown={(e) => e.stopPropagation()}
        onPointerMove={(e) => e.stopPropagation()}
        onPointerEnter={onPointerEnter}
        className={cn(
          "w-full max-w-[780px] rounded-[18px] bg-popover p-2 text-popover-foreground shadow-float",
          !receded && "pointer-events-auto",
        )}
      >
        <div className="flex items-center gap-2">
          <Segmented<PickMode>
            aria-label="Capture mode"
            value={mode}
            onValueChange={onMode}
            options={MODES.filter((m) => modes.includes(m.value))}
          />
          <div className="ml-auto flex items-center gap-1.5">
            <Button variant="ghost" onClick={onCancel} className="text-muted-foreground">
              Cancel
              <Kbd className="ml-0.5">Esc</Kbd>
            </Button>
            <Button variant="tactile" onClick={onConfirm} disabled={!canConfirm} className="min-w-32">
              {primaryLabel}
              <span aria-hidden className="text-[12px] opacity-70">
                ↵
              </span>
            </Button>
          </div>
        </div>
        <div className="mt-2 border-t px-1 pt-2">{children}</div>
      </div>
      <p className="rounded-full bg-popover/85 px-3 py-1 text-[11.5px] text-muted-foreground shadow-float backdrop-blur">
        {hint}
      </p>
    </div>
  );
}
