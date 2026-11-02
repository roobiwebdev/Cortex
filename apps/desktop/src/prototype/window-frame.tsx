import type * as React from "react";

import { cn } from "@starter/ui/lib/utils";

/**
 * SOURCE OF TRUTH KEYWORDS: WindowFrame, mac window chrome, traffic lights, hiddenInset title bar
 * WHAT:  Draws a macOS-style window (rounded, shadowed, traffic lights inset at 18,18)
 *        around a surface so the design can be judged in a browser.
 * WHY:   In the Tauri app the real window supplies this chrome; surfaces must not draw it.
 *        Surfaces receive `TITLEBAR_H` of empty space at the top for the inset title bar.
 * WHERE: prototype shell only.
 */
export const TITLEBAR_H = 44;

export function WindowFrame({
  width,
  height,
  children,
  className,
}: {
  width: number;
  height: number;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn("relative overflow-hidden rounded-[12px] bg-background shadow-float", className)}
      style={{ width, height }}
    >
      <div className="pointer-events-none absolute top-[15px] left-[16px] z-50 flex gap-2">
        <span className="size-3 rounded-full bg-rec/80" />
        <span className="size-3 rounded-full bg-warn/80" />
        <span className="size-3 rounded-full bg-ok/80" />
      </div>
      {children}
    </div>
  );
}
