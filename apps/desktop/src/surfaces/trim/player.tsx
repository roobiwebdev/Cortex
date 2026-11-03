import * as React from "react";
import { Play } from "lucide-react";

import { cn } from "@starter/ui/lib/utils";

import { CameraBubble } from "../../components/camera-bubble";
import type { PipPlace, RecorderSettings } from "../../lib/types";
import { MockScreen } from "../../prototype/mock-screen";

export type PlayerView = "composite" | "camera";

const MARGIN = 0.03;

/**
 * SOURCE OF TRUTH KEYWORDS: TrimPlayer, trim preview, video stage, burned-in camera, camera track view
 * WHAT:  The trim window's video area on the dark stage: the composite screen with the
 *        burned-in camera bubble, or the separate raw camera track on its own.
 * WHY:   Users need to see both what players show (composite, NEW-CT-02) and what the
 *        extra camera track holds (raw, unmirrored, NEW-CT-01) before deciding to keep it.
 *        In the real app `src` is the recording itself (camera already burned in), played by
 *        a <video> the parent drives through `mediaRef` (NEW-TR-02).
 * WHERE: TrimWindow.
 */
export function TrimPlayer({
  settings,
  view,
  playing,
  cameraStream,
  onToggle,
  aspect,
  src,
  mediaRef,
}: {
  settings: RecorderSettings;
  view: PlayerView;
  playing: boolean;
  cameraStream: MediaStream | null;
  onToggle: () => void;
  /** width / height of the recording */
  aspect: number;
  /** The recording's URL; without it the player shows the mock screen and bubble. */
  src?: string;
  mediaRef?: React.Ref<HTMLVideoElement>;
}) {
  const outer = React.useRef<HTMLDivElement>(null);
  const [box, setBox] = React.useState({ w: 0, h: 0 });

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

  const bubbleW = Math.round(box.w * settings.pipSize);
  const square = settings.pipShape === "circle" || settings.pipShape === "square" || settings.pipShape === "squircle";
  const bubbleH = square ? bubbleW : Math.round((bubbleW * 9) / 16);
  const pos = bubblePosition(settings.pipPlace, bubbleW, bubbleH, box);

  return (
    <div ref={outer} className="grid size-full place-items-center">
      <button
        type="button"
        onClick={onToggle}
        aria-label={playing ? "Pause" : "Play"}
        className="group relative overflow-hidden rounded-[6px] outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
        style={{ width: box.w, height: box.h }}
      >
        {view === "composite" && src !== undefined ? (
          <video ref={mediaRef} src={src} preload="auto" playsInline className="size-full bg-stage object-contain" />
        ) : view === "composite" ? (
          <>
            <MockScreen />
            {box.w > 0 && (
              <div className="absolute" style={{ left: pos.left, top: pos.top }}>
                <CameraBubble
                  shape={settings.pipShape}
                  roundness={settings.pipRoundness}
                  width={bubbleW}
                  border={settings.pipBorder}
                  shadow={settings.pipShadow}
                  mirror={settings.mirror}
                  stream={cameraStream}
                />
              </div>
            )}
          </>
        ) : (
          // The camera track is stored raw: full frame, unmirrored, no mask.
          <CameraBubble
            shape="rect"
            roundness={0}
            width={box.w}
            border={0}
            shadow={false}
            mirror={false}
            stream={cameraStream}
          />
        )}

        {view === "camera" && (
          <span className="absolute top-3 left-3 rounded-full bg-background/80 px-2.5 py-1 text-[11px] font-medium text-foreground backdrop-blur">
            Camera track · raw, unmirrored
          </span>
        )}

        {/* Paused: a quiet centre play affordance. */}
        <span
          className={cn(
            "absolute inset-0 grid place-items-center transition-opacity duration-200 ease-out-quick",
            playing ? "opacity-0" : "opacity-100",
          )}
          aria-hidden
        >
          <span className="grid size-14 place-items-center rounded-full bg-background/75 text-foreground shadow-float backdrop-blur transition-transform duration-150 group-hover:scale-105">
            <Play className="size-6 translate-x-px fill-current" />
          </span>
        </span>
      </button>
    </div>
  );
}

function bubblePosition(place: PipPlace, w: number, h: number, box: { w: number; h: number }) {
  const m = box.w * MARGIN;
  if (place.corner !== undefined) {
    const left = place.corner.endsWith("left") ? m : box.w - w - m;
    const top = place.corner.startsWith("top") ? m : box.h - h - m;
    return { left, top };
  }
  return { left: place.x * box.w - w / 2, top: place.y * box.h - h / 2 };
}
