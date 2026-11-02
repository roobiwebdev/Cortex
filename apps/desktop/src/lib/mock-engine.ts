import { useSyncExternalStore } from "react";

import type { EngineState, Permission, RecorderSettings, Recording, Toast, ToastKind } from "./types";

/**
 * SOURCE OF TRUTH KEYWORDS: engine, mockEngine, useEngine, startRecording, stopRecording, updateSettings
 * WHAT:  A fake recorder engine that behaves like the real one: countdown, recording clock,
 *        pause, finalize, audio levels, toasts, permissions and settings.
 * WHY:   Lets the whole UI be designed and reviewed before the Rust backend is wired.
 *        The action names match the planned Tauri commands so swapping it is mechanical.
 * WHERE: every desktop surface via `useEngine()` and the `engine` action object.
 */

const DEFAULT_SETTINGS: RecorderSettings = {
  outputDir: "~/Movies/Cortex",
  container: "mov",
  codec: "auto",
  resolution: "1080p",
  fps: 60,
  bitrateMbps: null,
  target: { kind: "display", displayId: 1 },
  scene: "screen-cam",
  cameraId: "facetime",
  cameraTrack: true,
  cameraResolution: "1080p",
  micEnabled: true,
  micId: "builtin-mic",
  systemAudio: true,
  audioTracks: "both",
  systemGainDb: 0,
  micGainDb: 0,
  systemMuted: false,
  micMuted: false,
  systemOffsetMs: null,
  micOffsetMs: 0,
  micBackend: "sck",
  pipSize: 0.22,
  pipPlace: { corner: "bottom-right" },
  pipShape: "squircle",
  pipRoundness: 0.35,
  pipBorder: 0,
  pipBorderColor: "ffffff",
  pipShadow: true,
  mirror: true,
  cursor: true,
  clicks: false,
  countdown: 3,
  previewLowPower: false,
  replayEnabled: false,
  replaySecs: 60,
  streamDests: [
    { service: "youtube", enabled: true, keyStored: true },
    { service: "twitch", enabled: false, keyStored: false },
    { service: "kick", enabled: false, keyStored: false },
  ],
  liveResolution: "1080p",
  streamBitrateKbps: 6000,
  streamBitrateAuto: true,
  streamCodec: "auto",
  keyframeSecs: 2,
  streamCbr: true,
  streamAudioKbps: 192,
  theme: "system",
  showHud: true,
  openAtLogin: false,
  showInDock: true,
  autoUpdate: true,
  accentHue: 188,
};

const HOUR = 3600;
const now = Date.now();
const SAMPLE_RECORDINGS: Recording[] = [
  rec("r3", "Product walkthrough", 412, 186_000_000, now - 2 * HOUR * 1000, true),
  rec("r2", "Bug repro — checkout", 95, 41_000_000, now - 26 * HOUR * 1000, false),
  rec("r1", "Weekly update", 1288, 612_000_000, now - 74 * HOUR * 1000, true),
];

function rec(
  id: string,
  name: string,
  durationSecs: number,
  bytes: number,
  createdAt: number,
  cam: boolean,
): Recording {
  return {
    id,
    name,
    path: `~/Movies/Cortex/${name}.mov`,
    durationSecs,
    bytes,
    createdAt,
    width: 1920,
    height: 1080,
    hasCameraTrack: cam,
  };
}

