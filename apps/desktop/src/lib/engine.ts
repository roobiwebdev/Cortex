import { useSyncExternalStore } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { commands, events, type EngineError } from "../ipc/bindings";
import * as mock from "./mock-engine";
import { errorMessage, errorToast, pushToastTo, replayToast, savedToast } from "./toasts";
import type { EngineSnapshot, EngineState, Permission, PermissionState, RecorderSettings, Toast } from "./types";

/**
 * SOURCE OF TRUTH KEYWORDS: engine, useEngine, connectEngine, tauriEngine, IPC adapter, engine://state, settings://changed
 * WHAT:  The engine every surface talks to. Inside Tauri it calls the generated IPC commands
 *        and follows the engine and settings events; in a plain browser it is the mock engine.
 * WHY:   Surfaces stay identical in the design-review harness and the real app. The API
 *        matches `mock-engine.ts` exactly (`typeof mock.engine`). The engine's snapshot, the
 *        stored settings and the UI's toasts arrive separately and are composed here.
 * WHERE: every desktop surface and the prototype shell; `connectEngine()` in main.tsx.
 */

const inTauri = isTauri();

let snapshot: EngineSnapshot | undefined;
let settings: RecorderSettings | undefined;
let toasts: Toast[] = [];
let composed: EngineState | undefined;
const listeners = new Set<() => void>();

function changed() {
  composed = snapshot && settings ? { ...snapshot, settings, toasts } : undefined;
  for (const l of listeners) l();
}
function subscribeTauri(l: () => void) {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}
// Before `connectEngine()` resolves nothing renders, so the mock fallback is never shown.
const snapshotTauri = () => composed ?? mock.getEngineState();

function current(): EngineState {
  return snapshotTauri();
}

function toast(t: Toast) {
  toasts = pushToastTo(toasts, t);
  changed();
}

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: EngineError };

/** Run a command; a failure becomes a warning toast. Resolves with the data, or undefined. */
async function run<T>(call: Promise<Result<T>>): Promise<T | undefined> {
  try {
    const r = await call;
    if (r.status === "ok") return r.data;
    toast(errorToast(errorMessage(r.error), true));
  } catch (err: unknown) {
    console.error("cortex: engine command failed", err);
  }
  return undefined;
}

function setSettings(next: RecorderSettings) {
  settings = next;
  changed();
}

const tauriEngine: typeof mock.engine = {
  start() {
    void run(commands.start());
  },
  cancelCountdown() {
    // Only meaningful during the countdown; Esc must never interrupt a recording.
    if (current().status === "countdown") void run(commands.cancelCountdown());
  },
  pause() {
    void run(commands.pause());
  },
  resume() {
    void run(commands.resume());
  },
  togglePause() {
    const s = current().status;
    if (s === "recording") void run(commands.pause());
    else if (s === "paused") void run(commands.resume());
  },
  stop() {
    void run(commands.stop());
  },
  toggleRecord() {
    const s = current().status;
    if (s === "idle") void run(commands.start());
    else if (s === "countdown") void run(commands.cancelCountdown());
    else if (s === "recording" || s === "paused") void run(commands.stop());
  },
  saveReplay() {
    void run(commands.saveReplay());
  },
  discard() {
    void run(commands.discard());
  },
  revealRecording(id: string) {
    const r = current().recordings.find((x) => x.id === id);
    if (r) void run(commands.revealRecording(r.path));
  },
  openRecordingsFolder() {
    void run(commands.openRecordingsFolder());
  },
  openSystemSettings(p: Permission) {
    void run(commands.openSystemSettings(p));
  },
  toggleLive() {
    void run(current().live ? commands.stopLive() : commands.goLive());
  },
  dismissToast(id: string) {
    toasts = toasts.filter((t) => t.id !== id);
    changed();
  },
  // The design harness's sample toasts; the real app only shows real ones.
  demoToast(_kind) {},
  update(p: Partial<RecorderSettings>) {
    const next = { ...current().settings, ...p };
    // Optimistic: sliders must not wait for the round trip. The stored (validated)
    // settings come back in the reply and in `settings://changed`.
    setSettings(next);
    void run(commands.updateSettings(next)).then((stored) => {
      if (stored) setSettings(stored);
    });
  },
  // Permissions are the system's to change; the harness-only override does nothing here.
  setPermission(_p: Permission, _value: PermissionState) {},
  async requestPermission(p: Permission, _outcome?: PermissionState): Promise<PermissionState> {
    return (await run(commands.requestPermission(p))) ?? current().permissions[p];
  },
  renameRecording(id: string, name: string) {
    // UI-only until the recordings library lands: the file keeps its name.
    if (!snapshot) return;
    snapshot = { ...snapshot, recordings: snapshot.recordings.map((r) => (r.id === id ? { ...r, name } : r)) };
    changed();
  },
};

/**
 * SOURCE OF TRUTH KEYWORDS: connectEngine, engine bootstrap, engine events, settings events, toasts
 * WHAT:  Subscribes to the engine and settings events, then loads the first snapshot and settings.
 * WHY:   Surfaces read state synchronously, so both must exist before render. Toasts are made
 *        here from recording-finished, replay-saved and error events.
 * WHERE: main.tsx, before mounting React inside Tauri. No-op in a browser.
 */
export async function connectEngine(): Promise<void> {
  if (!inTauri) return;
  // State and levels are scoped to this window, so the shell can skip hidden windows
  // (each window is its own WebKit process; see `emitter` in src-tauri/src/ipc.rs).
  const self = getCurrentWindow();
  await Promise.all([
    events.engineState(self).listen((e) => {
      snapshot = e.payload;
      changed();
    }),
    events.engineLevels(self).listen((e) => {
      if (!snapshot) return;
      snapshot = { ...snapshot, levels: e.payload };
      changed();
    }),
    events.settingsChanged.listen((e) => setSettings(e.payload)),
    events.engineRecordingFinished.listen((e) => toast(savedToast(e.payload))),
    events.engineReplaySaved.listen((e) => toast(replayToast(e.payload.path, current().settings.replaySecs))),
    events.engineError.listen((e) => toast(errorToast(e.payload.message, e.payload.recoverable))),
  ]);
  const [s, st] = await Promise.all([commands.getSettings(), commands.engineState()]);
  settings = s;
  snapshot = st;
  changed();
}

export const engine: typeof mock.engine = inTauri ? tauriEngine : mock.engine;

export function useEngine(): EngineState {
  return useSyncExternalStore(
    inTauri ? subscribeTauri : mock.subscribeEngine,
    inTauri ? snapshotTauri : mock.getEngineState,
  );
}
