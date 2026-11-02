import * as React from "react";

import { cn } from "@starter/ui/lib/utils";

import { hasNativePreview, useNativePreviewHole } from "../lib/native-preview";
import type { PipCorner, PipPlace, RecorderSettings } from "../lib/types";
import { CameraBubble } from "./camera-bubble";

const MARGIN = 0.03; // overlay inset from the frame edge, as a fraction of frame width
const SNAP = 0.12; // release within this distance of a corner slot snaps to it

/**
 * SOURCE OF TRUTH KEYWORDS: PreviewStage, live preview, stage, pip drag, snap to corner, overlay editor
 * WHAT:  The dark stage that holds the live preview, with the camera overlay that can be
 *        dragged anywhere and snaps to corners (with guides) on release.
 * WHY:   The video content is a slot: the prototype passes a mock screen, the real app
 *        leaves a transparent hole for the native Metal preview view. The overlay
 *        handles stay in React either way, so editing feels identical.
 *        In the Tauri main window the stage paints no background of its own: the hole's
 *        box-shadow paints the dark surround inside the stage, and a fixed backdrop paints
 *        the page around the stage, so nothing above the native view is opaque.
 * WHERE: main window centre.
 */
export function PreviewStage({
  settings,
  aspect,
  recording,
  paused,
  cameraStream,
  onPipPlace,
  children,
}: {
  settings: RecorderSettings;
  /** width / height of the output frame */
  aspect: number;
  recording: boolean;
  paused: boolean;
  cameraStream?: MediaStream | null;
  onPipPlace: (place: PipPlace) => void;
  children: React.ReactNode;
}) {
  const stage = React.useRef<HTMLDivElement>(null);
  const outer = React.useRef<HTMLDivElement>(null);
  const frame = React.useRef<HTMLDivElement>(null);
  const [native] = React.useState(hasNativePreview);
  const stageRect = useNativePreviewHole(native, frame, stage);
  const [box, setBox] = React.useState({ w: 0, h: 0 });
  const [drag, setDrag] = React.useState<{ x: number; y: number } | null>(null);

  // Fit the output aspect inside the available stage area.
  React.useLayoutEffect(() => {
    const el = outer.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      if (!entry) return;
      const { width, height } = entry.contentRect;
      const w = Math.min(width, height * aspect);
      setBox({ w: Math.floor(w), h: Math.floor(w / aspect) });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [aspect]);

  const showCam = settings.scene === "screen-cam";
  const camOnly = settings.scene === "cam";
  const bubbleW = Math.round(box.w * settings.pipSize);
  const square = settings.pipShape === "circle" || settings.pipShape === "square" || settings.pipShape === "squircle";
  const bubbleH = square ? bubbleW : Math.round((bubbleW * 9) / 16);

  const center = drag ?? centerOf(settings.pipPlace, bubbleW, bubbleH, box);
  const nearest = drag ? nearestCorner(drag, bubbleW, bubbleH, box) : null;

  function onPointerDown(e: React.PointerEvent) {
    e.currentTarget.setPointerCapture(e.pointerId);
    move(e);
  }
  function move(e: React.PointerEvent) {
    const rect = frame.current?.getBoundingClientRect();
    if (!rect) return;
    const x = clamp((e.clientX - rect.left) / rect.width, 0, 1);
    const y = clamp((e.clientY - rect.top) / rect.height, 0, 1);
    setDrag({ x, y });
  }
  function onPointerUp() {
    if (!drag) return;
    onPipPlace(nearest ? { corner: nearest } : { x: drag.x, y: drag.y });
    setDrag(null);
  }

  return (
    <div
      ref={stage}
      className={cn(
        "relative flex min-h-0 flex-1 flex-col rounded-[18px] p-4",
        native ? "overflow-hidden" : "bg-stage",
      )}
    >
      {stageRect && <PageAroundStage rect={stageRect} />}
      <div ref={outer} className="relative flex min-h-0 flex-1 items-center justify-center">
        <div
          ref={frame}
          className={cn(
            "relative overflow-hidden rounded-md ring-1 ring-foreground/10",
            recording && !paused && "ring-2 ring-rec",
            paused && "ring-2 ring-warn",
          )}
          style={{
            width: box.w,
            height: box.h,
            // The native video shows through this box; its shadow paints the stage around it.
            boxShadow: native ? "0 0 0 200vmax var(--color-stage)" : undefined,
          }}
        >
          {native ? null : camOnly ? (
            <CameraBubble
              shape="rect"
              roundness={0}
              width={box.w}
              border={0}
              shadow={false}
              mirror={settings.mirror}
              stream={cameraStream}
              className="[&>div]:!aspect-auto"
            />
          ) : (
            children
          )}

          {showCam && bubbleW > 0 && (
            <>
              {drag && <SnapGuides box={box} bubbleW={bubbleW} bubbleH={bubbleH} active={nearest} />}
              <div
                role="slider"
                aria-label="Camera overlay position. Drag to move; release near a corner to snap."
                aria-valuetext={settings.pipPlace.corner !== undefined ? settings.pipPlace.corner : "custom position"}
                tabIndex={0}
                onPointerDown={onPointerDown}
                onPointerMove={(e) => drag && move(e)}
                onPointerUp={onPointerUp}
                onKeyDown={(e) => {
                  const order: PipCorner[] = ["top-left", "top-right", "bottom-right", "bottom-left"];
                  const cur = settings.pipPlace.corner !== undefined ? order.indexOf(settings.pipPlace.corner) : 0;
                  if (e.key === "ArrowRight") onPipPlace({ corner: order[(cur + 1) % 4] ?? "bottom-right" });
                  if (e.key === "ArrowLeft") onPipPlace({ corner: order[(cur + 3) % 4] ?? "bottom-right" });
                }}
                className={cn(
                  "absolute cursor-grab touch-none outline-none focus-visible:ring-3 focus-visible:ring-ring/60",
                  drag ? "cursor-grabbing" : "transition-[left,top] duration-200 ease-spring",
                )}
                style={{ left: center.x * box.w - bubbleW / 2, top: center.y * box.h - bubbleH / 2 }}
              >
                {native ? (
                  // The engine draws the camera into the video; this is only the drag handle.
                  <div
                    className={cn(
                      "rounded-md border border-dashed transition-colors",
                      drag ? "border-primary bg-primary/10" : "border-transparent hover:border-foreground/40",
                    )}
                    style={{ width: bubbleW, height: bubbleH }}
                  />
                ) : (
                  <CameraBubble
                    shape={settings.pipShape}
                    roundness={settings.pipRoundness}
                    width={bubbleW}
                    border={Math.round((settings.pipBorder * box.h) / 1080)}
                    borderColor={settings.pipBorderColor}
                    shadow={settings.pipShadow}
                    mirror={settings.mirror}
                    stream={cameraStream}
                  />
                )}
              </div>
            </>
          )}

          {recording && (
            <div className="pointer-events-none absolute top-2 left-2 flex items-center gap-1.5 rounded-full bg-stage/70 px-2 py-0.5 text-[10.5px] font-semibold tracking-wider text-rec-fg uppercase backdrop-blur">
              <span className={cn("size-1.5 rounded-full", paused ? "bg-warn" : "animate-rec-pulse bg-rec")} />
              {paused ? "Paused" : "Rec"}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

/** Paints the page background everywhere except the stage, behind all other content. */
function PageAroundStage({ rect }: { rect: { x: number; y: number; w: number; h: number } }) {
  return (
    <div aria-hidden className="pointer-events-none fixed inset-0 -z-10 overflow-hidden">
      <div
        className="absolute rounded-[18px]"
        style={{
          left: rect.x,
          top: rect.y,
          width: rect.w,
          height: rect.h,
          boxShadow: "0 0 0 200vmax var(--color-background)",
        }}
      />
    </div>
  );
}

function SnapGuides({
  box,
  bubbleW,
  bubbleH,
  active,
}: {
  box: { w: number; h: number };
  bubbleW: number;
  bubbleH: number;
  active: PipCorner | null;
}) {
  const corners: PipCorner[] = ["top-left", "top-right", "bottom-left", "bottom-right"];
  return (
    <>
      {corners.map((c) => {
        const p = centerOf({ corner: c }, bubbleW, bubbleH, box);
        return (
          <div
            key={c}
            className={cn(
              "pointer-events-none absolute rounded-md border border-dashed transition-colors",
              active === c ? "border-primary bg-primary/15" : "border-foreground/20",
            )}
            style={{ left: p.x * box.w - bubbleW / 2, top: p.y * box.h - bubbleH / 2, width: bubbleW, height: bubbleH }}
          />
        );
      })}
    </>
  );
}

function centerOf(place: PipPlace, bw: number, bh: number, box: { w: number; h: number }) {
  if (place.corner === undefined) return { x: place.x, y: place.y };
  if (box.w === 0) return { x: 0.8, y: 0.8 };
  const m = MARGIN * box.w;
  const left = place.corner.endsWith("left");
  const top = place.corner.startsWith("top");
  return {
    x: (left ? m + bw / 2 : box.w - m - bw / 2) / box.w,
    y: (top ? m + bh / 2 : box.h - m - bh / 2) / box.h,
  };
}

function nearestCorner(p: { x: number; y: number }, bw: number, bh: number, box: { w: number; h: number }) {
  const corners: PipCorner[] = ["top-left", "top-right", "bottom-left", "bottom-right"];
  let best: PipCorner | null = null;
  let bestD = SNAP;
  for (const c of corners) {
    const q = centerOf({ corner: c }, bw, bh, box);
    const d = Math.hypot(q.x - p.x, (q.y - p.y) * (box.h / Math.max(1, box.w)));
    if (d < bestD) {
      bestD = d;
      best = c;
    }
  }
  return best;
}

function clamp(v: number, lo: number, hi: number) {
  return Math.max(lo, Math.min(hi, v));
}
