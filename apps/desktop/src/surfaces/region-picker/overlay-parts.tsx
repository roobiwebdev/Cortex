import * as React from "react";

import { cn } from "@starter/ui/lib/utils";

import { AppIconTile } from "./app-icon";
import { HANDLES, type Handle } from "./geometry";

/** A rect in CSS pixels of the overlay (what the browser draws), not display pixels. */
export interface CssRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/**
 * SOURCE OF TRUTH KEYWORDS: DimLayer, region dim, cut-out mask, overlay scrim, selection hole
 * WHAT:  Dims the whole display except one or more cut-outs (the selection or chosen windows).
 * WHY:   An SVG mask handles any number of rounded holes (an app may own several windows),
 *        which the "four panes" trick cannot.
 * WHERE: RegionPicker overlay.
 */
export function DimLayer({ holes }: { holes: ReadonlyArray<CssRect & { radius?: number }> }) {
  const maskId = `rp-mask-${React.useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  return (
    <svg aria-hidden className="pointer-events-none absolute inset-0 size-full">
      <defs>
        <mask id={maskId}>
          <rect width="100%" height="100%" fill="white" />
          {holes.map((h, i) => (
            <rect key={i} x={h.left} y={h.top} width={h.width} height={h.height} rx={h.radius ?? 0} fill="black" />
          ))}
        </mask>
      </defs>
      <rect width="100%" height="100%" mask={`url(#${maskId})`} className="fill-stage/60" />
    </svg>
  );
}

const HANDLE_POS: Record<Handle, { left: string; top: string; cursor: string }> = {
  nw: { left: "0%", top: "0%", cursor: "nwse-resize" },
  n: { left: "50%", top: "0%", cursor: "ns-resize" },
  ne: { left: "100%", top: "0%", cursor: "nesw-resize" },
  e: { left: "100%", top: "50%", cursor: "ew-resize" },
  se: { left: "100%", top: "100%", cursor: "nwse-resize" },
  s: { left: "50%", top: "100%", cursor: "ns-resize" },
  sw: { left: "0%", top: "100%", cursor: "nesw-resize" },
  w: { left: "0%", top: "50%", cursor: "ew-resize" },
};

export const HANDLE_CURSOR: Record<Handle, string> = {
  nw: HANDLE_POS.nw.cursor,
  n: HANDLE_POS.n.cursor,
  ne: HANDLE_POS.ne.cursor,
  e: HANDLE_POS.e.cursor,
  se: HANDLE_POS.se.cursor,
  s: HANDLE_POS.s.cursor,
  sw: HANDLE_POS.sw.cursor,
  w: HANDLE_POS.w.cursor,
};

/**
 * SOURCE OF TRUTH KEYWORDS: SelectionFrame, region handles, resize handles, size label, W×H readout
 * WHAT:  The selected rectangle: hairline accent edge, 8 resize handles, thirds guides while
 *        adjusting, and a live size label in display pixels.
 * WHY:   Precise, readable feedback is what makes a region picker feel professional.
 * WHERE: RegionPicker (Area mode, and snapped windows before confirm).
 */
