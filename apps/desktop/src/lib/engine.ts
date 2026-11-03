import { useRef, useSyncExternalStore } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { commands, events, type EngineError } from "../ipc/bindings";
import * as mock from "./mock-engine";
import { errorMessage, errorToast, isToast, newToastId, pushToastTo, replayToast, savedToast } from "./toasts";
import { onPageShown, pageHidden } from "./visibility";
import type { LevelSource } from "@starter/ui/components/level-meter";

import type {
  EngineSnapshot,
  EngineState,
  Levels,
  Permission,
  PermissionReport,
  PermissionState,
  RecorderSettings,
  Recording,
  Toast,
} from "./types";

/**
 * SOURCE OF TRUTH KEYWORDS: engine, useEngine, connectEngine, tauriEngine, IPC adapter, engine://state, settings://changed
 * WHAT:  The engine every surface talks to. Inside Tauri it calls the generated IPC commands
 *        and follows the engine and settings events; in a plain browser it is the mock engine.
 * WHY:   Surfaces stay identical in the design-review harness and the real app. The API
 *        matches `mock-engine.ts` exactly (`typeof mock.engine`). The engine's snapshot, the
 *        stored settings and the UI's toasts arrive separately and are composed here.
 *        Audio levels are a separate store (`micLevels`, `systemLevels`) so a level tick only
 *        redraws meters, and `useEngine(select, isEqual)` lets a view re-render only when the
 *        part it shows changes.
 * WHERE: every desktop surface and the prototype shell; `connectEngine()` in main.tsx.
 */

const inTauri = isTauri();
/** The toasts window owns the toast queue; every other window sends its toasts there. */
const TOASTS_WINDOW = "toasts";
const TOAST_EVENT = "ui://toast";
const ownsToasts = inTauri && getCurrentWindow().label === TOASTS_WINDOW;

let snapshot: EngineSnapshot | undefined;
let settings: RecorderSettings | undefined;
/** Every recording in the output folder (`list_recordings`); replaces the engine's per-run list once loaded. */
let library: Recording[] | undefined;
let toasts: Toast[] = [];
let composed: EngineState | undefined;
/** A state event arrived while the page was hidden and has not been rendered yet. */
let stateBehind = false;
const listeners = new Set<() => void>();

function changed() {
  if (snapshot && settings) {
    const { levels: _levels, ...rest } = snapshot;
    composed = { ...rest, recordings: library ?? rest.recordings, settings, toasts };
  } else composed = undefined;
  for (const l of listeners) l();
}

// The JSON of each object field of the last snapshot, to keep unchanged fields' identity.
const fieldJson = new Map<string, string>();
function isSnapshotKey(k: string, o: EngineSnapshot): k is keyof EngineSnapshot {
  return k in o;
}
function keepField<K extends keyof EngineSnapshot>(next: EngineSnapshot, prev: EngineSnapshot, k: K) {
  next[k] = prev[k];
}
/**
 * The new snapshot with every object field whose content did not change replaced by the
 * previous object. Each state event is freshly deserialized, so without this every list
 * (displays, windows, recordings…) would look new 10 times a second while recording.
 */
function shareUnchanged(next: EngineSnapshot): EngineSnapshot {
  const prev = snapshot;
  const out = { ...next };
  for (const k of Object.keys(out)) {
    if (k === "levels" || !isSnapshotKey(k, out)) continue;
    const v = out[k];
    if (typeof v !== "object" || v === null) continue;
    const json = JSON.stringify(v);
    if (prev && fieldJson.get(k) === json) keepField(out, prev, k);
    else fieldJson.set(k, json);
  }
  return out;
}

