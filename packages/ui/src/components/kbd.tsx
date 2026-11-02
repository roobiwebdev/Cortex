import * as React from "react";

import { cn } from "@starter/ui/lib/utils";

/**
 * SOURCE OF TRUTH KEYWORDS: Kbd, keyboard shortcut, hotkey label, key cap, ⌘⇧
 * WHAT:  A small key-cap label for keyboard shortcuts.
 * WHY:   Shortcuts appear in menus, tooltips, settings and onboarding; one style.
 * WHERE: desktop record bar, settings shortcuts, menu-bar panel, tooltips.
 */
function Kbd({ className, ...props }: React.ComponentProps<"kbd">) {
  return (
    <kbd
      data-slot="kbd"
      className={cn(
        "inline-flex h-5 min-w-5 items-center justify-center gap-0.5 rounded-[6px] border bg-card px-1.5 font-sans text-[11px] font-medium text-muted-foreground shadow-ledge",
        className,
      )}
      {...props}
    />
  );
}

export { Kbd };
