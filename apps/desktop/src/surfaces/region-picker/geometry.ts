/**
 * SOURCE OF TRUTH KEYWORDS: region geometry, Rect, rectFromDrag, resizeRect, moveRect, aspect presets, hit test, snap targets
 * WHAT:  Pure geometry for the region picker: drawing, resizing with aspect locks, moving,
 *        nudging, numeric sizing and window hit-testing, all in display pixels.
 * WHY:   Kept free of React so the maths can be unit-tested (NEW-RG-02/03 test rows) and
 *        reused unchanged when the real per-display overlay windows exist.
 * WHERE: surfaces/region-picker.tsx and its subcomponents.
 */

export interface Pt {
  x: number;
  y: number;
}
export interface Size {
  w: number;
  h: number;
}
export interface Rect extends Pt, Size {}

export type Aspect = "free" | "16:9" | "9:16" | "1:1" | "4:3";
export const ASPECTS: ReadonlyArray<{ value: Aspect; label: string }> = [
  { value: "free", label: "Free" },
  { value: "16:9", label: "16:9" },
  { value: "9:16", label: "9:16" },
  { value: "1:1", label: "1:1" },
  { value: "4:3", label: "4:3" },
];

export function aspectRatio(a: Aspect): number | null {
  switch (a) {
    case "16:9":
      return 16 / 9;
    case "9:16":
      return 9 / 16;
    case "1:1":
      return 1;
    case "4:3":
      return 4 / 3;
    default:
      return null;
  }
}

export type Handle = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";
export const HANDLES: readonly Handle[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];

/** Smallest region worth recording, in display pixels. */
export const MIN_SIZE = 16;

const EPS = 1e-6;

export function clamp(v: number, lo: number, hi: number) {
  return Math.min(hi, Math.max(lo, v));
}

function round(r: Rect, b: Size): Rect {
  const w = clamp(Math.round(r.w), 0, b.w);
  const h = clamp(Math.round(r.h), 0, b.h);
  return { x: clamp(Math.round(r.x), 0, b.w - w), y: clamp(Math.round(r.y), 0, b.h - h), w, h };
}

/** Rect spanned from a fixed anchor to the pointer, optionally locked to a ratio and kept on the display. */
export function rectFromDrag(anchor: Pt, p: Pt, ratio: number | null, b: Size): Rect {
  const dx = p.x - anchor.x;
  const dy = p.y - anchor.y;
  let w = Math.abs(dx);
  let h = Math.abs(dy);
  // Locked ratio follows the larger pull, then shrinks to fit the display.
  if (ratio) {
    if (w / Math.max(h, EPS) > ratio) h = w / ratio;
    else w = h * ratio;
  }
  const maxW = dx >= 0 ? b.w - anchor.x : anchor.x;
  const maxH = dy >= 0 ? b.h - anchor.y : anchor.y;
  if (ratio) {
    const k = Math.min(1, maxW / Math.max(w, EPS), maxH / Math.max(h, EPS));
    w *= k;
    h *= k;
  } else {
    w = Math.min(w, maxW);
    h = Math.min(h, maxH);
  }
  return round({ x: dx >= 0 ? anchor.x : anchor.x - w, y: dy >= 0 ? anchor.y : anchor.y - h, w, h }, b);
}

/** Resize from one of the 8 handles. Corners keep the opposite corner fixed; edges keep the opposite edge. */
export function resizeRect(start: Rect, handle: Handle, p: Pt, ratio: number | null, b: Size): Rect {
  const left = start.x;
  const top = start.y;
  const right = start.x + start.w;
  const bottom = start.y + start.h;

  if (handle.length === 2) {
    const anchor = { x: handle.includes("w") ? right : left, y: handle.includes("n") ? bottom : top };
    return rectFromDrag(anchor, p, ratio, b);
  }

  if (handle === "e" || handle === "w") {
    const ax = handle === "e" ? left : right;
    const dx = p.x - ax;
    let w = Math.min(Math.abs(dx), dx >= 0 ? b.w - ax : ax);
    let h = start.h;
    let y = start.y;
    if (ratio) {
      h = w / ratio;
      if (h > b.h) {
        h = b.h;
        w = h * ratio;
      }
      y = clamp(start.y + start.h / 2 - h / 2, 0, b.h - h);
    }
    return round({ x: dx >= 0 ? ax : ax - w, y, w, h }, b);
  }

  const ay = handle === "s" ? top : bottom;
  const dy = p.y - ay;
  let h = Math.min(Math.abs(dy), dy >= 0 ? b.h - ay : ay);
  let w = start.w;
  let x = start.x;
  if (ratio) {
    w = h * ratio;
    if (w > b.w) {
      w = b.w;
      h = w / ratio;
    }
    x = clamp(start.x + start.w / 2 - w / 2, 0, b.w - w);
  }
  return round({ x, y: dy >= 0 ? ay : ay - h, w, h }, b);
}

/** Translate, stopping at the display edges. */
export function moveRect(r: Rect, dx: number, dy: number, b: Size): Rect {
  return round({ ...r, x: clamp(r.x + dx, 0, b.w - r.w), y: clamp(r.y + dy, 0, b.h - r.h) }, b);
}

/** Grow or shrink from the top-left corner (keyboard ⌥-arrows). */
export function growRect(r: Rect, dw: number, dh: number, b: Size): Rect {
  return round({ ...r, w: clamp(r.w + dw, MIN_SIZE, b.w - r.x), h: clamp(r.h + dh, MIN_SIZE, b.h - r.y) }, b);
}

/** Set one dimension from the numeric fields; the other follows a locked ratio. Shifts left/up if it would overflow. */
export function setRectSize(r: Rect, which: "w" | "h", value: number, ratio: number | null, b: Size): Rect {
  let w = which === "w" ? value : r.w;
  let h = which === "h" ? value : r.h;
  if (ratio) {
    if (which === "w") h = w / ratio;
    else w = h * ratio;
  }
  const k = Math.min(1, b.w / Math.max(w, EPS), b.h / Math.max(h, EPS));
  w = Math.max(MIN_SIZE, w * k);
  h = Math.max(MIN_SIZE, h * k);
  return round({ x: Math.min(r.x, b.w - w), y: Math.min(r.y, b.h - h), w, h }, b);
}

/** Reshape an existing rect to a newly picked ratio, keeping its centre and roughly its width. */
export function fitAspect(r: Rect, ratio: number, b: Size): Rect {
  let w = r.w;
  let h = w / ratio;
  const k = Math.min(1, b.w / w, b.h / h);
  w *= k;
  h *= k;
  const cx = r.x + r.w / 2;
  const cy = r.y + r.h / 2;
  return round({ x: clamp(cx - w / 2, 0, b.w - w), y: clamp(cy - h / 2, 0, b.h - h), w, h }, b);
}

export function contains(r: Rect, p: Pt) {
  return p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h;
}

/** A window that is visible on this display and can be snapped to. */
export interface SnapTarget {
  windowId: number;
  app: string;
  title: string;
  frame: Rect;
  /** Corner radius as a fraction of the frame width (matches the mock desktop's drawing). */
  radius: number;
}

/** Topmost target under the pointer. `targets` is ordered back to front. */
export function targetAt(targets: readonly SnapTarget[], p: Pt): SnapTarget | null {
  for (let i = targets.length - 1; i >= 0; i -= 1) {
    const t = targets[i];
    if (t && contains(t.frame, p)) return t;
  }
  return null;
}
