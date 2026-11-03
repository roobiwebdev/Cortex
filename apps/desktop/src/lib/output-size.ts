import type { Codec, EngineSnapshot, EngineState, RecorderSettings, Resolution } from "./types";

/** A frame size in pixels. */
export interface PixelSize {
  w: number;
  h: number;
}

/** The long side of a "4K" frame (rec-core `UHD_WIDTH`). */
const UHD_WIDTH = 3840;
/** H.264 "4K" stays within 3840×2160's pixels (rec-core `H264_MAX_PIXELS`). */
const H264_MAX_PIXELS = 3840 * 2160;
/** Frames above this many pixels (1440p) get the 4K-class bitrate (rec-core `UHD_CLASS_MIN_PIXELS`). */
const UHD_CLASS_MIN_PIXELS = 2560 * 1440;
const BOX: Record<"720p" | "1080p" | "1440p", PixelSize> = {
  "720p": { w: 1280, h: 720 },
  "1080p": { w: 1920, h: 1080 },
  "1440p": { w: 2560, h: 1440 },
};

/** Nearest even number, at least 2 (Rust `f64::round` and `Math.round` agree for positive values). */
const even = (x: number) => Math.max(2, Math.round(x / 2) * 2);
/** Rust's `x & !1`, at least 2. */
const downEven = (x: number) => Math.max(2, Math.floor(x / 2) * 2);

/**
 * SOURCE OF TRUTH KEYWORDS: outputSize, Resolution, 4K, upscale, recorded frame size, native, picker detail
 * WHAT:  The recorded frame size for a Resolution setting and a source size in pixels.
 * WHY:   The pickers show the real output ("4K · 3840×2482"). Mirrors rec-core
 *        `output::output_size`, which the engine records with; both are tested against
 *        `crates/rec-core/tests/fixtures/output-sizes.json` (`pnpm --filter desktop test`).
 *        4K is 3840 on the long side (upscaling allowed); 720p/1080p/1440p fit inside their
 *        box (swapped for portrait) and never upscale; Native is the source rounded down to even.
 *        H.264 4K stays within 3840×2160's pixels (the hardware encoder's real-time limit).
 * WHERE: Settings → Recording, the main window's format chip, the mock engine's estimates.
 */
export function outputSize(res: Resolution, source: PixelSize, codec: Codec): PixelSize {
  const nw = downEven(source.w);
  const nh = downEven(source.h);
  if (res === "native") return { w: nw, h: nh };
  let k: number;
  if (res === "4k") {
    k = UHD_WIDTH / Math.max(nw, nh);
  } else {
    const box = BOX[res];
    const [bw, bh] = nh > nw ? [box.h, box.w] : [box.w, box.h];
    k = Math.min(bw / nw, bh / nh, 1);
  }
  const w = even(nw * k);
  const h = even(nh * k);
  if (res === "4k" && codec === "h264" && w * h > H264_MAX_PIXELS) {
    // Largest frame of the source's shape under the cap: round down, not to nearest.
    const kc = Math.sqrt(H264_MAX_PIXELS / (nw * nh));
    const down = (x: number) => Math.max(2, Math.floor(x / 2) * 2);
    return { w: down(nw * kc), h: down(nh * kc) };
  }
  return { w, h };
}

/**
 * SOURCE OF TRUTH KEYWORDS: resolveCodec, Auto codec, HEVC, H.264, codec label
 * WHAT:  The codec "Auto" records for a frame size: H.264 up to 1080p, HEVC above.
 * WHY:   Labels and bitrate estimates must name what is written. Mirrors rec-core `output::resolve_codec`.
 * WHERE: the main window's format chip, Settings → Recording bitrate hint.
 */
export function resolveCodec(codec: Codec, size: PixelSize): Exclude<Codec, "auto"> {
  if (codec !== "auto") return codec;
  return size.w * size.h > 1920 * 1080 ? "hevc" : "h264";
}

/**
 * SOURCE OF TRUTH KEYWORDS: autoBitrate, bits per pixel, bitrate estimate, 4K-class, Mb/s
 * WHAT:  The automatic video bitrate (bits per second) for a codec, frame size and frame rate.
 * WHY:   "Auto" should not be a black box. Mirrors rec-core `output::auto_bitrate`: pixels × fps ×
 *        bits per pixel (H.264 0.16, HEVC 0.10; above 1440p 0.20 and 0.14), clamped to 1–400 Mb/s.
 * WHERE: Settings → Recording and Storage, the mock engine.
 */
