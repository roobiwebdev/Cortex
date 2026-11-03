import { isTauri } from "@tauri-apps/api/core";

import { commands, events, type EngineError, type Recording, type TrimRequest } from "../../ipc/bindings";
import { errorMessage, warningToast } from "../../lib/toasts";

/** Thumbnails the timeline asks for once; the filmstrip picks from them at every zoom. */
export const THUMBNAIL_COUNT = 40;
/** Waveform peaks per recording; the timeline buckets them down to its bars. */
export const WAVEFORM_BUCKETS = 1200;

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: EngineError };

export class TrimCommandError extends Error {
  readonly cancelled: boolean;
  constructor(message: string) {
    super(message);
    this.cancelled = message === "Export cancelled";
  }
}

async function unwrap<T>(call: Promise<Result<T>>): Promise<T> {
  const r = await call;
  if (r.status === "ok") return r.data;
  throw new TrimCommandError(errorMessage(r.error));
}

/** A failure the user should hear about, through the shell's toasts window. */
export function reportTrimError(title: string, err: unknown) {
  if (err instanceof TrimCommandError && err.cancelled) return;
  const detail = err instanceof Error ? err.message : String(err);
  commands
    .pushToast({ ...warningToast(title, detail), recordingId: null })
    .catch((e: unknown) => console.error("cortex: toast not sent", e));
}

/** Peaks scaled so the loudest is 1, with a square-root curve so quiet speech still shows. */
export function normalizePeaks(peaks: number[]) {
  const max = peaks.reduce((m, v) => Math.max(m, v), 0);
  if (max <= 0) return peaks.map(() => 0);
  return peaks.map((v) => Math.sqrt(v / max));
}

/**
 * SOURCE OF TRUTH KEYWORDS: nativeTrim, cortex-media, trim export, recording thumbnails, waveform, cancel trim
 * WHAT:  The Trim window's calls into the shell: the playable URL of a recording, timeline
 *        thumbnails and waveform peaks, export with progress, cancel, rename and reveal.
 *        `null` in the browser harness, which keeps its simulated player and mock data.
 * WHY:   One place maps the generated commands to promises and errors, so trim.tsx only
 *        decides between real and simulated paths (NEW-TR-02..06).
 * WHERE: surfaces/trim.tsx.
 */
export const nativeTrim = isTauri()
  ? {
      /** Served by the shell's `cortex-media` scheme, to the Trim window only. */
      mediaUrl(id: string) {
        return `cortex-media://localhost/${encodeURIComponent(id)}`;
      },
      thumbnails(id: string) {
        return unwrap(commands.recordingThumbnails(id, THUMBNAIL_COUNT));
      },
      async waveform(id: string) {
        return normalizePeaks(await unwrap(commands.recordingWaveform(id, WAVEFORM_BUCKETS)));
      },
      async export(id: string, request: TrimRequest, onProgress: (p: number) => void): Promise<Recording> {
        const unlisten = await events.trimProgress.listen((e) => {
          if (e.payload.id === id) onProgress(e.payload.progress);
        });
        try {
          return await unwrap(commands.trimRecording(id, request));
        } finally {
          unlisten();
        }
      },
      cancel(id: string) {
        return commands.cancelTrim(id);
      },
      rename(id: string, name: string) {
        return unwrap(commands.renameRecording(id, name));
      },
      reveal(path: string) {
        return unwrap(commands.revealRecording(path));
      },
    }
  : null;
