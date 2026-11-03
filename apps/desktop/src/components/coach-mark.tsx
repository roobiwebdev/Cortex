import type * as React from "react";
import { Lightbulb, X } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { cn } from "@starter/ui/lib/utils";

import { engine, useEngine } from "../lib/engine";
import type { CoachHint, EngineState } from "../lib/types";

const seenOf = (s: EngineState) => s.settings.seenHints;

/**
 * SOURCE OF TRUTH KEYWORDS: CoachMark, coach mark, first-use tip, seenHints, dismissible hint, onboarding tip
 * WHAT:  A small dismissible tip card shown the first time a feature is opened. Dismissing
 *        it adds its hint to `settings.seenHints`, so it never shows again (Settings › General
 *        › Show tips again brings them back).
 * WHY:   The tour can't cover every gesture. A tip at the moment of use teaches the rest,
 *        once, without a manual. Stored in settings so every window agrees.
 * WHERE: region picker (area picking), Quick trim, the Go live popover. Position it with
 *        `className`; it swallows pointer events so overlays under it don't react.
 */
export function CoachMark({
  hint,
  title,
  children,
  className,
}: {
  hint: CoachHint;
  title: string;
  children: React.ReactNode;
  className?: string;
}) {
  const seen = useEngine(seenOf);
  if (seen.includes(hint)) return null;
  const dismiss = () => engine.update({ seenHints: [...seen, hint] });
  return (
    <div
      role="note"
      aria-label={title}
      onPointerDown={(ev) => ev.stopPropagation()}
      className={cn(
        "z-40 flex w-[280px] cursor-default gap-2.5 rounded-[14px] border bg-popover p-3 text-popover-foreground shadow-float",
        "animate-in duration-300 ease-out-quick fade-in-0 slide-in-from-bottom-1",
        className,
      )}
    >
      <span className="grid size-7 shrink-0 place-items-center rounded-[8px] bg-accent-tint text-accent-text">
        <Lightbulb className="size-3.5" />
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex items-start justify-between gap-2">
          <p className="pt-0.5 text-[13px] leading-5 font-semibold">{title}</p>
          <button
            type="button"
            aria-label="Dismiss tip"
            onClick={dismiss}
            className="-mt-0.5 -mr-1 grid size-6 shrink-0 place-items-center rounded-full text-muted-foreground outline-none hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
          >
            <X className="size-3.5" />
          </button>
        </div>
        <div className="mt-0.5 text-[12px] leading-relaxed text-muted-foreground">{children}</div>
        <Button variant="soft" size="xs" className="mt-2" onClick={dismiss}>
          Got it
        </Button>
      </div>
    </div>
  );
}