const ZERO_LEVELS: Levels = { system: 0, systemPeak: 0, mic: 0, micPeak: 0 };
let levels: Levels = ZERO_LEVELS;
const levelListeners = new Set<() => void>();
function setLevels(next: Levels) {
  const l = levels;
  if (
    l.mic === next.mic &&
    l.micPeak === next.micPeak &&
    l.system === next.system &&
    l.systemPeak === next.systemPeak
  ) {
    return;
  }
  levels = next;
  for (const fn of levelListeners) fn();
}
function subscribeLevelsTauri(l: () => void) {
  levelListeners.add(l);
  return () => {
    levelListeners.delete(l);
  };
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
  if (!ownsToasts) {
    // Through the shell: it creates the toasts window if needed and holds the toast until it is ready.
    commands
      .pushToast({ ...t, recordingId: t.recordingId ?? null })
      .catch((err: unknown) => console.error("cortex: toast not sent", err));
    return;
  }
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
  /** Re-list displays, windows and apps; the new list arrives as an engine state event. */
  refreshDevices() {
    void commands.devices().catch(() => undefined);
  },
  // A failed check reports the last known state instead of toasting once a second.
  async checkPermissions(): Promise<PermissionReport> {
    const r = await commands.checkPermissions().catch(() => undefined);
    return r?.status === "ok" ? r.data : { permissions: current().permissions, screenNeedsRelaunch: false };
  },
  relaunch() {
    void run(commands.relaunch());
  },
  // Permissions are the system's to change; the harness-only override does nothing here.
  setPermission(_p: Permission, _value: PermissionState) {},
  async requestPermission(p: Permission, _outcome?: PermissionState): Promise<PermissionState> {
    return (await run(commands.requestPermission(p))) ?? current().permissions[p];
  },
  renameRecording(id: string, name: string) {
    // Renames the file; the new list arrives as `recordings://changed`.
    void run(commands.renameRecording(id, name));
  },
};

/**
 * SOURCE OF TRUTH KEYWORDS: connectEngine, engine bootstrap, engine events, settings events, toasts
 * WHAT:  Subscribes to the engine and settings events, then loads the first snapshot and settings.
 * WHY:   Surfaces read state synchronously, so both must exist before render. Toasts are made
 *        here from recording-finished, replay-saved and error events, in the toasts window
 *        only (it owns the queue; other windows send theirs as `ui://toast`).
 * WHERE: main.tsx, before mounting React inside Tauri. No-op in a browser.
 */
export async function connectEngine(): Promise<void> {
  if (!inTauri) return;
  // State and levels are scoped to this window, so the shell can skip hidden windows
  // (each window is its own WebKit process; see `emitter` in src-tauri/src/ipc.rs).
  const self = getCurrentWindow();
  await Promise.all([
    events.engineState(self).listen((e) => {
      snapshot = shareUnchanged(e.payload);
      setLevels(e.payload.levels);
      // A hidden window renders the latest state when it is shown, not on every update.
      if (pageHidden()) stateBehind = true;
      else changed();
    }),
    // Levels reach only the meters (see `micLevels`); nothing else re-renders per tick.
    events.engineLevels(self).listen((e) => setLevels(e.payload)),
    events.settingsChanged.listen((e) => setSettings(e.payload)),
    events.recordingsChanged.listen((e) => setLibrary(e.payload)),
    ...(ownsToasts ? listenForToasts() : []),
  ]);
  onPageShown(() => {
    if (!stateBehind) return;
    stateBehind = false;
    changed();
  });
  const [s, st] = await Promise.all([commands.getSettings(), commands.engineState()]);
  settings = s;
  snapshot = shareUnchanged(st);
  setLevels(st.levels);
  changed();
  // The first scan opens every file in the folder; render without waiting for it.
  void commands.listRecordings().then((r) => {
    if (r.status === "ok") setLibrary(r.data);
  });
}

function setLibrary(next: Recording[]) {
  library = next;
  changed();
}

/** The toasts window's sources: engine events, and toasts other windows send. */
function listenForToasts() {
  return [
    events.engineRecordingFinished.listen((e) => toast(savedToast(e.payload))),
    events.engineReplaySaved.listen((e) => toast(replayToast(e.payload.path, current().settings.replaySecs))),
    events.engineError.listen((e) => toast(errorToast(e.payload.message, e.payload.recoverable))),
    listen<unknown>(TOAST_EVENT, (e) => {
      // Ids are per window; give the toast one that is unique in this queue.
      if (isToast(e.payload)) toast({ ...e.payload, id: newToastId() });
    }),
  ];
}

export const engine: typeof mock.engine = inTauri ? tauriEngine : mock.engine;

const subscribe = inTauri ? subscribeTauri : mock.subscribeEngine;
const getState = inTauri ? snapshotTauri : mock.getEngineState;

/**
 * SOURCE OF TRUTH KEYWORDS: useEngine, engine selector, fine-grained subscription, isEqual, re-render
 * WHAT:  The engine state in React. `useEngine()` returns all of it; `useEngine(select, isEqual)`
 *        returns `select(state)` and re-renders only when that changes (by `isEqual`, default
 *        `Object.is`).
 * WHY:   While recording the engine sends a snapshot 10 times a second (clock, bytes). A view
 *        that does not show those numbers selects what it shows so it stays still; unchanged
 *        fields keep their object identity (`shareUnchanged`), so reference checks work.
 * WHERE: every desktop surface; `sameExceptProgress` for whole-window views.
 */
