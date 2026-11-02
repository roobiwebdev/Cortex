import * as React from "react";
import { Circle, Ellipsis, Eye, GripVertical, History, Mic, Pause, Play, Square, Trash2, X } from "lucide-react";

import { Button, buttonVariants } from "@starter/ui/components/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuTrigger,
} from "@starter/ui/components/dropdown-menu";
import { LevelMeter } from "@starter/ui/components/level-meter";
import { cn } from "@starter/ui/lib/utils";

import { CameraBubble } from "../components/camera-bubble";
import { formatClock } from "../lib/format";
import { engine, useEngine } from "../lib/mock-engine";
import type { SurfaceProps } from "../prototype/surfaces";
import { CaptureExcludedHint, FloatingCard, MENU_CONTENT, MENU_ITEM } from "./floating/floating-card";
import { HoldToConfirm } from "./floating/hold-to-confirm";
import { isCapturing, LiveBadge, RecStatus } from "./floating/status";

const HUD_W = 300;
const HUD_H = 56;
const EDGE = 16;
const CAM_W = 128;

/**
 * SOURCE OF TRUTH KEYWORDS: HudSurface, recording HUD, mini controller, floating capsule, stop pause, REC-05
 * WHAT:  The small floating capsule shown while recording: Stop, Pause/Resume, clock, mic meter
 *        and a menu (save replay, hide, hold-to-discard), with an optional docked camera tile.
 * WHY:   The main window is usually hidden while recording; the HUD gives one-click control
 *        from any Space. It is excluded from capture so it never appears in the video.
 * WHERE: Prototype surface "hud"; later a non-activating floating NSPanel (sharingType none).
 */
