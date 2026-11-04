import { useSyncExternalStore } from "react";

import { isTauri } from "@tauri-apps/api/core";

import { harnessPlatform, subscribePlatform } from "./platform";
import { PLATFORM_WORDS } from "./platform-words";
import { errorToast, pushToastTo, replayToast, savedToast } from "./toasts";
import type {
  ExternalLink,
  EngineState,
  Levels,
  Permission,
  PermissionReport,
  PermissionState,
  PlatformCapabilities,
  PlatformOs,
  RecorderSettings,
  Recording,
  StreamDest,
  Toast,
  ToastKind,
} from "./types";

/**
 * SOURCE OF TRUTH KEYWORDS: engine, mockEngine, useEngine, startRecording, stopRecording, updateSettings
 * WHAT:  A fake recorder engine that behaves like the real one: countdown, recording clock,
 *        pause, finalize, audio levels, toasts, permissions and settings.
 * WHY:   Keeps the design-review harness running in a plain browser. It produces the same
 *        generated types as the Tauri engine (`ipc/bindings.ts`), and `engine.ts` implements
 *        the same action object over IPC.
 * WHERE: every desktop surface via `useEngine()` and the `engine` action object.
 */

/** Mirrors rec-core's `RecorderSettings::default()` (devices: system default), plus demo key status. */
const DEFAULT_SETTINGS: RecorderSettings = {
  outputDir: "~/Movies/Cortex",
  container: "mov",
  codec: "auto",
  resolution: "1080p",
  fps: 60,
  bitrateMbps: null,
  target: { kind: "display", displayId: 1 },
  rememberRegion: true,
  lastRegions: [],
  scene: "screen-cam",
  cameraId: null,
  cameraTrack: true,
  cameraResolution: "1080p",
  micEnabled: true,
  micId: null,
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
  // The harness behaves like a first run: the tour resumes where it was, every coach mark shows.
  onboardingCompleted: false,
  seenHints: [],
  sounds: true,
};

const HOUR = 3600;
/** How long the simulated permission prompt takes to answer. */
const PROMPT_MS = 1500;
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
    fps: 60,
    audioTracks: 3,
  };
}

