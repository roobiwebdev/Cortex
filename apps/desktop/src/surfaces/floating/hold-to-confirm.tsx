import * as React from "react";

import { cn } from "@starter/ui/lib/utils";

/**
 * SOURCE OF TRUTH KEYWORDS: HoldToConfirm, hold to discard, long press, destructive confirm, progress fill
 * WHAT:  A button that fires only after being held (pointer or Space/Enter) for `holdMs`;
 *        a fill shows progress and releasing early cancels.
 * WHY:   Discarding a recording cannot be undone. A hold is faster than a dialog and
 *        cannot be triggered by a stray click on a floating surface.
 * WHERE: HudSurface discard confirm (and later the menu-bar panel).
 */
export function HoldToConfirm({
  onConfirm,
  holdMs = 1200,
  children,
  className,
  "aria-label": ariaLabel,
}: {
  onConfirm: () => void;
  holdMs?: number;
  children: React.ReactNode;
  className?: string;
  "aria-label"?: string;
}) {
  const [progress, setProgress] = React.useState(0);
  const frame = React.useRef<number | null>(null);
  const startedAt = React.useRef(0);

  const stop = React.useCallback(() => {
    if (frame.current !== null) cancelAnimationFrame(frame.current);
    frame.current = null;
    setProgress(0);
  }, []);

  const start = React.useCallback(() => {
    if (frame.current !== null) return;
    startedAt.current = performance.now();
    const tick = (nowMs: number) => {
      const p = Math.min(1, (nowMs - startedAt.current) / holdMs);
      setProgress(p);
      if (p >= 1) {
        frame.current = null;
        onConfirm();
        return;
      }
      frame.current = requestAnimationFrame(tick);
    };
    frame.current = requestAnimationFrame(tick);
  }, [holdMs, onConfirm]);

  React.useEffect(() => stop, [stop]);

  return (
    <button
      type="button"
      aria-label={ariaLabel}
      onPointerDown={start}
      onPointerUp={stop}
      onPointerLeave={stop}
      onPointerCancel={stop}
      onKeyDown={(ev) => {
        if ((ev.key === " " || ev.key === "Enter") && !ev.repeat) {
          ev.preventDefault();
          start();
        }
      }}
      onKeyUp={(ev) => {
        if (ev.key === " " || ev.key === "Enter") stop();
      }}
      className={cn(
        "relative isolate inline-flex h-8 items-center justify-center overflow-hidden rounded-[10px] bg-rec-tint px-3 text-[13px] font-semibold text-rec outline-none select-none focus-visible:ring-3 focus-visible:ring-rec/40",
        className,
      )}
    >
      <span aria-hidden className="absolute inset-y-0 left-0 -z-10 bg-rec/25" style={{ width: `${progress * 100}%` }} />
      {children}
    </button>
  );
}
