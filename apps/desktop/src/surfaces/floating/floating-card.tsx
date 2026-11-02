import type * as React from "react";
import { EyeOff } from "lucide-react";

import { cn } from "@starter/ui/lib/utils";

/**
 * SOURCE OF TRUTH KEYWORDS: FloatingCard, floating shell, panel shell, HUD shell, toast shell, shadow-float
 * WHAT:  The shell every floating surface draws on: popover background, radius 18, float elevation.
 * WHY:   Panel, HUD, toasts and countdown must read as one product; one shell keeps the
 *        radius and elevation identical.
 * WHERE: MenuBarPanel, HudSurface, ToastStack, CountdownSurface, PrototypeNote.
 */
export function FloatingCard({ className, ...props }: React.ComponentProps<"div">) {
  return <div className={cn("rounded-[18px] bg-popover text-popover-foreground shadow-float", className)} {...props} />;
}

/**
 * SOURCE OF TRUTH KEYWORDS: PrototypeNote, design review hint, empty state, demo controls, prototype only
 * WHAT:  A centred card that explains a floating surface and offers buttons to show its states.
 * WHY:   Some floating surfaces are empty until the engine does something; reviewers need a
 *        way to trigger them. Not shipped in the app.
 * WHERE: ToastStack (no toasts), CountdownSurface (idle / recording).
 */
export function PrototypeNote({
  title,
  children,
  actions,
  className,
}: {
  title: React.ReactNode;
  children: React.ReactNode;
  actions?: React.ReactNode;
  className?: string;
}) {
  return (
    <FloatingCard className={cn("w-[380px] p-5", className)}>
      <div className="text-[10.5px] font-semibold tracking-[0.08em] text-muted-foreground uppercase">Prototype</div>
      <div className="mt-1 text-[15px] font-semibold">{title}</div>
      <div className="mt-1.5 text-[13px] leading-relaxed text-muted-foreground">{children}</div>
      {actions && <div className="mt-4 flex flex-wrap gap-2">{actions}</div>}
    </FloatingCard>
  );
}

/**
 * SOURCE OF TRUTH KEYWORDS: CaptureExcludedHint, not in recording, sharingType none, floating caption
 * WHAT:  A small pill saying a surface is left out of the recording.
 * WHY:   Reviewers (and later first-run users) need to know the HUD, toasts and countdown
 *        never appear in the captured video.
 * WHERE: HudSurface, CountdownSurface.
 */
export function CaptureExcludedHint({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full bg-card/85 px-3 py-1 text-xs text-muted-foreground shadow-float backdrop-blur-md",
        className,
      )}
    >
      <EyeOff className="size-3.5 shrink-0" aria-hidden />
      {children}
    </div>
  );
}

/**
 * SOURCE OF TRUTH KEYWORDS: MENU_CONTENT, MENU_ITEM, floating menu style, dropdown overrides
 * WHAT:  Class names that give the shared dropdown menu the floating look (radius 12, roomy rows).
 * WHY:   The base dropdown is square and tight; floating surfaces need the studio radius scale.
 * WHERE: MenuBarPanel overflow menu, HudSurface ⋯ menu.
 */
export const MENU_CONTENT = "w-60 rounded-[12px] p-1 shadow-float ring-0";
export const MENU_ITEM = "rounded-[8px] px-2 py-1.5 text-[13px]";
