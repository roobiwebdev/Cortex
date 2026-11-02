/**
 * SOURCE OF TRUTH KEYWORDS: RecorderSettings, EngineState, Scene, PipShape, CaptureTarget, Device, desktop types
 * WHAT:  The recorder's settings, devices and engine state as the UI sees them.
 * WHY:   Mirrors Rec's settings.json schema so every Rec option has a home in the UI.
 *        During the design phase a mock engine fills these; later they are generated
 *        from the Rust `rec-core` crate (tauri-specta) and this file is replaced.
 * WHERE: every desktop surface, the mock engine, and (later) the IPC bindings.
 */

export type Container = "mov" | "mp4";
export type Codec = "auto" | "h264" | "hevc" | "prores";
export type Resolution = "720p" | "1080p" | "1440p" | "4k" | "native";
export type Fps = 24 | 30 | 60;
export type Scene = "screen" | "screen-cam" | "cam";
export type PipShape = "rounded" | "circle" | "square" | "pill" | "squircle" | "rect";
export type PipCorner = "top-left" | "top-right" | "bottom-left" | "bottom-right";
export type AudioTracks = "both" | "mix" | "stems";
export type StreamService = "youtube" | "twitch" | "kick" | "custom";
export type ThemePref = "system" | "light" | "dark";

/** What is being captured. Region and app are new in Cortex (Rec had them CLI-only or not at all). */
export type CaptureTarget =
  | { kind: "display"; displayId: number }
  | { kind: "window"; windowId: number }
  | { kind: "app"; bundleId: string }
  | { kind: "region"; displayId: number; x: number; y: number; w: number; h: number };

/** Overlay placement: snapped to a corner, or free (centre as 0..1 of the frame). */
export type PipPlace = { corner: PipCorner } | { x: number; y: number };

export interface StreamDest {
  service: StreamService;
  url?: string;
  enabled: boolean;
  keyStored: boolean;
}

export interface RecorderSettings {
  outputDir: string;
  container: Container;
  codec: Codec;
  resolution: Resolution;
  fps: Fps;
  bitrateMbps: number | null;
  target: CaptureTarget;
  scene: Scene;
  cameraId: string | null;
  cameraTrack: boolean;
  cameraResolution: "720p" | "1080p";
  micEnabled: boolean;
  micId: string | null;
  systemAudio: boolean;
  audioTracks: AudioTracks;
  systemGainDb: number;
  micGainDb: number;
  systemMuted: boolean;
  micMuted: boolean;
  systemOffsetMs: number | null;
  micOffsetMs: number;
  micBackend: "sck" | "avcapture";
  pipSize: number;
  pipPlace: PipPlace;
  pipShape: PipShape;
  pipRoundness: number;
  pipBorder: number;
  /** Border colour as RRGGBB hex (user data, like Rec's pip_border_color). */
  pipBorderColor: string;
  pipShadow: boolean;
  mirror: boolean;
  cursor: boolean;
  clicks: boolean;
  countdown: 0 | 3 | 5;
  previewLowPower: boolean;
  replayEnabled: boolean;
  replaySecs: number;
  streamDests: StreamDest[];
  liveResolution: Resolution;
  streamBitrateKbps: number;
  streamBitrateAuto: boolean;
  streamCodec: "auto" | "h264" | "hevc";
  keyframeSecs: number;
  streamCbr: boolean;
  streamAudioKbps: number;
  theme: ThemePref;
  showHud: boolean;
  openAtLogin: boolean;
  showInDock: boolean;
  autoUpdate: boolean;
  accentHue: number;
}

export interface Display {
  id: number;
  name: string;
  width: number;
  height: number;
}
export interface CaptureWindow {
  id: number;
  app: string;
  title: string;
}
export interface CaptureApp {
  bundleId: string;
  name: string;
  windows: number;
}
export interface InputDevice {
  id: string;
  name: string;
  connected: boolean;
}

export type Permission = "screen" | "microphone" | "camera";
export type PermissionState = "granted" | "denied" | "not-determined";

export type EngineStatus = "idle" | "countdown" | "recording" | "paused" | "finalizing";

export interface Levels {
  system: number;
  systemPeak: number;
  mic: number;
  micPeak: number;
}

export interface Recording {
  id: string;
  name: string;
  path: string;
  durationSecs: number;
  bytes: number;
  createdAt: number;
  width: number;
  height: number;
  hasCameraTrack: boolean;
}

export type ToastKind = "saved" | "replay" | "warning";
export interface Toast {
  id: string;
  kind: ToastKind;
  title: string;
  detail: string;
  recordingId?: string;
}

export interface EngineState {
  status: EngineStatus;
  countdownLeft: number;
  elapsedSecs: number;
  bytesWritten: number;
  droppedFrames: number;
  live: boolean;
  replayBufferedSecs: number;
  levels: Levels;
  permissions: Record<Permission, PermissionState>;
  displays: Display[];
  windows: CaptureWindow[];
  apps: CaptureApp[];
  cameras: InputDevice[];
  mics: InputDevice[];
  recordings: Recording[];
  toasts: Toast[];
  settings: RecorderSettings;
  freeDiskBytes: number;
}