export function autoBitrate(codec: Codec, size: PixelSize, fps: number): number {
  const uhd = size.w * size.h > UHD_CLASS_MIN_PIXELS;
  const c = resolveCodec(codec, size);
  const bpp = c === "prores" ? 0 : c === "h264" ? (uhd ? 0.2 : 0.16) : uhd ? 0.14 : 0.1;
  return Math.min(400_000_000, Math.max(1_000_000, Math.trunc(size.w * size.h * fps * bpp)));
}

/**
 * SOURCE OF TRUTH KEYWORDS: sourcePixels, capture source size, target pixels, region, window, display
 * WHAT:  The size in pixels of what the capture target delivers, or null while it is unknown.
 * WHY:   The output size depends on it. Mirrors how `rec-engine-macos::args::prepare` sizes the
 *        source: a display's pixels, a region (clipped to its display) × scale, a window's frame
 *        × its display's scale, an app's display.
 * WHERE: `outputSizes`, the preview stage's aspect ratio.
 */
export function sourcePixels(
  target: RecorderSettings["target"],
  e: Pick<EngineSnapshot, "displays" | "windows">,
): PixelSize | null {
  const display = (id: number) => e.displays.find((d) => d.id === id);
  switch (target.kind) {
    case "display": {
      const d = display(target.displayId);
      return d ? { w: d.width, h: d.height } : null;
    }
    case "region": {
      const d = display(target.displayId);
      if (!d) return null;
      const x0 = Math.max(0, target.x);
      const y0 = Math.max(0, target.y);
      const w = Math.min(target.x + target.w, d.frame.w) - x0;
      const h = Math.min(target.y + target.h, d.frame.h) - y0;
      return w > 0 && h > 0 ? { w: Math.round(w * d.scale), h: Math.round(h * d.scale) } : null;
    }
    case "window": {
      const win = e.windows.find((x) => x.id === target.windowId);
      if (!win) return null;
      const scale = display(win.displayId)?.scale ?? e.displays[0]?.scale ?? 1;
      return { w: Math.trunc(win.frame.w * scale), h: Math.trunc(win.frame.h * scale) };
    }
    case "app": {
      // Recorded on the display holding the app's frontmost window.
      const win = e.windows.find((x) => x.bundleId === target.bundleId);
      const d = (win && display(win.displayId)) ?? e.displays[0];
      return d ? { w: d.width, h: d.height } : null;
    }
  }
}

export const RESOLUTIONS: readonly Resolution[] = ["720p", "1080p", "1440p", "4k", "native"];

/** What the next recording writes with the current settings. */
export interface RecordedOutput {
  source: PixelSize;
  size: PixelSize;
  codec: Exclude<Codec, "auto">;
  /** The manual bitrate, else the automatic one (unused by ProRes). */
  bitsPerSecond: number;
}

/**
 * SOURCE OF TRUTH KEYWORDS: recordedOutput, sameOutput, output chip, current output, bitrate estimate, useEngine selector
 * WHAT:  The size, codec and bitrate the next recording gets for the current target and settings,
 *        or null while the source size is unknown (a window that closed, no displays yet).
 * WHY:   The format chip, the Resolution picker, the bitrate hint and the storage estimate read
 *        the same numbers. Pass it to `useEngine(recordedOutput, sameOutput)`.
 * WHERE: main window format chip, Settings → Recording and Storage.
 */
export function recordedOutput(e: Pick<EngineState, "settings" | "displays" | "windows">): RecordedOutput | null {
  const s = e.settings;
  const source = sourcePixels(s.target, e);
  if (!source) return null;
  const size = outputSize(s.resolution, source, s.codec);
  const codec = resolveCodec(s.codec, size);
  const bitsPerSecond = s.bitrateMbps === null ? autoBitrate(codec, size, s.fps) : s.bitrateMbps * 1_000_000;
  return { source, size, codec, bitsPerSecond };
}

export function sameOutput(a: RecordedOutput | null, b: RecordedOutput | null): boolean {
  if (a === null || b === null) return a === b;
  return (
    a.source.w === b.source.w &&
    a.source.h === b.source.h &&
    a.size.w === b.size.w &&
    a.size.h === b.size.h &&
    a.codec === b.codec &&
    a.bitsPerSecond === b.bitsPerSecond
  );
}

/** "3840×2482" */
export const formatSize = (s: PixelSize) => `${s.w}×${s.h}`;
