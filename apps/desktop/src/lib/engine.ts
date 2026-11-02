import { useSyncExternalStore } from "react";
import { isTauri } from "@tauri-apps/api/core";

import { commands, events, type EngineState_Serialize as WireState } from "../ipc/bindings";
import * as mock from "./mock-engine";
import type { EngineState, Fps, RecorderSettings } from "./types";

/**
 * SOURCE OF TRUTH KEYWORDS: engine, useEngine, connectEngine, tauriEngine, IPC adapter, engine://state
 * WHAT:  The engine every surface talks to. Inside Tauri it calls the generated IPC
 *        commands and follows the `engine://state` event; in a plain browser it is the
 *        mock engine, unchanged.
 * WHY:   Surfaces stay identical in the design-review harness and the real app. The
 *        API matches `mock-engine.ts` exactly (`typeof mock.engine`).
 * WHERE: every desktop surface and the prototype shell; `connectEngine()` in main.tsx.
 */

const inTauri = isTauri();

let state: EngineState | undefined;
const listeners = new Set<() => void>();

function set(next: EngineState) {
  state = next;
  for (const l of listeners) l();
}
function subscribeTauri(l: () => void) {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}
// Before `connectEngine()` resolves nothing renders, so the mock fallback is never shown.
const snapshotTauri = () => state ?? mock.getEngineState();

/** The wire types use plain numbers where the UI uses literal unions; narrow them here. */
function toFps(n: number): Fps {
  return n === 24 || n === 30 ? n : 60;
}
function toCountdown(n: number): RecorderSettings["countdown"] {
  return n === 0 || n === 5 ? n : 3;
}
function fromWire(w: WireState): EngineState {
  return {
    ...w,
    settings: { ...w.settings, fps: toFps(w.settings.fps), countdown: toCountdown(w.settings.countdown) },
  };
}

function run(call: Promise<WireState>) {
  call.then((s) => set(fromWire(s))).catch((err: unknown) => console.error("cortex: engine command failed", err));
}

function current(): EngineState {
  return snapshotTauri();
}

/** Local-only edits for actions the native engine does not expose yet. */
function patch(p: Partial<EngineState>) {
  set({ ...current(), ...p });
}

const tauriEngine: typeof mock.engine = {
  start() {
    run(commands.engineStart());
  },
  cancelCountdown() {
    if (current().status === "countdown") run(commands.engineStop());
  },
  pause() {
    run(commands.enginePause());
  },
  resume() {
    run(commands.engineResume());
  },
  togglePause() {
    const s = current().status;
    if (s === "recording") run(commands.enginePause());
    else if (s === "paused") run(commands.engineResume());
  },
  stop() {
    run(commands.engineStop());
  },
  toggleRecord() {
    if (current().status === "idle") run(commands.engineStart());
    else run(commands.engineStop());
  },
  // Not wired yet: replay buffer, discard, reveal/open, permissions, live, rename.
  saveReplay() {},
  discard() {
    run(commands.engineStop());
  },
  revealRecording(_id: string) {},
  openRecordingsFolder() {},
  openSystemSettings(_p) {},
  toggleLive() {},
  dismissToast(id: string) {
    patch({ toasts: current().toasts.filter((t) => t.id !== id) });
  },
  demoToast(_kind) {},
  update(p: Partial<RecorderSettings>) {
    const settings = { ...current().settings, ...p };
    // Optimistic: sliders must not wait for the round trip.
    patch({ settings });
    run(commands.updateSettings(settings));
  },
  setPermission(_p, _value) {},
  renameRecording(id: string, name: string) {
    patch({ recordings: current().recordings.map((r) => (r.id === id ? { ...r, name } : r)) });
  },
};

/**
 * SOURCE OF TRUTH KEYWORDS: connectEngine, engine bootstrap, engine://state listener
 * WHAT:  Loads the first engine snapshot and subscribes to `engine://state`.
 * WHY:   Surfaces read state synchronously, so the snapshot must exist before render.
 * WHERE: main.tsx, before mounting React inside Tauri. No-op in a browser.
 */
export async function connectEngine(): Promise<void> {
  if (!inTauri) return;
  await events.engineState.listen((e) => set(fromWire(e.payload)));
  set(fromWire(await commands.engineState()));
}

export const engine: typeof mock.engine = inTauri ? tauriEngine : mock.engine;

export function useEngine(): EngineState {
  return useSyncExternalStore(
    inTauri ? subscribeTauri : mock.subscribeEngine,
    inTauri ? snapshotTauri : mock.getEngineState,
  );
}