let state: EngineState = {
  status: "idle",
  countdownLeft: 0,
  elapsedSecs: 0,
  bytesWritten: 0,
  droppedFrames: 0,
  live: false,
  streams: [],
  replayBufferedSecs: 0,
  permissions: { screen: "granted", microphone: "granted", camera: "granted" },
  displays: [
    {
      id: 1,
      name: "Built-in Retina Display",
      width: 3456,
      height: 2234,
      frame: { x: 0, y: 0, w: 1728, h: 1117 },
      scale: 2,
    },
    { id: 2, name: "Studio Display", width: 5120, height: 2880, frame: { x: 1728, y: 0, w: 2560, h: 1440 }, scale: 2 },
  ],
  // Front to back, frames in points from their display's top-left. Preview and Figma sit
  // where MockScreen variant="desktop" draws them on display 1.
  windows: [
    {
      id: 12013,
      app: "Preview",
      bundleId: "com.apple.Preview",
      title: "Q4 roadmap.pdf",
      displayId: 1,
      frame: { x: 1279, y: 246, w: 346, h: 380 },
    },
    {
      id: 1898,
      app: "Figma",
      bundleId: "com.figma.Desktop",
      title: "Cortex — Desktop UI",
      displayId: 1,
      frame: { x: 156, y: 123, w: 1071, h: 827 },
    },
    {
      id: 12075,
      app: "Ghostty",
      bundleId: "com.mitchellh.ghostty",
      title: "~/Projects/Cortex",
      displayId: 2,
      frame: { x: 200, y: 160, w: 1200, h: 800 },
    },
    {
      id: 2201,
      app: "Safari",
      bundleId: "com.apple.Safari",
      title: "Docs — Getting started",
      displayId: 2,
      frame: { x: 900, y: 300, w: 1400, h: 900 },
    },
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
  outputLatencyMs: 12,
  previewing: true,
  // What the macOS engine reports (`mac_capabilities` in rec-engine-macos).
  capabilities: {
    os: "macos",
    recording: true,
    displayCapture: true,
    regionCapture: true,
    windowCapture: true,
    appCapture: true,
    sourcePicker: "app",
    systemAudio: true,
    perAppAudio: true,
    captureExclusion: true,
    globalHotkeys: true,
    hevc: true,
    prores: true,
    preview: "native",
    liveStreaming: true,
    replayBuffer: true,
    quickTrim: true,
    movContainer: true,
    cameraTrack: true,
  },
};

const listeners = new Set<() => void>();
/**
 * What each engine reports (`mac_capabilities`, `windows_capabilities`, `linux_capabilities`
 * with a working PC), so the harness platform switch shows each platform's real screens.
 */
const CAPABILITIES: Record<PlatformOs, PlatformCapabilities> = {
  macos: state.capabilities,
  windows: {
    ...state.capabilities,
    os: "windows",
    appCapture: false,
    perAppAudio: false,
    prores: false,
    preview: "fallback",
    liveStreaming: false,
    replayBuffer: false,
    quickTrim: false,
    movContainer: false,
    cameraTrack: false,
  },
  linux: {
    ...state.capabilities,
    os: "linux",
    appCapture: false,
    sourcePicker: "system",
    perAppAudio: false,
    captureExclusion: false,
    prores: false,
    preview: "none",
    liveStreaming: false,
    replayBuffer: false,
    quickTrim: false,
  },
};
const MAC_SOURCES = { displays: state.displays, windows: state.windows, apps: state.apps };

/** Harness: switch the mock to another platform's capabilities, sources and default folder. */
function applyPlatform(os: PlatformOs) {
  const was = state.capabilities.os;
  if (os === was) return;
  const caps = CAPABILITIES[os];
  const s = state.settings;
  const settings: RecorderSettings = {
    ...s,
    outputDir: s.outputDir === PLATFORM_WORDS[was].defaultFolder ? PLATFORM_WORDS[os].defaultFolder : s.outputDir,
    container: caps.movContainer ? s.container : "mp4",
    replayEnabled: caps.replayBuffer && s.replayEnabled,
    // A target the platform can't record falls back to the first display.
    target: s.target.kind === "app" && !caps.appCapture ? { kind: "display", displayId: 1 } : s.target,
  };
  // Linux lists one stand-in display (the portal picks the real one) and no windows or apps.
  const sources =
    caps.sourcePicker === "system"
      ? {
          displays: [
            {
              id: 1,
              name: "Screen chosen in the system dialog",
              width: 1920,
              height: 1080,
              frame: { x: 0, y: 0, w: 1920, h: 1080 },
              scale: 1,
            },
          ],
          windows: [],
          apps: [],
        }
      : { ...MAC_SOURCES, apps: caps.appCapture ? MAC_SOURCES.apps : [] };
  state = { ...state, ...sources, capabilities: caps, settings };
}
if (!isTauri()) {
  applyPlatform(harnessPlatform());
  subscribePlatform(() => {
    applyPlatform(harnessPlatform());
    for (const l of listeners) l();
  });
}

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
    if (listeners.size === 0 && levelListeners.size === 0) stopLevels();
  };
}

export function useEngine(): EngineState {
  return useSyncExternalStore(subscribeEngine, getEngineState);
}

// Levels are their own store (as in `engine.ts`): a level tick notifies only the meters.
let levels: Levels = { system: 0, systemPeak: 0, mic: 0, micPeak: 0 };
const levelListeners = new Set<() => void>();
/** Level store subscription; keeps the simulated levels running while a meter listens. */
export function subscribeLevels(l: () => void) {
  levelListeners.add(l);
  startLevels();
  return () => {
    levelListeners.delete(l);
    if (listeners.size === 0 && levelListeners.size === 0) stopLevels();
  };
}
/** Current simulated levels. */
export function getLevels(): Levels {
  return levels;
}

