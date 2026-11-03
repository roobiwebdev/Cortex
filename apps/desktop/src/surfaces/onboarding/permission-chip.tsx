import { Check, Circle, LoaderCircle, TriangleAlert } from "lucide-react";

import { cn } from "@starter/ui/lib/utils";

import type { PermissionState } from "../../lib/types";

/**
 * What the chip shows: the TCC state, "waiting" while a prompt is open, or "relaunch" when
 * Screen Recording is on but macOS applies it only after Cortex reopens.
 */
export type ChipState = PermissionState | "waiting" | "relaunch";

const CHIP: Record<ChipState, { label: (settingsApp: string) => string; className: string }> = {
  "not-determined": { label: () => "Not granted", className: "bg-muted text-muted-foreground" },
  waiting: { label: (app) => `Waiting for ${app}…`, className: "bg-accent-tint text-accent-text" },
  granted: { label: () => "Granted", className: "bg-ok-tint text-ok" },
  relaunch: { label: () => "Allowed · reopen Cortex to apply", className: "bg-accent-tint text-accent-text" },
  denied: { label: () => "Denied", className: "bg-warn-tint text-warn" },
};

/**
 * SOURCE OF TRUTH KEYWORDS: PermissionChip, TCC status, granted, denied, waiting, permission badge
 * WHAT:  A small status pill for one macOS privacy permission.
 * WHY:   The user needs to see the result of the system prompt without leaving the step,
 *        including the waiting period while System Settings is in front.
 * WHERE: the tour's permissions step.
 */
export function PermissionChip({
  state,
  settingsApp = "System Settings",
  className,
}: {
  state: ChipState;
  /** The app that holds the privacy switches ("System Settings" on macOS, "Settings" on Windows). */
  settingsApp?: string;
  className?: string;
}) {
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
      {(state === "granted" || state === "relaunch") && <Check className="size-3.5" strokeWidth={2.5} />}
      {state === "waiting" && <LoaderCircle className="size-3.5 animate-spin" />}
      {state === "denied" && <TriangleAlert className="size-3.5" />}
      {state === "not-determined" && <Circle className="size-2.5" strokeWidth={3} />}
      {c.label(settingsApp)}
    </span>
  );
}
