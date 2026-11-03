import * as React from "react";

import { cn } from "@starter/ui/lib/utils";

import { MockScreen } from "../../prototype/mock-screen";
import {
  bucketLevels,
  clamp,
  formatShortClock,
  formatTimecode,
  MIN_SELECTION,
  rulerStep,
  seededWave,
  snapToFrame,
  TRIM_FPS,
} from "./time";

const SNAP_PX = 8; // handles within this distance of the playhead snap to it
const FILM_H = 52;
const WAVE_H = 30;

/**
 * SOURCE OF TRUTH KEYWORDS: TrimTimeline, filmstrip, waveform, trim handles, playhead scrub, in/out
 * WHAT:  The trim timeline: time ruler, thumbnail filmstrip, audio waveform, a draggable
 *        playhead and start/end handles that dim everything outside the kept range.
 * WHY:   One lane carries all trim gestures (scrub, drag handles, snap to playhead, nudge
 *        by frame) so the selection reads as one object, like a strip of film (NEW-TR-03).
 * WHERE: TrimWindow.
 */
export function TrimTimeline({
  recordingId,
  duration,
  inT,
  outT,
  time,
  zoom,
  playing,
  audioKept,
  onSeek,
  onScrub,
  onInChange,
  onOutChange,
  thumbnails,
  peaks,
}: {
  recordingId: string;
  duration: number;
  inT: number;
  outT: number;
  time: number;
  zoom: number;
  playing: boolean;
  /** False when every audio track is excluded; the waveform greys out. */
  audioKept: boolean;
  onSeek: (t: number) => void;
  /** Called with true when a scrub/drag starts and false when it ends. */
  onScrub: (active: boolean) => void;
  onInChange: (t: number) => void;
  onOutChange: (t: number) => void;
  /** Real frames, evenly spread over the file (the browser harness draws mock screens). */
  thumbnails?: string[];
  /** Real audio peaks 0..1, evenly spread over the file (the harness uses a seeded wave). */
  peaks?: number[];
}) {
  const scroller = React.useRef<HTMLDivElement>(null);
  const lane = React.useRef<HTMLDivElement>(null);
  const [viewW, setViewW] = React.useState(0);
  const [snapped, setSnapped] = React.useState(false);
  const [dragging, setDragging] = React.useState<"in" | "out" | "head" | null>(null);

  React.useLayoutEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => entry && setViewW(entry.contentRect.width));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const pad = 14; // room for the handles at the very start and end
  const laneW = Math.max(0, viewW * zoom - pad * 2);
  const pxPerSec = duration > 0 ? laneW / duration : 0;
  const pct = (t: number) => `${(t / duration) * 100}%`;

  // Keep the playhead centred when zoom (scale) changes.
  const timeRef = React.useRef(time);
  React.useLayoutEffect(() => {
    timeRef.current = time;
  }, [time]);
  React.useLayoutEffect(() => {
    const el = scroller.current;
    if (!el) return;
    el.scrollLeft = pad + timeRef.current * pxPerSec - el.clientWidth / 2;
  }, [pxPerSec]);

  // While playing, page the view so the playhead never leaves it.
  React.useEffect(() => {
    const el = scroller.current;
    if (!el || !playing) return;
    const x = pad + time * pxPerSec;
    if (x < el.scrollLeft || x > el.scrollLeft + el.clientWidth - 24) el.scrollLeft = x - el.clientWidth * 0.2;
  }, [time, playing, pxPerSec]);

  function timeAt(clientX: number) {
    const rect = lane.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return 0;
    return snapToFrame(clamp(((clientX - rect.left) / rect.width) * duration, 0, duration));
  }

  // Scrub: press anywhere on the lane (outside the handles) and drag.
  function onLaneDown(e: React.PointerEvent<HTMLDivElement>) {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    setDragging("head");
    onScrub(true);
    onSeek(timeAt(e.clientX));
  }
  function onLaneMove(e: React.PointerEvent<HTMLDivElement>) {
    if (dragging === "head") onSeek(timeAt(e.clientX));
  }
  function onLaneUp() {
    if (dragging !== "head") return;
    setDragging(null);
    onScrub(false);
  }

  function handleDrag(which: "in" | "out") {
    return {
      onPointerDown(e: React.PointerEvent<HTMLDivElement>) {
        if (e.button !== 0) return;
        e.stopPropagation();
        e.currentTarget.setPointerCapture(e.pointerId);
        setDragging(which);
      },
      onPointerMove(e: React.PointerEvent<HTMLDivElement>) {
        if (dragging !== which) return;
        let t = timeAt(e.clientX);
        const snap = Math.abs(t - time) * pxPerSec < SNAP_PX;
        if (snap) t = time;
        setSnapped(snap);
        if (which === "in") onInChange(clamp(t, 0, outT - MIN_SELECTION));
        else onOutChange(clamp(t, inT + MIN_SELECTION, duration));
      },
      onPointerUp() {
        setDragging(null);
        setSnapped(false);
      },
    };
  }

  // Keyboard on a focused handle: ←/→ one frame, ⇧ one second.
  function handleKeys(which: "in" | "out") {
    return (e: React.KeyboardEvent) => {
      const step = e.shiftKey ? 1 : 1 / TRIM_FPS;
      let d = 0;
      if (e.key === "ArrowLeft") d = -step;
      else if (e.key === "ArrowRight") d = step;
      else return;
      e.preventDefault();
      e.stopPropagation();
      if (which === "in") onInChange(snapToFrame(clamp(inT + d, 0, outT - MIN_SELECTION)));
      else onOutChange(snapToFrame(clamp(outT + d, inT + MIN_SELECTION, duration)));
    };
  }

  const step = rulerStep(pxPerSec || 1);
  const ticks: number[] = [];
  for (let t = 0; t <= duration + 1e-6; t += step / 2) ticks.push(t);

  const frames = Math.round(clamp(12 * zoom, 12, 40));
  // Levels at 4 per second, shown as peak bars at roughly one per 4 px.
  const levels = React.useMemo(
    () => peaks ?? seededWave(recordingId, Math.round(duration * 4)),
    [peaks, recordingId, duration],
  );
  const bars = React.useMemo(() => bucketLevels(levels, Math.max(60, Math.round(laneW / 4))), [levels, laneW]);

  return (
    <div
      ref={scroller}
      className="relative overflow-x-auto overflow-y-hidden [scrollbar-width:thin]"
      aria-label="Timeline"
    >
      <div style={{ width: laneW + pad * 2, paddingInline: pad }} className="pt-1 pb-2">
        <div
          ref={lane}
          className={cn("relative touch-none select-none", dragging === "head" ? "cursor-grabbing" : "cursor-text")}
          onPointerDown={onLaneDown}
          onPointerMove={onLaneMove}
          onPointerUp={onLaneUp}
          onPointerCancel={onLaneUp}
        >
          {/* Ruler */}
          <div className="relative h-5" aria-hidden>
            {ticks.map((t) => {
              const major = Math.abs(t / step - Math.round(t / step)) < 1e-6;
              return (
                <div key={t} className="absolute bottom-0" style={{ left: pct(t) }}>
                  <div className={cn("w-px bg-border", major ? "h-2" : "h-1")} />
                  {major && (
                    <span className="absolute bottom-2.5 left-1.5 font-mono text-[10px] text-muted-foreground tabular">
                      {formatShortClock(t)}
                    </span>
                  )}
                </div>
              );
            })}
          </div>

          {/* Filmstrip */}
          <div
            className="relative mt-1 flex overflow-hidden rounded-[8px] bg-stage"
            style={{ height: FILM_H }}
            aria-hidden
          >
            {Array.from({ length: frames }, (_, i) => (
              <div key={i} className="relative h-full flex-1 border-r border-stage last:border-r-0">
                {thumbnails === undefined ? (
                  <MockScreen variant={sceneAt(i / frames)} className="pointer-events-none" />
                ) : (
                  <FilmFrame src={thumbnails[Math.floor(((i + 0.5) / frames) * thumbnails.length)]} />
                )}
              </div>
            ))}
          </div>

          {/* Waveform */}
          <div
            className="relative mt-1.5 overflow-hidden rounded-[8px] bg-muted"
            style={{ height: WAVE_H }}
            aria-hidden
          >
            <svg
              className="size-full"
              viewBox={`0 0 ${bars.length} 100`}
              preserveAspectRatio="none"
              role="presentation"
            >
              {bars.map((v, i) => (
                <rect
                  key={i}
                  x={i + 0.2}
                  width={0.6}
                  y={50 - v * 44}
                  height={v * 88}
                  className={audioKept ? "fill-accent-text/70" : "fill-muted-foreground/30"}
                />
              ))}
            </svg>
            {!audioKept && (
              <span className="absolute inset-0 grid place-items-center">
                <span className="rounded-full bg-card px-2 py-0.5 text-[11px] text-muted-foreground shadow-ledge">
                  No audio tracks kept
                </span>
              </span>
            )}
          </div>

          {/* Outside the selection: dimmed */}
          <div
            className="pointer-events-none absolute bottom-0 left-0 rounded-l-[8px] bg-stage/60 backdrop-grayscale"
            style={{ top: 24, width: pct(inT) }}
          />
          <div
            className="pointer-events-none absolute right-0 bottom-0 rounded-r-[8px] bg-stage/60 backdrop-grayscale"
            style={{ top: 24, left: pct(outT) }}
          />

          {/* Selection frame + handles */}
          <div
            className="pointer-events-none absolute bottom-0 rounded-[9px] border-y-2 border-accent-solid"
            style={{ top: 22, left: pct(inT), width: pct(outT - inT) }}
          />
          <Handle
            side="in"
            left={pct(inT)}
            active={dragging === "in"}
            value={inT}
            min={0}
            max={outT - MIN_SELECTION}
            onKeyDown={handleKeys("in")}
            {...handleDrag("in")}
          />
          <Handle
            side="out"
            left={pct(outT)}
            active={dragging === "out"}
            value={outT}
            min={inT + MIN_SELECTION}
            max={duration}
            onKeyDown={handleKeys("out")}
            {...handleDrag("out")}
          />

          {/* Playhead */}
          <div className="pointer-events-none absolute top-0 bottom-0 z-20" style={{ left: pct(time) }}>
            <div
              className={cn(
                "absolute top-0 -left-[5px] h-3 w-[11px] rounded-t-[3px] rounded-b-[6px] transition-colors",
                snapped ? "bg-accent-solid" : "bg-foreground",
              )}
            />
            <div
              className={cn(
                "absolute top-2 bottom-0 -left-px w-0.5 transition-colors",
                snapped ? "bg-accent-solid" : "bg-foreground",
              )}
            />
          </div>
        </div>
      </div>
    </div>
  );
}

