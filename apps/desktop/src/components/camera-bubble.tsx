import * as React from "react";
import { UserRound } from "lucide-react";

import { cn } from "@starter/ui/lib/utils";

import type { PipShape } from "../lib/types";

/**
 * SOURCE OF TRUTH KEYWORDS: CameraBubble, PiP, webcam overlay, pip shape, squircle, superellipse
 * WHAT:  Draws the webcam overlay in any of Rec's six shapes, with border, shadow and mirror.
 * WHY:   The same overlay appears in the preview, the camera settings and the HUD tile, and
 *        must look exactly like what gets burned into the recording.
 * WHERE: PreviewStage, camera settings, HUD camera tile.
 */
export function CameraBubble({
  shape,
  roundness,
  width,
  border,
  borderColor,
  shadow,
  mirror,
  stream,
  className,
}: {
  shape: PipShape;
  roundness: number;
  /** Rendered width in px. */
  width: number;
  border: number;
  /** Border colour as RRGGBB hex; defaults to the card colour. */
  borderColor?: string;
  shadow: boolean;
  mirror: boolean;
  /** Optional live camera stream (prototype "use my camera"). */
  stream?: MediaStream | null;
  className?: string;
}) {
  const square = shape === "circle" || shape === "square" || shape === "squircle";
  const height = square ? width : Math.round((width * 9) / 16);
  const clip = clipFor(shape, roundness, width, height);

  return (
    <div
      className={cn("relative", shadow && "drop-shadow-[0_10px_18px_rgb(0_0_0/0.35)]", className)}
      style={{ width, height }}
    >
      {border > 0 && (
        <div
          className="absolute inset-0 bg-card"
          style={{ ...clip, ...(borderColor ? { background: `#${borderColor}` } : {}) }}
        />
      )}
      <div
        className="absolute overflow-hidden"
        style={{ inset: border, ...clipFor(shape, roundness, width - border * 2, height - border * 2) }}
      >
        <CameraFeed stream={stream} mirror={mirror} />
      </div>
    </div>
  );
}

function CameraFeed({ stream, mirror }: { stream?: MediaStream | null; mirror: boolean }) {
  const ref = React.useRef<HTMLVideoElement>(null);
  React.useEffect(() => {
    if (ref.current) ref.current.srcObject = stream ?? null;
  }, [stream]);

  if (stream) {
    return (
      <video ref={ref} autoPlay muted playsInline className={cn("size-full object-cover", mirror && "-scale-x-100")} />
    );
  }
  // Placeholder subject: soft studio backdrop with a silhouette.
  return (
    <div className="flex size-full items-end justify-center bg-gradient-to-br from-accent-tint via-muted to-secondary">
      <UserRound className="size-[70%] translate-y-[12%] text-muted-foreground/60" strokeWidth={1.2} />
    </div>
  );
}

function clipFor(shape: PipShape, roundness: number, w: number, h: number): React.CSSProperties {
  const short = Math.min(w, h);
  switch (shape) {
    case "circle":
    case "pill":
      return { borderRadius: 9999 };
    case "rect":
      return { borderRadius: 0 };
    case "rounded":
    case "square":
      return { borderRadius: Math.round((roundness * short) / 2) };
    case "squircle":
      return { clipPath: superellipse(roundness) };
  }
}

// Superellipse |x|^n + |y|^n = 1 as a percentage polygon. Higher roundness → lower n (rounder).
function superellipse(roundness: number) {
  const n = 2 + (1 - Math.max(0, Math.min(1, roundness))) * 6;
  const pts: string[] = [];
  const steps = 72;
  for (let i = 0; i < steps; i++) {
    const a = (i / steps) * Math.PI * 2;
    const c = Math.cos(a);
    const s = Math.sin(a);
    const x = Math.sign(c) * Math.pow(Math.abs(c), 2 / n);
    const y = Math.sign(s) * Math.pow(Math.abs(s), 2 / n);
    pts.push(`${(50 + x * 50).toFixed(2)}% ${(50 + y * 50).toFixed(2)}%`);
  }
  return `polygon(${pts.join(",")})`;
}
