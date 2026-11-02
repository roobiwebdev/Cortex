import * as React from "react";

import { cn } from "@starter/ui/lib/utils";

const SEGMENTS = 24;
// Segment thresholds as a fraction of full scale: green below, amber, then red.
const MID_AT = 0.7;
const HIGH_AT = 0.88;
// Live meters hold their peak tick and let it fall at this rate (full scale per second).
const PEAK_DECAY_PER_SEC = 0.36;
// Screen readers do not need 30 updates a second; the value attribute follows at ~4 Hz.
const ARIA_INTERVAL_MS = 250;

/**
 * SOURCE OF TRUTH KEYWORDS: LevelSource, live levels, level store, meter source, subscribe, read
 * WHAT:  A store a live `LevelMeter` reads from: `subscribe` registers a change callback and
 *        returns an unsubscribe; `read` returns the current level and held peak (0..1).
 * WHY:   Audio levels change ~30 times a second. Reading them from a store lets the meter
 *        draw straight into the DOM, so no React component re-renders per level tick.
 * WHERE: desktop `lib/engine.ts` (`micLevels`, `systemLevels`); any meter fed by a stream.
 */
export interface LevelSource {
  subscribe(onChange: () => void): () => void;
  read(): { level: number; peak?: number };
}

type LevelMeterProps = Omit<React.ComponentProps<"div">, "children"> & {
  muted?: boolean;
  segments?: number;
} & (
    | {
        /** Current level, 0..1 of full scale. */
        level: number;
        /** Held peak, 0..1. */
        peak?: number;
        source?: undefined;
      }
    | {
        /** Live levels: the meter subscribes and redraws itself without re-rendering. */
        source: LevelSource;
        level?: undefined;
        peak?: undefined;
      }
  );

/**
 * SOURCE OF TRUTH KEYWORDS: LevelMeter, audio meter, VU, peak hold, levels, segments
 * WHAT:  A segmented horizontal audio level meter with a peak-hold tick. Pass `level`/`peak`
 *        props, or a live `source` that the meter reads on each animation frame after a change.
 * WHY:   Meters must read at a glance and use fixed semantic colours (never the user accent),
 *        so recording levels always look the same. With a `source` only the segments whose
 *        state changed are touched (a data attribute), so a recording's level stream costs
 *        no React renders and repaints only the meter.
 * WHERE: desktop mixer strips, menu-bar panel, recording HUD, settings audio, onboarding.
 */
function LevelMeter({ level, peak, source, muted = false, segments = SEGMENTS, className, ...props }: LevelMeterProps) {
  const ref = React.useRef<HTMLDivElement>(null);
  const live = source !== undefined;
  const lit = live ? 0 : litCount(level ?? 0, segments);
  const peakIndex = live ? -1 : peakAt(peak, segments);

  React.useEffect(() => {
    const el = ref.current;
    if (!source || !el) return;
    return driveMeter(el, source, segments);
  }, [source, segments]);

  return (
    <div
      ref={ref}
      role="meter"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={live ? 0 : Math.round(clamp01(level ?? 0) * 100)}
      data-slot="level-meter"
      data-muted={muted || undefined}
      className={cn("flex h-2 items-stretch gap-[2px] data-muted:opacity-40", className)}
      {...props}
    >
      {Array.from({ length: segments }, (_, i) => {
        const at = (i + 1) / segments;
        const tone =
          at > HIGH_AT ? "data-on:bg-meter-high" : at > MID_AT ? "data-on:bg-meter-mid" : "data-on:bg-meter-low";
        const on = i < lit || i === peakIndex;
        return <span key={i} data-on={on || undefined} className={cn("flex-1 rounded-[1px] bg-muted", tone)} />;
      })}
    </div>
  );
}

/** Subscribes `el` (a rendered meter) to `source`; returns the cleanup. */
function driveMeter(el: HTMLDivElement, source: LevelSource, segments: number) {
  const cells = Array.from(el.children);
  // Start from what is on screen: a previous source may have lit some segments.
  const shown = cells.map((c) => c.hasAttribute("data-on"));
  let frame = 0;
  let held = 0;
  let heldAt = 0;
  let ariaAt = 0;
  let ariaValue = -1;

  const draw = (now: number) => {
    frame = 0;
    const { level, peak } = source.read();
    const v = clamp01(level);
    // Peak hold: the tick never falls faster than the decay rate, whatever the source sends.
    const decayed = held - (PEAK_DECAY_PER_SEC * (now - heldAt)) / 1000;
    held = Math.max(v, clamp01(peak ?? v), decayed);
    heldAt = now;
    const lit = litCount(v, segments);
    const peakIndex = peakAt(held, segments);
    for (let i = 0; i < cells.length; i++) {
      const on = i < lit || i === peakIndex;
      if (on === shown[i]) continue;
      shown[i] = on;
      const cell = cells[i];
      if (on) cell?.setAttribute("data-on", "");
      else cell?.removeAttribute("data-on");
    }
    const pct = Math.round(v * 100);
    if (pct !== ariaValue && now - ariaAt >= ARIA_INTERVAL_MS) {
      ariaValue = pct;
      ariaAt = now;
      el.setAttribute("aria-valuenow", String(pct));
    }
  };
  const schedule = () => {
    frame ||= requestAnimationFrame(draw);
  };
  const unsubscribe = source.subscribe(schedule);
  schedule();
  return () => {
    unsubscribe();
    if (frame) cancelAnimationFrame(frame);
  };
}

function litCount(level: number, segments: number) {
  return Math.round(clamp01(level) * segments);
}

function peakAt(peak: number | undefined, segments: number) {
  return peak === undefined ? -1 : Math.min(segments - 1, Math.round(clamp01(peak) * segments) - 1);
}

function clamp01(v: number) {
  return Math.max(0, Math.min(1, v));
}

export { LevelMeter };
