import * as React from "react";
import { isTauri } from "@tauri-apps/api/core";

import { commands } from "../ipc/bindings";

const inTauri = isTauri();
/** Bundle ID → icon `data:` URL. Icons don't change while an app runs. */
const cache = new Map<string, string>();
const pending = new Set<string>();
const listeners = new Set<() => void>();

function subscribe(l: () => void) {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}

function load(bundleId: string) {
  if (pending.has(bundleId)) return;
  pending.add(bundleId);
  void commands
    .appIcon(bundleId)
    .then((r) => (r.status === "ok" ? r.data : null))
    .catch(() => null)
    .then((icon) => {
      pending.delete(bundleId);
      // A miss is not cached: the app may not be running yet.
      if (!icon) return;
      cache.set(bundleId, icon);
      for (const l of listeners) l();
    });
}

/**
 * SOURCE OF TRUTH KEYWORDS: useAppIcon, app icon, appIcon command, bundle id icon, NSRunningApplication icon
 * WHAT:  A running app's real icon (small PNG `data:` URL) by bundle ID, or null until it loads
 *        or when there is none. Always null in the browser harness.
 * WHY:   Icons are fetched on demand (`app_icon`) instead of riding in every engine state event.
 * WHERE: AppIconTile (region picker and the main window's App list).
 */
export function useAppIcon(bundleId: string | undefined): string | null {
  const icon = React.useSyncExternalStore(subscribe, () => (bundleId ? (cache.get(bundleId) ?? null) : null));
  React.useEffect(() => {
    if (inTauri && bundleId && !cache.has(bundleId)) load(bundleId);
  }, [bundleId]);
  return icon;
}
