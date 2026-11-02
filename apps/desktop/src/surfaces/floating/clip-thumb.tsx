import { cn } from "@starter/ui/lib/utils";

import { formatClock } from "../../lib/format";
import { MockScreen } from "../../prototype/mock-screen";

/**
 * SOURCE OF TRUTH KEYWORDS: ClipThumb, recording thumbnail, clip tile, poster frame, duration chip
 * WHAT:  A 16:9 thumbnail tile for a saved clip with an optional duration chip.
 * WHY:   Toasts and the menu-bar panel identify a file by its picture first, its name second.
 *        The prototype draws a fake screen; the app will show the first-frame poster.
 * WHERE: ToastStack cards, MenuBarPanel "last recording" card.
 */
export function ClipThumb({
  durationSecs,
  width = 72,
  className,
}: {
  durationSecs?: number;
  /** Rendered width in px; height follows 16:9. */
  width?: number;
  className?: string;
}) {
  return (
    <div
      className={cn("relative shrink-0 overflow-hidden rounded-[8px] bg-stage ring-1 ring-border", className)}
      style={{ width, height: Math.round((width * 9) / 16) }}
      aria-hidden
    >
      <MockScreen variant="window" className="opacity-90" />
      {durationSecs !== undefined && (
        <span className="absolute right-1 bottom-1 rounded-[4px] bg-card/90 px-1 text-[9.5px] leading-[14px] font-medium text-foreground tabular">
          {formatClock(durationSecs)}
        </span>
      )}
    </div>
  );
}