function Handle({
  side,
  left,
  active,
  value,
  min,
  max,
  onKeyDown,
  onPointerDown,
  onPointerMove,
  onPointerUp,
}: {
  side: "in" | "out";
  left: string;
  active: boolean;
  value: number;
  min: number;
  max: number;
  onKeyDown: (e: React.KeyboardEvent) => void;
  onPointerDown: (e: React.PointerEvent<HTMLDivElement>) => void;
  onPointerMove: (e: React.PointerEvent<HTMLDivElement>) => void;
  onPointerUp: () => void;
}) {
  const label = side === "in" ? "Trim start" : "Trim end";
  return (
    <div
      role="slider"
      tabIndex={0}
      aria-label={label}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={value}
      aria-valuetext={formatTimecode(value)}
      title={`${label} — drag, or use ← → (⇧ for 1 s)`}
      onKeyDown={onKeyDown}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      className={cn(
        "absolute bottom-0 z-10 grid w-3 cursor-ew-resize touch-none place-items-center bg-accent-solid outline-none",
        "transition-[box-shadow] duration-150 focus-visible:ring-3 focus-visible:ring-ring/40",
        side === "in" ? "-translate-x-full rounded-l-[8px]" : "rounded-r-[8px]",
        active && "ring-3 ring-ring/30",
      )}
      style={{ top: 22, left }}
    >
      <span className="h-5 w-0.5 rounded-full bg-primary-foreground/80" />
    </div>
  );
}

// Fake scene changes so the filmstrip reads like a real walkthrough.
function sceneAt(p: number): "desktop" | "window" {
  return p < 0.25 || (p > 0.55 && p < 0.7) ? "desktop" : "window";
}

/** One filmstrip tile from a real thumbnail; empty while thumbnails load. */
function FilmFrame({ src }: { src: string | undefined }) {
  if (src === undefined) return null;
  return <div className="size-full bg-cover bg-center" style={{ backgroundImage: `url("${src}")` }} />;
}
