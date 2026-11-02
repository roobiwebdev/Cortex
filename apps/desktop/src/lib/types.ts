/**
 * SOURCE OF TRUTH KEYWORDS: RecorderSettings, EngineState, EngineSnapshot, Toast, Scene, CaptureTarget, desktop types
 * WHAT:  The recorder's types as the UI sees them. Everything the engine owns is re-exported
 *        from the generated IPC bindings (Rust `rec-core` is the source of truth); this file
 *        only adds the UI-owned parts: toasts, option lists, and the composed `EngineState`.
 * WHY:   One schema from Rust to every screen. The webview's view of the engine is the engine
 *        snapshot plus the stored settings (from `get_settings` / `settings://changed`) and
 *        the toasts the UI makes from engine events.
 * WHERE: every desktop surface, the mock engine and the Tauri adapter (`engine.ts`).
 */

import type { EngineState_Serialize, RecorderSettings_Serialize, StreamDest_Serialize } from "../ipc/bindings";

export type {
  AudioTracks,
  CameraResolution,
  CaptureApp,
  CaptureTarget,
  CaptureWindow,
  Codec,
  Container,
  Devices,
  Display,
  EngineError,
  EngineStatus,
  InputDevice,
  Levels,
  MicBackend,
  Permission,
  PermissionState,
  Permissions,
  PipCorner,
  PipPlace,
  PipShape,
  Recording,
  Resolution,
  Scene,
  StreamCodec,
  StreamService,
  StreamStatus,
  ThemePref,
} from "../ipc/bindings";

/** Settings as the engine stores and sends them (every field present). */
export type RecorderSettings = RecorderSettings_Serialize;
export type StreamDest = StreamDest_Serialize;
/** What the engine reports, without the UI-owned parts. */
export type EngineSnapshot = EngineState_Serialize;

/** The frame rates the engine accepts (rec-core `FPS_CHOICES`); settings carry a plain number. */
export type Fps = 24 | 25 | 30 | 50 | 60;
/** Countdown lengths in seconds (rec-core `COUNTDOWN_CHOICES`). */
export type CountdownSecs = 0 | 3 | 5;

export type ToastKind = "saved" | "replay" | "warning";
export interface Toast {
  id: string;
  kind: ToastKind;
  title: string;
  detail: string;
  recordingId?: string;
}

/**
 * The engine snapshot plus the stored settings and the UI's toasts: what `useEngine()` returns.
 * Audio levels are left out: they change ~30 times a second and live in their own store
 * (`micLevels` / `systemLevels` in `engine.ts`), read only by the meters.
 */
export type EngineState = Omit<EngineSnapshot, "levels"> & {
  settings: RecorderSettings;
  toasts: Toast[];
};
