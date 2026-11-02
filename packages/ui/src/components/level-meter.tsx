import * as React from "react";

import { cn } from "@starter/ui/lib/utils";

const SEGMENTS = 24;
// Segment thresholds as a fraction of full scale: green below, amber, then red.
const MID_AT = 0.7;
const HIGH_AT = 0.88;

/**
 * SOURCE OF TRUTH KEYWORDS: LevelMeter, audio meter, VU, peak hold, levels, segments
 * WHAT:  A segmented horizontal audio level meter with a peak-hold tick.
 * WHY:   Meters must read at a glance and use fixed semantic colours (never the
 *        user accent), so recording levels always look the same.
 * WHERE: desktop mixer strips, menu-bar panel, recording HUD.
 */
function LevelMeter({
  level,
  peak,
  muted = false,
  segments = SEGMENTS,
  className,
  ...props
}: Omit<React.ComponentProps<"div">, "children"> & {
  /** Current level, 0..1 of full scale. */
  level: number;
  /** Held peak, 0..1. */
  peak?: number;
  muted?: boolean;
  segments?: number;
}) {
  const lit = Math.round(clamp01(level) * segments);
  const peakIndex = peak === undefined ? -1 : Math.min(segments - 1, Math.round(clamp01(peak) * segments) - 1);

  return (
    <div
      role="meter"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(clamp01(level) * 100)}
      data-slot="level-meter"
      data-muted={muted || undefined}
      className={cn("flex h-2 items-stretch gap-[2px] data-muted:opacity-40", className)}
      {...props}
    >
      {Array.from({ length: segments }, (_, i) => {
        const at = (i + 1) / segments;
        const tone = at > HIGH_AT ? "bg-meter-high" : at > MID_AT ? "bg-meter-mid" : "bg-meter-low";
        const on = i < lit || i === peakIndex;
        return <span key={i} className={cn("flex-1 rounded-[1px]", on ? tone : "bg-muted")} />;
      })}
    </div>
  );
}

function clamp01(v: number) {
  return Math.max(0, Math.min(1, v));
}

export { LevelMeter };
