import { useSyncExternalStore } from "react";
import { isTauri } from "@tauri-apps/api/core";

import { commands, events, type UpdateStatus, type UpdateStatus_Serialize } from "../ipc/bindings";

/**
 * SOURCE OF TRUTH KEYWORDS: useUpdates, checkForUpdates, installUpdate, updater://status, APP_VERSION, software updates
 * WHAT:  The update status for the Settings Updates group and the menu bar panel, and the two
 *        actions: check the feed, and install (download, verify, relaunch).
 * WHY:   Checking and installing run in Rust (`src-tauri/src/updater.rs`); the webview only
 *        reads the status and asks. Every window sees the same status through
 *        `updater://status`, including errors, so a check started from the menu bar shows
 *        its result in Settings too. In a plain browser a mock check finds no update.
 * WHERE: `surfaces/settings/general.tsx` (UpdateRow), `surfaces/menubar-panel.tsx`.
 */

/** The app version, from `src-tauri/tauri.conf.json` at build time (see `vite.config.ts`). */
export const APP_VERSION = import.meta.env.CORTEX_APP_VERSION;

export type UpdateState = UpdateStatus_Serialize;

const inTauri = isTauri();

let state: UpdateState = {
  currentVersion: APP_VERSION,
  lastChecked: null,
  checking: false,
  available: null,
  download: null,
  ready: false,
  installing: false,
  error: null,
};
const listeners = new Set<() => void>();
let connected = false;

function set(next: UpdateState) {
  state = next;
  for (const l of listeners) l();
}

/** Fills in the fields the event may leave out. */
function normalize(s: UpdateStatus): UpdateState {
  return {
    currentVersion: s.currentVersion,
    lastChecked: s.lastChecked ?? null,
    checking: s.checking,
    available: s.available
      ? { version: s.available.version, notes: s.available.notes ?? null, date: s.available.date ?? null }
      : null,
    download: s.download ? { downloaded: s.download.downloaded, total: s.download.total ?? null } : null,
    ready: s.ready,
    installing: s.installing,
    error: s.error ?? null,
  };
}

/** Starts following the shell's status the first time a view needs it. */
function connect() {
  if (connected || !inTauri) return;
  connected = true;
  void events.updaterStatus.listen((e) => set(normalize(e.payload)));
  void commands
    .updateStatus()
    .then((s) => set(normalize(s)))
    .catch((err: unknown) => console.error("cortex: update status unavailable", err));
}

function subscribe(l: () => void) {
  connect();
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}

export function useUpdates(): UpdateState {
  return useSyncExternalStore(subscribe, () => state);
}

/** Checks the feed now. The result (or why it failed) arrives as the status. */
export function checkForUpdates() {
  if (!inTauri) {
    // Harness: a short check that finds the current version.
    set({ ...state, checking: true, error: null });
    setTimeout(() => set({ ...state, checking: false, lastChecked: Date.now() }), 1200);
    return;
  }
  // A failure is already in the status (`error`); nothing else to show.
  void commands.checkForUpdates().catch((err: unknown) => console.error("cortex: update check failed", err));
}

/** Downloads, verifies and installs the update, then Cortex relaunches. Refused while recording. */
export function installUpdate() {
  if (!inTauri) return;
  void commands.installUpdate().catch((err: unknown) => console.error("cortex: update install failed", err));
}
