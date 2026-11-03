import * as React from "react";
import { ShieldAlert } from "lucide-react";

import { cn } from "@starter/ui/lib/utils";

import { engine } from "../lib/engine";
import type { Permission, PermissionState } from "../lib/types";

const NAME: Record<Permission, string> = { screen: "Screen recording", microphone: "Microphone", camera: "Camera" };

/**
 * SOURCE OF TRUTH KEYWORDS: askPermission, allow camera, allow microphone, TCC prompt, open System Settings
 * WHAT:  Asks macOS for a permission: the system prompt when it has never asked, otherwise
 *        the System Settings pane (macOS shows its prompt only once).
 * WHY:   Every "Allow" affordance must do the one thing that can still change the answer.
 * WHERE: PermissionAsk, the device menus' "Allow … access" items.
 */
export function askPermission(p: Permission, state: PermissionState): Promise<PermissionState> {
  if (state === "not-determined") return engine.requestPermission(p);
  engine.openSystemSettings(p);
  return Promise.resolve(state);
}

/**
 * SOURCE OF TRUTH KEYWORDS: PermissionAsk, needs permission, Allow button, camera overlay, mixer strip, graceful degradation
 * WHAT:  A small "<Source> needs permission · Allow" pill.
 * WHY:   A source without its grant is left out of the preview and recordings instead of
 *        blocking them; this says so where the source would be and fixes it in one click.
 * WHERE: the preview's camera overlay spot, the main window's microphone strip.
 */
export function PermissionAsk({
  permission,
  state,
  className,
}: {
  permission: Permission;
  state: PermissionState;
  className?: string;
}) {
  const [asking, setAsking] = React.useState(false);
  const ask = () => {
    setAsking(true);
    void askPermission(permission, state).finally(() => setAsking(false));
  };
  return (
    <div
      role="status"
      className={cn(
        "flex max-w-full items-center gap-1.5 rounded-full bg-card/90 px-2.5 py-1 text-[11.5px] text-foreground shadow-float",
        className,
      )}
    >
      <ShieldAlert className="size-3.5 shrink-0 text-warn" aria-hidden />
      <span className="truncate">{NAME[permission]} needs permission</span>
      <span aria-hidden className="text-muted-foreground">
        ·
      </span>
      <button
        type="button"
        onClick={ask}
        disabled={asking}
        className="shrink-0 font-semibold text-accent-text outline-none hover:underline focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-60"
      >
        {asking ? "Asking…" : "Allow"}
      </button>
    </div>
  );
}

/** The device-menu value of the "Allow … access" item (never stored in settings). */
export const ALLOW_ACCESS = "__allow-access__";

/**
 * SOURCE OF TRUTH KEYWORDS: allowAccessItem, ALLOW_ACCESS, empty device list, camera menu, microphone menu
 * WHAT:  The "Allow camera access…" / "Allow microphone access…" menu entry, or null when the
 *        list has devices or the permission is granted.
 * WHY:   macOS may list no cameras or mics to an app it has not granted; an empty menu
 *        explains nothing, this entry asks and the list fills in after the grant.
 * WHERE: the camera and microphone menus in the main window and Settings.
 */
export function allowAccessItem(
  p: "camera" | "microphone",
  devices: readonly unknown[],
  state: PermissionState,
): { value: string; label: string } | null {
  if (state === "granted" || devices.length > 0) return null;
  return { value: ALLOW_ACCESS, label: p === "camera" ? "Allow camera access…" : "Allow microphone access…" };
}