export function useEngine(): EngineState;
export function useEngine<T>(select: (s: EngineState) => T, isEqual?: (a: T, b: T) => boolean): T;
export function useEngine<T>(select?: (s: EngineState) => T, isEqual: (a: T, b: T) => boolean = Object.is) {
  const memo = useRef<{ state: EngineState; select: (s: EngineState) => T; value: T } | null>(null);
  const getSelected = () => {
    const state = getState();
    const m = memo.current;
    if (!select) return state;
    if (m && m.state === state && m.select === select) return m.value;
    const value = select(state);
    // Keep the previous value when equal, so React sees the same snapshot and skips the render.
    memo.current = { state, select, value: m && isEqual(m.value, value) ? m.value : value };
    return memo.current.value;
  };
  return useSyncExternalStore(subscribe, getSelected);
}

/** Fields that change while recording without changing what most views show. */
const PROGRESS_FIELDS = new Set([
  "elapsedSecs",
  "bytesWritten",
  "droppedFrames",
  "replayBufferedSecs",
  "freeDiskBytes",
]);

/**
 * SOURCE OF TRUTH KEYWORDS: sameExceptProgress, progress fields, clock, bytes written, whole-window view
 * WHAT:  An `isEqual` for `useEngine(s => s, sameExceptProgress)`: two states are the same when
 *        every field but the progress counters (clock, bytes, dropped frames, replay buffer,
 *        free disk) is the same object or value.
 * WHY:   A window that passes the whole state down then re-renders on real changes only; the
 *        counters are shown by components that subscribe to them (`useClock`, `useProgress`).
 * WHERE: the main window, menu-bar panel and HUD roots.
 */
export function sameExceptProgress(a: EngineState, b: EngineState): boolean {
  if (a === b) return true;
  for (const k of Object.keys(b)) {
    if (PROGRESS_FIELDS.has(k)) continue;
    if (!Object.is(Reflect.get(a, k), Reflect.get(b, k))) return false;
  }
  return true;
}

const wholeState = (s: EngineState) => s;

/** The whole state, re-rendering only when something other than the progress counters changes. */
export function useEngineCalm(): EngineState {
  return useEngine(wholeState, sameExceptProgress);
}

const clockSecs = (s: EngineState) => Math.floor(s.elapsedSecs);

/** Whole seconds recorded: re-renders once a second while recording. */
export function useClock(): number {
  return useEngine(clockSecs);
}

type Progress = { secs: number; bytesWritten: number; droppedFrames: number; freeDiskBytes: number };
const progressOf = (s: EngineState): Progress => ({
  secs: Math.floor(s.elapsedSecs),
  bytesWritten: s.bytesWritten,
  droppedFrames: s.droppedFrames,
  freeDiskBytes: s.freeDiskBytes,
});
// Counters refresh with the clock (1 Hz) or when the status changes; a new dropped frame shows at once.
const sameSecond = (a: Progress, b: Progress) => a.secs === b.secs && a.droppedFrames === b.droppedFrames;

/** Clock, bytes written, dropped frames and free disk, sampled once a second. */
export function useProgress(): Progress {
  return useEngine(progressOf, sameSecond);
}

function levelSource(pick: (l: Levels) => { level: number; peak: number }): LevelSource {
  return inTauri
    ? { subscribe: subscribeLevelsTauri, read: () => pick(levels) }
    : { subscribe: mock.subscribeLevels, read: () => pick(mock.getLevels()) };
}

/**
 * SOURCE OF TRUTH KEYWORDS: micLevels, systemLevels, silentLevels, LevelSource, levels store, meters
 * WHAT:  Live audio level sources for `<LevelMeter source={…} />`.
 * WHY:   Levels arrive ~30 times a second; meters read them from this store and draw
 *        themselves, so no React component re-renders per level tick.
 * WHERE: main-window mixer, menu-bar panel, HUD, settings audio, onboarding.
 */
export const micLevels = levelSource((l) => ({ level: l.mic, peak: l.micPeak }));
export const systemLevels = levelSource((l) => ({ level: l.system, peak: l.systemPeak }));
export const silentLevels: LevelSource = { subscribe: () => () => {}, read: () => ({ level: 0, peak: 0 }) };