/** Current mock snapshot, for callers outside React. */
export function getEngineState(): EngineState {
  return state;
}

let clock: ReturnType<typeof setInterval> | undefined;
let countdownTimer: ReturnType<typeof setInterval> | undefined;

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
  levels = {
    mic,
    system,
    micPeak: Math.max(mic, levels.micPeak - decay),
    systemPeak: Math.max(system, levels.systemPeak - decay),
  };
  for (const l of levelListeners) l();
  if (replayBufferedSecs !== state.replayBufferedSecs) set({ replayBufferedSecs });
}

function dbToGain(db: number) {
  return Math.pow(10, db / 20);
}

function pushToast(toast: Toast) {
  set({ toasts: pushToastTo(state.toasts, toast) });
}

function beginRecording() {
  set({ status: "recording", countdownLeft: 0, elapsedSecs: 0, bytesWritten: 0, droppedFrames: 0 });
  startClock();
}

function setKeyStored(dest: StreamDest, keyStored: boolean) {
  const streamDests = state.settings.streamDests.map((d) =>
    d.service === dest.service && d.url === dest.url ? { ...d, keyStored } : d,
  );
  set({ settings: { ...state.settings, streamDests } });
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
      r.path = `~/Movies/Cortex/${name}.${state.settings.container}`;
      set({ status: "idle", recordings: [r, ...state.recordings] });
      pushToast(savedToast(r));
    }, 700);
  },
  toggleRecord() {
    if (state.status === "idle") engine.start();
    else if (state.status === "countdown") engine.cancelCountdown();
    else engine.stop();
  },
  saveReplay() {
    if (!state.settings.replayEnabled) return;
    pushToast(replayToast("~/Movies/Cortex/replay.mp4", state.settings.replaySecs));
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
  openLogFolder() {},
  openLink(_link: ExternalLink) {},
  openSystemSettings(_p: Permission) {},
  toggleLive() {
    set({ live: !state.live });
  },
  dismissToast(id: string) {
    set({ toasts: state.toasts.filter((x) => x.id !== id) });
  },
  demoToast(kind: ToastKind) {
    if (kind === "warning") pushToast(errorToast("Less than 5 GB left on Macintosh HD", true));
    else if (kind === "replay") pushToast(replayToast("~/Movies/Cortex/replay.mp4", 60));
    else if (SAMPLE_RECORDINGS[0]) pushToast(savedToast(SAMPLE_RECORDINGS[0]));
  },
  /** The real engine stores the key in the Keychain; the mock only flips `keyStored`. */
  setStreamKey(dest: StreamDest, _key: string): Promise<boolean> {
    setKeyStored(dest, true);
    return Promise.resolve(true);
  },
  removeStreamKey(dest: StreamDest): Promise<void> {
    setKeyStored(dest, false);
    return Promise.resolve();
  },
  update(patch: Partial<RecorderSettings>) {
    set({ settings: { ...state.settings, ...patch } });
  },
  /** The harness's device list is fixed. */
  refreshDevices() {},
  /** The harness's grants; nothing waits for a relaunch here. */
  checkPermissions(): Promise<PermissionReport> {
    return Promise.resolve({ permissions: state.permissions, screenNeedsRelaunch: false });
  },
  /** The real app quits and reopens; the harness has nothing to reopen. */
  relaunch() {},
  setPermission(p: Permission, value: PermissionState) {
    set({ permissions: { ...state.permissions, [p]: value } });
  },
  /** Simulates the macOS prompt: answers `outcome` after a short wait. */
  requestPermission(p: Permission, outcome: PermissionState = "granted"): Promise<PermissionState> {
    return new Promise((resolve) => {
      setTimeout(() => {
        engine.setPermission(p, outcome);
        resolve(outcome);
      }, PROMPT_MS);
    });
  },
  renameRecording(id: string, name: string) {
    set({ recordings: state.recordings.map((r) => (r.id === id ? { ...r, name } : r)) });
  },
};
