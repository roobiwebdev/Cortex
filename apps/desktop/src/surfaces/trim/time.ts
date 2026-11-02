/**
 * SOURCE OF TRUTH KEYWORDS: trim time math, formatTimecode, snapToFrame, rulerStep, seededWave, TRIM_FPS
 * WHAT:  Pure helpers for the Quick trim window: frame snapping, precise timecodes,
 *        ruler spacing, deterministic waveform bars and the export easing curve.
 * WHY:   Time math is the part of trim that must be exact (NEW-TR-03); keeping it pure
 *        makes it testable and identical between the timeline, readouts and keys.
 * WHERE: surfaces/trim/* only.
 */

/** Frame grid used for nudges and snapping. The real value comes from the file's track. */
export const TRIM_FPS = 60;
/** Shortest selection the handles allow, in seconds. */
export const MIN_SELECTION = 0.5;

export function clamp(v: number, min: number, max: number) {
  return Math.min(max, Math.max(min, v));
}

export function snapToFrame(secs: number) {
  return Math.round(secs * TRIM_FPS) / TRIM_FPS;
}

/** "mm:ss.cc" (or "h:mm:ss.cc"): precise enough to read a frame-level change. */
export function formatTimecode(totalSecs: number) {
  const t = Math.max(0, totalSecs);
  const whole = Math.floor(t);
  const cs = Math.min(99, Math.round((t - whole) * 100));
  const h = Math.floor(whole / 3600);
  const m = Math.floor((whole % 3600) / 60);
  const s = whole % 60;
  const body = `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}.${String(cs).padStart(2, "0")}`;
  return h > 0 ? `${h}:${body}` : body;
}

/** "6:52" style short duration for labels and the ruler. */
export function formatShortClock(totalSecs: number) {
  const secs = Math.max(0, Math.round(totalSecs));
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

const RULER_STEPS = [0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600];

/** Smallest labelled ruler step that keeps labels at least `minPx` apart. */
export function rulerStep(pxPerSec: number, minPx = 72) {
  return RULER_STEPS.find((s) => s * pxPerSec >= minPx) ?? 600;
}

/** Deterministic 0..1 levels: speech phrases with pauses over a quieter system-audio bed. */
export function seededWave(seed: string, count: number) {
  let h = 2166136261;
  for (const ch of seed) h = Math.imul(h ^ ch.charCodeAt(0), 16777619);
  const rand = () => {
    h += 0x6d2b79f5;
    let x = h;
    x = Math.imul(x ^ (x >>> 15), x | 1);
    x ^= x + Math.imul(x ^ (x >>> 7), x | 61);
    return ((x ^ (x >>> 14)) >>> 0) / 4294967296;
  };
  const raw: number[] = [];
  let speaking = true;
  let left = 0;
  let loud = 0.7;
  for (let i = 0; i < count; i++) {
    if (left <= 0) {
      speaking = !speaking;
      left = speaking ? 10 + rand() * 40 : 2 + rand() * 9;
      loud = 0.45 + rand() * 0.5;
    }
    left -= 1;
    const bed = 0.08 + rand() * 0.07;
    raw.push(speaking ? bed + loud * (0.35 + rand() * 0.65) : bed);
  }
  // Light smoothing so syllables read as shapes, not noise.
  return raw.map((v, i) => clamp(((raw[i - 1] ?? v) + v * 2 + (raw[i + 1] ?? v)) / 4, 0.04, 1));
}

/** Downsample levels to `count` bars, blending each bucket's mean and peak so pauses survive. */
export function bucketLevels(levels: number[], count: number) {
  if (levels.length <= count) return levels;
  const out: number[] = [];
  const size = levels.length / count;
  for (let b = 0; b < count; b++) {
    let peak = 0;
    let sum = 0;
    const from = Math.floor(b * size);
    const to = Math.max(from + 1, Math.floor((b + 1) * size));
    for (let i = from; i < to; i++) {
      const v = levels[i] ?? 0;
      peak = Math.max(peak, v);
      sum += v;
    }
    out.push((sum / (to - from)) * 0.6 + peak * 0.4);
  }
  return out;
}

export function easeInOutCubic(x: number) {
  return x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2;
}