let state: EngineState = {
  status: "idle",
  countdownLeft: 0,
  elapsedSecs: 0,
  bytesWritten: 0,
  droppedFrames: 0,
  live: false,
  replayBufferedSecs: 0,
  levels: { system: 0, systemPeak: 0, mic: 0, micPeak: 0 },
  permissions: { screen: "granted", microphone: "granted", camera: "granted" },
  displays: [
    { id: 1, name: "Built-in Retina Display", width: 3456, height: 2234 },
    { id: 2, name: "Studio Display", width: 5120, height: 2880 },
  ],
  windows: [
    { id: 12075, app: "Ghostty", title: "~/Projects/Cortex" },
    { id: 12013, app: "Preview", title: "Q4 roadmap.pdf" },
    { id: 1898, app: "Figma", title: "Cortex — Desktop UI" },
    { id: 2201, app: "Safari", title: "Docs — Getting started" },
  ],
  apps: [
    { bundleId: "com.mitchellh.ghostty", name: "Ghostty", windows: 2 },
    { bundleId: "com.figma.Desktop", name: "Figma", windows: 1 },
    { bundleId: "com.apple.Safari", name: "Safari", windows: 3 },
    { bundleId: "com.apple.Preview", name: "Preview", windows: 1 },
  ],
  cameras: [
    { id: "facetime", name: "FaceTime HD Camera", connected: true },
    { id: "iphone", name: "iPhone Camera", connected: false },
  ],
  mics: [
    { id: "builtin-mic", name: "MacBook Pro Microphone", connected: true },
    { id: "shure", name: "Shure MV7+", connected: true },
  ],
  recordings: SAMPLE_RECORDINGS,
  toasts: [],
  settings: DEFAULT_SETTINGS,
  freeDiskBytes: 182_000_000_000,
};

const listeners = new Set<() => void>();
function set(patch: Partial<EngineState>) {
  state = { ...state, ...patch };
  for (const l of listeners) l();
}
/** Store subscription, shared with `engine.ts` so both modes use one `useSyncExternalStore`. */
export function subscribeEngine(l: () => void) {
  listeners.add(l);
  startLevels();
  return () => {
    listeners.delete(l);
    if (listeners.size === 0) stopLevels();
  };
}

export function useEngine(): EngineState {
  return useSyncExternalStore(subscribeEngine, getEngineState);
}

/** Current mock snapshot, for callers outside React. */
export function getEngineState(): EngineState {
  return state;
}

let clock: ReturnType<typeof setInterval> | undefined;
let countdownTimer: ReturnType<typeof setInterval> | undefined;
let toastSeq = 0;

function startClock() {
  stopClock();
  clock = setInterval(() => {
    if (state.status !== "recording") return;
    const mbps = state.settings.resolution === "4k" ? 2.4 : 1.1;
    set({ elapsedSecs: state.elapsedSecs + 1, bytesWritten: state.bytesWritten + mbps * 1_000_000 });
  }, 1000);
}
function stopClock() {
  if (clock) clearInterval(clock);
  clock = undefined;
}

// Simulated levels: a speech-like envelope for the mic, a steadier bed for system audio.
// The ticker only runs while something is subscribed, so importing this module is free.
let t = 0;
let levelsTimer: ReturnType<typeof setInterval> | undefined;
function startLevels() {
  levelsTimer ??= setInterval(tickLevels, 1000 / 30);
}
function stopLevels() {
  if (levelsTimer) clearInterval(levelsTimer);
  levelsTimer = undefined;
}
function tickLevels() {
  t += 1;
  const s = state.settings;
  const speech = Math.max(0, Math.sin(t / 3.1) * 0.5 + Math.sin(t / 1.7) * 0.3 + Math.random() * 0.25);
  const bed = 0.35 + Math.sin(t / 9) * 0.1 + Math.random() * 0.12;
  const mic = s.micEnabled && !s.micMuted ? Math.min(1, speech * dbToGain(s.micGainDb)) : 0;
  const system = s.systemAudio && !s.systemMuted ? Math.min(1, bed * dbToGain(s.systemGainDb)) : 0;
  // Replay buffer fills while enabled (one tick ≈ 1/30 s), capped at its length.
  const replayBufferedSecs = s.replayEnabled ? Math.min(s.replaySecs, state.replayBufferedSecs + 1 / 30) : 0;
  const decay = 0.012;
  const lv = state.levels;
  set({
    replayBufferedSecs,
    levels: {
      mic,
      system,
      micPeak: Math.max(mic, lv.micPeak - decay),
      systemPeak: Math.max(system, lv.systemPeak - decay),
    },
  });
}

