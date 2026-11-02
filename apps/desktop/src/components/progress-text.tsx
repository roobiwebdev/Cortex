import { useClock, useProgress } from "../lib/engine";
import { formatBytes, formatClock } from "../lib/format";

/**
 * SOURCE OF TRUTH KEYWORDS: ElapsedClock, BytesWritten, DroppedFrames, FreeDisk, progress text, 1 Hz
 * WHAT:  Text that shows the recording's counters: elapsed clock, bytes written, dropped
 *        frames and free disk. Each subscribes to the engine itself and renders bare text.
 * WHY:   The engine reports progress 10 times a second. Only these leaves re-render (once a
 *        second), so the bars and panels around them stay still while recording.
 * WHERE: main-window title bar and record bar, menu-bar panel, HUD.
 */
export function ElapsedClock() {
  return <>{formatClock(useClock())}</>;
}

export function BytesWritten() {
  return <>{formatBytes(useProgress().bytesWritten)}</>;
}

export function DroppedFrames() {
  const n = useProgress().droppedFrames;
  return <>{n === 0 ? "no dropped frames" : `${n} dropped`}</>;
}

export function FreeDisk() {
  return <>{formatBytes(useProgress().freeDiskBytes)}</>;
}
