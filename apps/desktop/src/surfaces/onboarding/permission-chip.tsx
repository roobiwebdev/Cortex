import { Check, Circle, LoaderCircle, TriangleAlert } from "lucide-react";

import { cn } from "@starter/ui/lib/utils";

import type { PermissionState } from "../../lib/types";

/** What the chip shows: the engine's TCC state, or "waiting" while System Settings is open. */
export type ChipState = PermissionState | "waiting";

const CHIP: Record<ChipState, { label: string; className: string }> = {
  "not-determined": { label: "Not granted", className: "bg-muted text-muted-foreground" },
  waiting: { label: "Waiting for System Settings…", className: "bg-accent-tint text-accent-text" },
  granted: { label: "Granted", className: "bg-ok-tint text-ok" },
  denied: { label: "Denied — enable in System Settings", className: "bg-warn-tint text-warn" },
};

/**
 * SOURCE OF TRUTH KEYWORDS: PermissionChip, TCC status, granted, denied, waiting, permission badge
 * WHAT:  A small status pill for one macOS privacy permission.
 * WHY:   The user needs to see the result of the system prompt without leaving the step,
 *        including the waiting period while System Settings is in front.
 * WHERE: onboarding permission steps and the Ready summary.
 */
export function PermissionChip({ state, className }: { state: ChipState; className?: string }) {
  const c = CHIP[state];
  return (
    <span
      role="status"
      aria-live="polite"
      className={cn(
        "inline-flex h-6 items-center gap-1.5 rounded-full px-2.5 text-[12px] font-medium transition-colors duration-200 ease-out-quick",
        c.className,
        className,
      )}
    >
      {state === "granted" && <Check className="size-3.5" strokeWidth={2.5} />}
      {state === "waiting" && <LoaderCircle className="size-3.5 animate-spin" />}
      {state === "denied" && <TriangleAlert className="size-3.5" />}
      {state === "not-determined" && <Circle className="size-2.5" strokeWidth={3} />}
      {c.label}
    </span>
  );
}
