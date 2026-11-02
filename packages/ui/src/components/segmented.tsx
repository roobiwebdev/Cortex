import * as React from "react";

import { cn } from "@starter/ui/lib/utils";

/**
 * SOURCE OF TRUTH KEYWORDS: Segmented, segmented control, toggle group, radio pills, picker
 * WHAT:  A single-choice segmented control (sunken track, raised selected pill).
 * WHY:   Short exclusive choices (scene, capture target, shape, theme) read faster
 *        as visible segments than as a dropdown.
 * WHERE: desktop sidebar, camera overlay controls, settings rows, menu-bar panel.
 */
function Segmented<T extends string>({
  value,
  onValueChange,
  options,
  size = "default",
  className,
  "aria-label": ariaLabel,
}: {
  value: T;
  onValueChange: (value: T) => void;
  options: ReadonlyArray<{ value: T; label: React.ReactNode; title?: string }>;
  size?: "sm" | "default";
  className?: string;
  "aria-label"?: string;
}) {
  return (
    <div
      role="radiogroup"
      aria-label={ariaLabel}
      data-slot="segmented"
      data-size={size}
      className={cn(
        "inline-flex items-center gap-0.5 rounded-[calc(var(--radius)-4px)] bg-muted p-0.5 text-muted-foreground",
        className,
      )}
    >
      {options.map((o) => {
        const selected = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={selected}
            title={o.title}
            onClick={() => onValueChange(o.value)}
            className={cn(
              "inline-flex flex-1 items-center justify-center gap-1.5 rounded-[calc(var(--radius)-6px)] font-medium whitespace-nowrap transition-[background,color,box-shadow] duration-150 outline-none focus-visible:ring-3 focus-visible:ring-ring/50 [&_svg]:size-3.5",
              size === "sm" ? "h-6 px-2 text-xs" : "h-7 px-2.5 text-[13px]",
              selected ? "bg-card text-foreground shadow-ledge dark:bg-input" : "hover:text-foreground",
            )}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

export { Segmented };