function dbToGain(db: number) {
  return Math.pow(10, db / 20);
}

function pushToast(kind: ToastKind, title: string, detail: string, recordingId?: string) {
  toastSeq += 1;
  const toast: Toast = { id: `t${toastSeq}`, kind, title, detail, recordingId };
  // At most three queued, newest last.
  set({ toasts: [...state.toasts, toast].slice(-3) });
}

function beginRecording() {
  set({ status: "recording", countdownLeft: 0, elapsedSecs: 0, bytesWritten: 0, droppedFrames: 0 });
  startClock();
}

export const engine = {
  start() {
    if (state.status !== "idle") return;
    const n = state.settings.countdown;
    if (n === 0) return beginRecording();
    set({ status: "countdown", countdownLeft: n });
    countdownTimer = setInterval(() => {
      const left = state.countdownLeft - 1;
      if (left <= 0) {
        if (countdownTimer) clearInterval(countdownTimer);
        beginRecording();
      } else set({ countdownLeft: left });
    }, 1000);
  },
  cancelCountdown() {
    // Only meaningful during the countdown; Esc must never interrupt a recording.
    if (state.status !== "countdown") return;
    if (countdownTimer) clearInterval(countdownTimer);
    set({ status: "idle", countdownLeft: 0 });
  },
  pause() {
    if (state.status === "recording") set({ status: "paused" });
  },
  resume() {
    if (state.status === "paused") set({ status: "recording" });
  },
  togglePause() {
    if (state.status === "recording") engine.pause();
    else if (state.status === "paused") engine.resume();
  },
  stop() {
    if (state.status !== "recording" && state.status !== "paused") return;
    stopClock();
    set({ status: "finalizing" });
    setTimeout(() => {
      const id = `r${Date.now()}`;
      const name = `Recording ${new Date().toLocaleString(undefined, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" })}`;
      const r = rec(
        id,
        name,
        Math.max(1, state.elapsedSecs),
        state.bytesWritten,
        Date.now(),
        state.settings.cameraTrack,
      );
      set({ status: "idle", recordings: [r, ...state.recordings] });
      pushToast("saved", name, `${formatShort(r.durationSecs)} · ${state.settings.container.toUpperCase()}`, id);
    }, 700);
  },
  toggleRecord() {
    if (state.status === "idle") engine.start();
    else if (state.status === "countdown") engine.cancelCountdown();
    else engine.stop();
  },
  saveReplay() {
    if (!state.settings.replayEnabled) return;
    pushToast("replay", "Replay saved", `Last ${state.settings.replaySecs}s · MP4`);
  },
  /** Stop without saving (HUD "Discard recording"). */
  discard() {
    if (countdownTimer) clearInterval(countdownTimer);
    stopClock();
    set({ status: "idle", countdownLeft: 0, elapsedSecs: 0, bytesWritten: 0 });
  },
  // Native-only actions; the prototype has nothing to open.
  revealRecording(_id: string) {},
  openRecordingsFolder() {},
  openSystemSettings(_p: Permission) {},
  toggleLive() {
    set({ live: !state.live });
  },
  dismissToast(id: string) {
    set({ toasts: state.toasts.filter((x) => x.id !== id) });
  },
  demoToast(kind: ToastKind) {
    if (kind === "warning") pushToast("warning", "Disk space is low", "Less than 5 GB left on Macintosh HD");
    else if (kind === "replay") pushToast("replay", "Replay saved", "Last 60s · MP4");
    else pushToast("saved", "Product walkthrough", "6:52 · MOV", "r3");
  },
  update(patch: Partial<RecorderSettings>) {
    set({ settings: { ...state.settings, ...patch } });
  },
  setPermission(p: Permission, value: EngineState["permissions"][Permission]) {
    set({ permissions: { ...state.permissions, [p]: value } });
  },
  renameRecording(id: string, name: string) {
    set({ recordings: state.recordings.map((r) => (r.id === id ? { ...r, name } : r)) });
  },
};

function formatShort(secs: number) {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}