export function HudSurface({ cameraStream, navigate }: SurfaceProps) {
  const e = useEngine();
  const s = e.settings;
  const capturing = isCapturing(e.status);
  const paused = e.status === "paused";

  // Prototype-local: position, visibility and the discard confirm. The app keeps position per display.
  const area = React.useRef<HTMLDivElement>(null);
  const [offset, setOffset] = React.useState({ x: 0, y: 0 });
  const [hidden, setHidden] = React.useState(false);
  const [discardOpen, setConfirmDiscard] = React.useState(false);
  const drag = React.useRef<{ px: number; py: number; ox: number; oy: number } | null>(null);

  // The confirm only makes sense while a file is being written.
  const confirmDiscard = discardOpen && capturing;

  // Keep the stack inside the desktop with a 16px margin. Home is bottom centre, 48px up;
  // the reserve leaves room for the camera tile above and the caption below.
  const clamp = React.useCallback((x: number, y: number) => {
    const el = area.current;
    if (!el) return { x, y };
    const maxX = el.clientWidth / 2 - HUD_W / 2 - EDGE;
    const minY = -(el.clientHeight - 48 - HUD_H - CAM_W - 40 - EDGE);
    const maxY = 48 - 36 - EDGE;
    return { x: Math.max(-maxX, Math.min(maxX, x)), y: Math.max(minY, Math.min(maxY, y)) };
  }, []);

  function onGripDown(ev: React.PointerEvent<HTMLButtonElement>) {
    ev.currentTarget.setPointerCapture(ev.pointerId);
    drag.current = { px: ev.clientX, py: ev.clientY, ox: offset.x, oy: offset.y };
  }
  function onGripMove(ev: React.PointerEvent<HTMLButtonElement>) {
    const d = drag.current;
    if (!d) return;
    setOffset(clamp(d.ox + ev.clientX - d.px, d.oy + ev.clientY - d.py));
  }
  function onGripKey(ev: React.KeyboardEvent<HTMLButtonElement>) {
    const step = ev.shiftKey ? 64 : 16;
    const moves: Record<string, [number, number]> = {
      ArrowLeft: [-step, 0],
      ArrowRight: [step, 0],
      ArrowUp: [0, -step],
      ArrowDown: [0, step],
    };
    const m = moves[ev.key];
    if (!m) return;
    ev.preventDefault();
    setOffset((o) => clamp(o.x + m[0], o.y + m[1]));
  }

  const showCamera = s.scene !== "screen" && !confirmDiscard;

  return (
    <div ref={area} className="absolute inset-0 top-7">
      {hidden ? (
        <div className="absolute bottom-12 left-1/2 -translate-x-1/2">
          <CaptureExcludedHint>
            HUD hidden. Bring it back from the menu bar.
            <Button variant="ghost" size="xs" className="-my-1 -mr-2" onClick={() => setHidden(false)}>
              <Eye />
              Show
            </Button>
          </CaptureExcludedHint>
        </div>
      ) : (
        <div
          className="absolute bottom-12 left-1/2 flex flex-col items-center gap-2.5"
          style={{ transform: `translate(calc(-50% + ${offset.x}px), ${offset.y}px)` }}
        >
          {confirmDiscard && (
            <FloatingCard className="w-[300px] p-3 duration-150 ease-out-quick animate-in fade-in slide-in-from-bottom-1">
              <div className="flex items-start gap-2">
                <div className="min-w-0 flex-1">
                  <div className="text-[13px] font-semibold">Discard this recording?</div>
                  <div className="text-xs text-muted-foreground">
                    {formatClock(e.elapsedSecs)} of video will be deleted. This cannot be undone.
                  </div>
                </div>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label="Keep recording"
                  onClick={() => setConfirmDiscard(false)}
                >
                  <X />
                </Button>
              </div>
              <div className="mt-2.5 flex gap-2">
                <HoldToConfirm
                  className="flex-1"
                  aria-label="Hold to discard the recording"
                  onConfirm={() => {
                    setConfirmDiscard(false);
                    engine.discard();
                  }}
                >
                  <Trash2 className="mr-1.5 size-3.5" aria-hidden />
                  Hold to discard
                </HoldToConfirm>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-8 rounded-[10px]"
                  onClick={() => setConfirmDiscard(false)}
                >
                  Keep
                </Button>
              </div>
            </FloatingCard>
          )}

          {showCamera && (
            <CameraBubble
              shape={s.pipShape}
              roundness={s.pipRoundness}
              width={CAM_W}
              border={s.pipBorder}
              shadow={s.pipShadow}
              mirror={s.mirror}
              stream={cameraStream}
            />
          )}

          <FloatingCard
            role="toolbar"
            aria-label="Recording controls"
            className="flex items-center gap-1.5 rounded-full pr-2 pl-1"
            style={{ width: HUD_W, height: HUD_H }}
          >
            <button
              type="button"
              aria-label="Move HUD (drag, or use arrow keys)"
              title="Drag to move"
              onPointerDown={onGripDown}
              onPointerMove={onGripMove}
              onPointerUp={() => {
                drag.current = null;
              }}
              onKeyDown={onGripKey}
              className="grid h-9 w-4 shrink-0 cursor-grab touch-none place-items-center rounded-[6px] text-muted-foreground/60 outline-none hover:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring/50 active:cursor-grabbing"
            >
              <GripVertical className="size-3.5" />
            </button>

            {capturing || e.status === "finalizing" ? (
              <RoundButton
                label="Stop and save (⇧⌘R)"
                tone="rec"
                disabled={e.status === "finalizing"}
                onClick={() => engine.stop()}
              >
                <Square className="size-3.5 fill-current" />
              </RoundButton>
            ) : (
              <RoundButton
                label={e.status === "countdown" ? "Cancel countdown (Esc)" : "Start recording (⇧⌘R)"}
                tone="rec"
                onClick={() => engine.toggleRecord()}
              >
                {e.status === "countdown" ? <X className="size-4" /> : <Circle className="size-3.5 fill-current" />}
              </RoundButton>
            )}

            <RoundButton
              label={paused ? "Resume" : "Pause"}
              tone="soft"
              disabled={!capturing}
              onClick={() => engine.togglePause()}
            >
              {paused ? <Play className="size-4 fill-current" /> : <Pause className="size-4 fill-current" />}
            </RoundButton>

            <div className="ml-1 min-w-0 flex-1">
              <div className="flex items-center gap-1.5">
                <RecStatus status={e.status} />
                {e.live && <LiveBadge className="h-4 text-[9px]" />}
              </div>
              <div
                className={cn(
                  "text-[19px] leading-6 font-semibold tracking-tight tabular",
                  paused || e.status === "finalizing" || e.status === "idle"
                    ? "text-muted-foreground"
                    : "text-foreground",
                )}
              >
                {e.status === "countdown" ? `Starts in ${e.countdownLeft}` : formatClock(e.elapsedSecs)}
              </div>
            </div>

            <div className="flex w-12 shrink-0 items-center gap-1" title="Microphone level">
              <Mic
                className={cn("size-3 shrink-0", s.micEnabled ? "text-muted-foreground" : "text-muted-foreground/40")}
                aria-hidden
              />
              <LevelMeter
                aria-label="Microphone level"
                className="h-1.5 flex-1 gap-px"
                segments={8}
                level={e.levels.mic}
                peak={e.levels.micPeak}
                muted={!s.micEnabled || s.micMuted}
              />
            </div>

            <DropdownMenu>
              <DropdownMenuTrigger
                aria-label="More recording actions"
                className={cn(
                  buttonVariants({ variant: "ghost", size: "icon-sm" }),
                  "rounded-full text-muted-foreground",
                )}
              >
                <Ellipsis />
              </DropdownMenuTrigger>
              <DropdownMenuContent side="top" align="end" sideOffset={10} className={MENU_CONTENT}>
                <DropdownMenuItem className={MENU_ITEM} disabled={!s.replayEnabled} onClick={() => engine.saveReplay()}>
                  <History />
                  Save replay
                  <DropdownMenuShortcut>⇧⌘S</DropdownMenuShortcut>
                </DropdownMenuItem>
                <DropdownMenuItem className={MENU_ITEM} onClick={() => setHidden(true)}>
                  <Eye />
                  Hide HUD
                </DropdownMenuItem>
                <DropdownMenuItem className={MENU_ITEM} onClick={() => navigate("menubar")}>
                  Open menu bar panel
                </DropdownMenuItem>
                <DropdownMenuSeparator className="mx-1 my-1" />
                <DropdownMenuItem
                  className={MENU_ITEM}
                  variant="destructive"
                  disabled={!capturing}
                  onClick={() => setConfirmDiscard(true)}
                >
                  <Trash2 />
                  Discard recording…
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </FloatingCard>

          <CaptureExcludedHint>
            {e.status === "idle"
              ? "Ready. In the app this bar floats over every Space and is left out of the recording."
              : "Left out of the recording. Drag the grip to move it."}
          </CaptureExcludedHint>
        </div>
      )}
    </div>
  );
}

function RoundButton({
  label,
  tone,
  disabled,
  onClick,
  children,
}: {
  label: string;
  tone: "rec" | "soft";
  disabled?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "grid size-10 shrink-0 place-items-center rounded-full transition-[filter,transform,box-shadow] outline-none focus-visible:ring-3 focus-visible:ring-ring/50 active:translate-y-0.5 active:shadow-none disabled:pointer-events-none disabled:opacity-40",
        tone === "rec"
          ? "bg-rec text-rec-fg shadow-ledge-rec hover:brightness-105"
          : "bg-secondary text-secondary-foreground shadow-ledge hover:bg-secondary/80",
      )}
    >
      {children}
    </button>
  );
}