export function SelectionFrame({
  rect,
  sizeLabel,
  detail,
  snappedApp,
  adjusting,
  editable,
  containerHeight,
  radius = 0,
  onHandleDown,
}: {
  rect: CssRect;
  sizeLabel: string;
  detail?: string;
  snappedApp?: string;
  adjusting: boolean;
  editable: boolean;
  containerHeight: number;
  radius?: number;
  onHandleDown: (h: Handle, e: React.PointerEvent) => void;
}) {
  const small = rect.width < 64 || rect.height < 64;
  // Label sits above the rect; falls back to below, then inside, when the edge is near.
  const labelTop = rect.top >= 36 ? -32 : rect.top + rect.height + 36 <= containerHeight ? rect.height + 8 : 8;

  return (
    <div className="pointer-events-none absolute" style={rect}>
      <div
        className="absolute inset-0 outline-[1.5px] outline-accent-solid outline-solid"
        style={{ borderRadius: radius }}
      />
      {adjusting && (
        <div aria-hidden className="absolute inset-0">
          {[1 / 3, 2 / 3].map((f) => (
            <React.Fragment key={f}>
              <div className="absolute inset-y-0 w-px bg-accent-solid/35" style={{ left: `${f * 100}%` }} />
              <div className="absolute inset-x-0 h-px bg-accent-solid/35" style={{ top: `${f * 100}%` }} />
            </React.Fragment>
          ))}
        </div>
      )}

      <div
        className="absolute left-0 flex h-6 items-center gap-1.5 rounded-[7px] bg-popover px-2 text-[11.5px] font-medium whitespace-nowrap text-popover-foreground shadow-float"
        style={{ top: labelTop }}
      >
        {snappedApp && <AppIconTile app={snappedApp} size={16} className="shadow-none" />}
        <span className="tabular">{sizeLabel}</span>
        {detail && <span className="text-muted-foreground">{detail}</span>}
      </div>

      {editable &&
        HANDLES.filter((h) => !small || h.length === 2).map((h) => {
          const pos = HANDLE_POS[h];
          const corner = h.length === 2;
          return (
            <div
              key={h}
              aria-hidden
              onPointerDown={(e) => onHandleDown(h, e)}
              className="pointer-events-auto absolute grid size-6 -translate-x-1/2 -translate-y-1/2 place-items-center"
              style={{ left: pos.left, top: pos.top, cursor: pos.cursor }}
            >
              <span
                className={cn(
                  "rounded-full bg-card shadow-ledge ring-[1.5px] ring-accent-solid",
                  corner ? "size-2.5" : h === "n" || h === "s" ? "h-1.5 w-4" : "h-4 w-1.5",
                )}
              />
            </div>
          );
        })}
    </div>
  );
}

/**
 * SOURCE OF TRUTH KEYWORDS: SnapHighlight, window snapping, hover window, window outline, pick window
 * WHAT:  Accent outline and a centred name card over the window under the pointer.
 * WHY:   Clicking a window is faster and more exact than dragging around it (NEW-RG-02).
 * WHERE: RegionPicker in Area, Window and App modes.
 */
export function SnapHighlight({
  rect,
  radius,
  app,
  title,
  hint,
}: {
  rect: CssRect;
  radius: number;
  app: string;
  title: string;
  hint: string;
}) {
  return (
    <div
      className="pointer-events-none absolute grid place-items-center border-2 border-accent-solid bg-accent-solid/10 transition-[left,top,width,height] duration-150 ease-out-quick"
      style={{ ...rect, borderRadius: radius }}
    >
      <div className="flex max-w-[80%] items-center gap-2.5 rounded-[14px] bg-popover py-2 pr-3.5 pl-2 text-popover-foreground shadow-float">
        <AppIconTile app={app} size={30} />
        <div className="min-w-0">
          <div className="truncate text-[13px] font-semibold">
            {app} <span className="font-normal text-muted-foreground">— {title}</span>
          </div>
          <div className="text-[11.5px] text-muted-foreground">{hint}</div>
        </div>
      </div>
    </div>
  );
}

/**
 * SOURCE OF TRUTH KEYWORDS: Crosshair, pointer guides, coordinates readout, region start
 * WHAT:  Full-length hairlines through the pointer with its display-pixel coordinates.
 * WHY:   Lets people start a drag on an exact pixel before anything is selected.
 * WHERE: RegionPicker, Area mode with no selection.
 */
export function Crosshair({ x, y, label }: { x: number; y: number; label: string }) {
  return (
    <div aria-hidden className="pointer-events-none absolute inset-0">
      <div className="absolute inset-y-0 w-px bg-accent-solid/60" style={{ left: x }} />
      <div className="absolute inset-x-0 h-px bg-accent-solid/60" style={{ top: y }} />
      <div
        className="tabular absolute rounded-[6px] bg-popover px-1.5 py-0.5 text-[11px] font-medium text-popover-foreground shadow-float"
        style={{ left: x + 12, top: y + 12 }}
      >
        {label}
      </div>
    </div>
  );
}
