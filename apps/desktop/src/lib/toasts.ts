import type { EngineError, Recording, Toast, ToastKind } from "./types";

/**
 * SOURCE OF TRUTH KEYWORDS: toasts, savedToast, replayToast, errorToast, pushToastTo, engine events, UI-06
 * WHAT:  Builds the UI's toasts from engine events (recording finished, replay saved, error)
 *        and keeps the queue short.
 * WHY:   Toasts are UI-owned: the engine only reports what happened. The Tauri adapter and
 *        the browser mock both use these, so the same event reads the same everywhere.
 *        In the app the toasts window owns the queue (see `engine.ts`).
 * WHERE: `engine.ts` (Tauri events, failed commands), `mock-engine.ts`.
 */

/** At most this many toasts are queued, newest last. */
export const MAX_TOASTS = 3;

let seq = 0;
function nextId() {
  seq += 1;
  return `t${seq}`;
}

/** A fresh toast id for this window's queue (toasts sent from other windows get one). */
export const newToastId = nextId;

const KINDS: ReadonlyArray<ToastKind> = ["saved", "replay", "warning"];

/** Narrows a toast received from another window (`ui://toast`). */
export function isToast(v: unknown): v is Toast {
  if (typeof v !== "object" || v === null) return false;
  const kind = "kind" in v ? v.kind : undefined;
  const ok = (k: string) => k in v && typeof Reflect.get(v, k) === "string";
  const rid = "recordingId" in v ? v.recordingId : undefined;
  return (
    ok("id") &&
    ok("title") &&
    ok("detail") &&
    KINDS.some((k) => k === kind) &&
    (rid === undefined || typeof rid === "string")
  );
}

/** `m:ss` for a duration in seconds. */
export function formatShort(secs: number) {
  const total = Math.max(0, Math.round(secs));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

function extension(path: string) {
  const dot = path.lastIndexOf(".");
  return dot >= 0 ? path.slice(dot + 1).toUpperCase() : "";
}

export function savedToast(r: Recording): Toast {
  const ext = extension(r.path);
  return {
    id: nextId(),
    kind: "saved",
    title: r.name,
    detail: ext ? `${formatShort(r.durationSecs)} · ${ext}` : formatShort(r.durationSecs),
    recordingId: r.id,
  };
}

export function replayToast(path: string, replaySecs: number): Toast {
  return {
    id: nextId(),
    kind: "replay",
    title: "Replay saved",
    detail: `Last ${replaySecs}s · ${extension(path) || "MP4"}`,
  };
}

export function warningToast(title: string, detail: string): Toast {
  return { id: nextId(), kind: "warning", title, detail };
}

/** An engine `error` event: recoverable ones are warnings, the rest stopped something. */
export function errorToast(message: string, recoverable: boolean): Toast {
  return warningToast(recoverable ? "Heads up" : "Recording stopped", message);
}

/** The user-facing text of a failed command. */
export function errorMessage(e: EngineError): string {
  switch (e.kind) {
    case "invalid-state":
      return `Can't ${e.action.split("-").join(" ")} while ${e.status}`;
    case "missing-permissions":
      return `Cortex needs ${e.permissions.join(", ")} access`;
    case "unsupported":
    case "failed":
      return e.message;
  }
}

/** `toasts` with `t` appended and trimmed to `MAX_TOASTS`. */
export function pushToastTo(toasts: Toast[], t: Toast): Toast[] {
  return [...toasts, t].slice(-MAX_TOASTS);
}
