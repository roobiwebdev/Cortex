import type * as React from "react";
import {
  CameraOff,
  Check,
  Clapperboard,
  Folder,
  HelpCircle,
  Lock,
  Mic,
  MonitorUp,
  MousePointer2,
  Radio,
  Scissors,
  Sparkle,
  TriangleAlert,
  Video,
  Wifi,
} from "lucide-react";

import { Kbd } from "@starter/ui/components/kbd";
import { LevelMeter, type LevelSource } from "@starter/ui/components/level-meter";
import { cn } from "@starter/ui/lib/utils";

import { CameraBubble } from "../../components/camera-bubble";
import { micLevels, silentLevels } from "../../lib/engine";
import type { EngineState, Permission, PipPlace, PlatformOs } from "../../lib/types";
import { MockScreen } from "../../prototype/mock-screen";
import type { ChipState } from "./permission-chip";
import { PLATFORM_FACTS, type TourStepId } from "./steps";

/** What the capture step's mock shows (the tour's own choice; it never changes the target). */
export type CaptureMode = "display" | "window" | "area" | "app";

export interface StageProps {
  step: TourStepId;
  platform: PlatformOs;
  engine: EngineState;
  /** Chip state per permission, including "waiting" while a prompt is open. */
  chips: Record<Permission, ChipState>;
  cameraStream: MediaStream | null;
  captureMode: CaptureMode;
  /** The shortcut step caught a real press of the record shortcut. */
  hotkeyPressed: boolean;
}

/**
 * SOURCE OF TRUTH KEYWORDS: OnboardingStage, tour stage, animated mock, teach by doing, onboarding left pane, sparkles
 * WHAT:  The left half of the tour: an accent-washed, dotted stage with a small animated mock
 *        of the current step (capture modes, camera bubble, shortcut, files, trim, replay).
 * WHY:   Each feature is easier to understand when you see it move. Mocks are built from the
 *        app's own components (MockScreen, CameraBubble, LevelMeter, Kbd) or simple shapes in
 *        the same tokens, and react to the real settings and grants the panel changes.
 * WHERE: OnboardingWindow only.
 */
export function OnboardingStage(props: StageProps) {
  const { step } = props;
  return (
    <div className="tour-stage relative size-full overflow-hidden">
      <div className="tour-dots pointer-events-none absolute inset-0" />
      <Sparkles />
      <div key={step} className="tour-scene relative flex size-full items-center justify-center px-8 pt-11 pb-6">
        {step === "welcome" && <WelcomeScene {...props} />}
        {step === "permissions" && <PermissionsScene {...props} />}
        {step === "capture" && <CaptureScene {...props} />}
        {step === "camera" && <CameraScene {...props} />}
        {step === "shortcut" && <ShortcutScene {...props} />}
        {step === "files" && <FilesScene {...props} />}
        {step === "trim" && <TrimScene />}
        {step === "replay-live" && <ReplayLiveScene {...props} />}
        {step === "done" && <DoneScene {...props} />}
      </div>
    </div>
  );
}

const SPARKLES: ReadonlyArray<{ className: string; delay: string }> = [
  { className: "top-[14%] left-[12%] size-3.5", delay: "0s" },
  { className: "top-[22%] right-[14%] size-2.5", delay: "0.9s" },
  { className: "bottom-[16%] left-[20%] size-2.5", delay: "1.6s" },
  { className: "right-[10%] bottom-[26%] size-3", delay: "2.2s" },
];

function Sparkles() {
  return (
    <>
      {SPARKLES.map((s) => (
        <Sparkle
          key={s.className}
          aria-hidden
          className={cn("tour-sparkle pointer-events-none absolute fill-current text-accent-text", s.className)}
          style={{ animationDelay: s.delay }}
        />
      ))}
    </>
  );
}

/** Mock pieces enter one after another; `delay` in ms. */
function Rise({ delay = 0, className, children }: { delay?: number; className?: string; children: React.ReactNode }) {
  return (
    <div className={cn("tour-rise", className)} style={{ animationDelay: `${delay}ms` }}>
      {children}
    </div>
  );
}

const SCREEN_W = 320;
const SCREEN_H = 200;

/** A small framed display with optional overlays on top. */
function MiniScreen({ children, className }: { children?: React.ReactNode; className?: string }) {
  return (
    <div
      className={cn("relative overflow-hidden rounded-[10px] bg-card shadow-float", className)}
      style={{ width: SCREEN_W, height: SCREEN_H }}
    >
      <MockScreen />
      {children}
    </div>
  );
}

function RecPill({ label, className }: { label: string; className?: string }) {
  return (
    <div
      className={cn(
        "absolute top-3 left-3 flex h-6 items-center gap-1.5 rounded-full bg-card/90 px-2 text-[11px] font-semibold shadow-ledge",
        className,
      )}
    >
      <span className="size-2 rounded-full bg-rec animate-rec-pulse" aria-hidden />
      <span className="tabular">{label}</span>
    </div>
  );
}

function WelcomeScene({ engine, cameraStream }: StageProps) {
  const s = engine.settings;
  return (
    <div className="relative">
      <Rise>
        <MiniScreen>
          <RecPill label="0:12" />
          <CameraBubble
            className="absolute right-3 bottom-3"
            shape={s.pipShape}
            roundness={s.pipRoundness}
            width={60}
            border={0}
            shadow
            mirror={s.mirror}
            stream={cameraStream}
          />
        </MiniScreen>
      </Rise>
      {/* The floating control a recording gets. */}
      <Rise delay={180} className="absolute -bottom-7 left-1/2 -translate-x-1/2">
        <div className="flex h-10 items-center gap-3 rounded-[18px] bg-card px-3 shadow-float">
          <span className="grid size-6 place-items-center rounded-full bg-rec-tint">
            <span className="size-2.5 rounded-[3px] bg-rec" />
          </span>
          <span className="text-[13px] font-semibold tabular">0:12</span>
          <LevelMeter className="w-16" source={micLevels} segments={12} />
        </div>
      </Rise>
    </div>
  );
}

function PermissionsScene(props: StageProps) {
  const model = PLATFORM_FACTS[props.platform].permissionModel;
  if (model === "portal") return <PortalScene />;
  if (model === "privacy") return <PrivacyScene chips={props.chips} />;
  return <TccScene {...props} />;
}

/** macOS: the screen unlocks, the bubble and the mic come alive as each grant arrives. */
function TccScene({ engine, chips, cameraStream }: StageProps) {
  const screen = chips.screen === "granted";
  const camera = chips.camera === "granted";
  const mic = chips.microphone === "granted";
  const s = engine.settings;
  return (
    <div className="flex flex-col items-center gap-3">
      <Rise>
        <MiniScreen className={cn("transition-shadow duration-300", screen && "ring-2 ring-primary")}>
          {screen ? (
            <RecPill label="0:04" />
          ) : (
            <div className="absolute inset-0 grid place-items-center bg-background/75">
              <div className="flex flex-col items-center gap-2 text-center">
                <span
                  className={cn(
                    "grid size-9 place-items-center rounded-full bg-card shadow-ledge",
                    chips.screen === "denied" ? "text-warn" : "text-muted-foreground",
                  )}
                >
                  {chips.screen === "denied" ? <TriangleAlert className="size-4" /> : <Lock className="size-4" />}
                </span>
                <span className="max-w-[220px] text-[12px] font-medium text-muted-foreground">
                  {chips.screen === "waiting" ? "Waiting for your answer…" : "Cortex can’t see your screen yet"}
                </span>
              </div>
            </div>
          )}
          <div className="absolute right-3 bottom-3">
            <CameraBubble
              className={cn("transition-[opacity,filter] duration-300", !camera && "opacity-50 grayscale")}
              shape={s.pipShape}
              roundness={s.pipRoundness}
              width={56}
              border={0}
              shadow={camera}
              mirror={s.mirror}
              stream={camera ? cameraStream : null}
            />
            {!camera && (
              <span className="absolute inset-0 grid place-items-center text-muted-foreground">
                <CameraOff className="size-4" />
              </span>
            )}
          </div>
        </MiniScreen>
      </Rise>
      <Rise delay={160}>
        <SourceRow
          icon={<Mic className="size-3.5" />}
          label="Microphone"
          levels={mic ? micLevels : silentLevels}
          live={mic}
        />
      </Rise>
    </div>
  );
}

/** Windows: the two privacy switches, mirroring the real grants. */
function PrivacyScene({ chips }: { chips: Record<Permission, ChipState> }) {
  const rows: Array<{ p: Permission; label: string; icon: React.ReactNode }> = [
    { p: "camera", label: "Camera", icon: <Video className="size-3.5" /> },
    { p: "microphone", label: "Microphone", icon: <Mic className="size-3.5" /> },
  ];
  return (
    <Rise className="w-[300px] rounded-[14px] bg-card p-4 shadow-float">
      <div className="text-[10.5px] font-semibold tracking-[0.08em] text-muted-foreground uppercase">
        Privacy &amp; security
      </div>
      <div className="mt-3 divide-y rounded-[10px] border">
        {rows.map((r) => {
          const on = chips[r.p] === "granted";
          return (
            <div key={r.p} className="flex items-center gap-3 px-3 py-2.5">
              <span className="grid size-7 place-items-center rounded-[8px] bg-accent-tint text-accent-text">
                {r.icon}
              </span>
              <div className="min-w-0 flex-1">
                <div className="text-[12.5px] font-medium">{r.label}</div>
                <div className="text-[11px] text-muted-foreground">Let desktop apps access</div>
              </div>
              <FakeSwitch on={on} />
            </div>
          );
        })}
      </div>
      <div className="mt-3 flex items-center gap-2 text-[11.5px] text-muted-foreground">
        <MonitorUp className="size-3.5" /> Screen capture needs no switch
      </div>
    </Rise>
  );
}

/** Linux: the desktop portal's "choose what to share" dialog. */
function PortalScene() {
  return (
    <Rise className="w-[300px] rounded-[16px] bg-card p-4 shadow-float">
      <div className="text-center text-[13px] font-semibold">Choose what to share</div>
      <div className="mt-0.5 text-center text-[11.5px] text-muted-foreground">Cortex wants to record your screen</div>
      <div className="mt-3 grid grid-cols-2 gap-2">
        {["Entire screen", "A window"].map((label, i) => (
          <div
            key={label}
            className={cn(
              "overflow-hidden rounded-[10px] border p-1.5 text-center text-[11px] font-medium",
              i === 0 && "border-primary ring-2 ring-primary/40",
            )}
          >
            <div className="mb-1.5 aspect-[16/10] overflow-hidden rounded-[6px]">
              <MockScreen variant={i === 0 ? "desktop" : "window"} />
            </div>
            {label}
          </div>
        ))}
      </div>
      <div className="mt-3 flex justify-end gap-2">
        <span className="rounded-[8px] bg-muted px-3 py-1 text-[11.5px] font-medium">Cancel</span>
        <span className="rounded-[8px] bg-primary px-3 py-1 text-[11.5px] font-semibold text-primary-foreground shadow-ledge-accent">
          Share
        </span>
      </div>
    </Rise>
  );
}

function FakeSwitch({ on }: { on: boolean }) {
  return (
    <span
      aria-hidden
      className={cn(
        "relative h-[18px] w-8 shrink-0 rounded-full transition-colors duration-300",
        on ? "bg-primary" : "bg-foreground/15",
      )}
    >
      <span
        className={cn(
          "absolute top-px left-px size-4 rounded-full shadow-ledge transition-transform duration-300 ease-spring",
          on ? "translate-x-[14px] bg-primary-foreground" : "bg-background",
        )}
      />
    </span>
  );
}

/** Selection rectangles over MockScreen's desktop, in % of the mini screen. */
const SELECTION: Record<CaptureMode, { top: number; left: number; w: number; h: number }> = {
  display: { top: 0, left: 0, w: 100, h: 100 },
  window: { top: 11, left: 9, w: 62, h: 74 },
  area: { top: 26, left: 22, w: 42, h: 40 },
  app: { top: 11, left: 9, w: 62, h: 74 },
};

const CAPTURE_LABEL: Record<CaptureMode, string> = {
  display: "Whole display",
  window: "One window",
  area: "1280 × 720",
  app: "Every window of one app",
};

function CaptureScene({ engine, captureMode }: StageProps) {
  const r = SELECTION[captureMode];
  const display = engine.displays[0];
  const dim = captureMode === "window" || captureMode === "area";
  return (
    <div className="flex flex-col items-center gap-3">
      <Rise>
        <MiniScreen>
          {/* One selection that morphs between modes. */}
          <div
            className={cn(
              "absolute rounded-[4px] transition-all duration-500 ease-out-quick",
              captureMode === "area" ? "border-2 border-dashed border-primary" : "ring-2 ring-primary",
              captureMode === "display" && "rounded-[10px] ring-inset",
              dim && "tour-dim-outside",
            )}
            style={{ top: `${r.top}%`, left: `${r.left}%`, width: `${r.w}%`, height: `${r.h}%` }}
          >
            {captureMode === "area" && (
              <>
                {["-top-1 -left-1", "-top-1 -right-1", "-bottom-1 -left-1", "-right-1 -bottom-1"].map((pos) => (
                  <span key={pos} className={cn("absolute size-2 rounded-[2px] border border-primary bg-card", pos)} />
                ))}
                <MousePointer2 className="absolute -right-4 -bottom-5 size-4 fill-card text-foreground drop-shadow" />
              </>
            )}
          </div>
          {/* App mode also takes the app's other window. */}
          <div
            className={cn(
              "absolute top-[22%] right-[6%] h-[34%] w-[20%] rounded-[4px] ring-2 ring-primary transition-opacity duration-300",
              captureMode === "app" ? "opacity-100" : "opacity-0",
            )}
          />
        </MiniScreen>
      </Rise>
      <Rise delay={140}>
        <div className="flex h-7 items-center gap-2 rounded-full bg-card px-3 text-[12px] font-medium shadow-ledge">
          <span className="size-1.5 rounded-full bg-primary" />
          <span className="tabular">{CAPTURE_LABEL[captureMode]}</span>
          {captureMode === "display" && display && (
            <span className="text-muted-foreground tabular">
              · {display.width} × {display.height}
            </span>
          )}
          {captureMode === "area" && <span className="text-muted-foreground">· drag, or click a window</span>}
        </div>
      </Rise>
    </div>
  );
}

const BUBBLE_W = 64;
const INSET = 12;

/** Where the bubble sits on the mini screen for a placement. */
function bubbleSpot(place: PipPlace, h: number): { left: number; top: number } {
  if (place.corner !== undefined) {
    const right = place.corner.endsWith("right");
    const bottom = place.corner.startsWith("bottom");
    return {
      left: right ? SCREEN_W - INSET - BUBBLE_W : INSET,
      top: bottom ? SCREEN_H - INSET - h : INSET,
    };
  }
  const clamp = (v: number, max: number) => Math.max(INSET, Math.min(max, v));
  return {
    left: clamp(place.x * SCREEN_W - BUBBLE_W / 2, SCREEN_W - INSET - BUBBLE_W),
    top: clamp(place.y * SCREEN_H - h / 2, SCREEN_H - INSET - h),
  };
}

function CameraScene({ engine, cameraStream }: StageProps) {
  const s = engine.settings;
  const square = s.pipShape === "circle" || s.pipShape === "square" || s.pipShape === "squircle";
  const h = square ? BUBBLE_W : Math.round((BUBBLE_W * 9) / 16);
  const spot = bubbleSpot(s.pipPlace, h);
  return (
    <div className="flex flex-col items-center gap-3">
      <Rise>
        <MiniScreen>
          <div
            className="absolute transition-[left,top] duration-500 ease-spring"
            style={{ left: spot.left, top: spot.top }}
          >
            <CameraBubble
              shape={s.pipShape}
              roundness={s.pipRoundness}
              width={BUBBLE_W}
              border={0}
              shadow
              mirror={s.mirror}
              stream={cameraStream}
            />
            <MousePointer2 className="absolute -right-2 -bottom-3 size-4 fill-card text-foreground drop-shadow" />
          </div>
        </MiniScreen>
      </Rise>
      <Rise delay={160} className="w-[320px] space-y-1.5 rounded-[12px] bg-card p-2.5 shadow-ledge">
        <TrackLane icon={<MonitorUp className="size-3" />} label="Screen + camera" />
        <div
          className={cn(
            "transition-[opacity,max-height] duration-300 ease-out-quick",
            s.cameraTrack ? "max-h-6 opacity-100" : "max-h-0 opacity-0",
          )}
        >
          <TrackLane icon={<Video className="size-3" />} label="Camera track" tone="accent" />
        </div>
      </Rise>
    </div>
  );
}

function TrackLane({
  icon,
  label,
  tone = "muted",
}: {
  icon: React.ReactNode;
  label: string;
  tone?: "muted" | "accent";
}) {
  return (
    <div className="flex items-center gap-2 text-[11px]">
      <span className="flex w-[124px] shrink-0 items-center gap-1.5 truncate text-muted-foreground">
        {icon}
        {label}
      </span>
      <span className="h-3 flex-1 overflow-hidden rounded-[4px] bg-muted">
        <span
          className={cn(
            "tour-fill block h-full rounded-[4px]",
            tone === "accent" ? "bg-primary/70" : "bg-foreground/25",
          )}
        />
      </span>
    </div>
  );
}

function ShortcutScene({ platform, hotkeyPressed }: StageProps) {
  const keys = PLATFORM_FACTS[platform].recordKeys;
  return (
    <div className="flex flex-col items-center gap-6">
      <div className="flex items-center gap-2" aria-hidden>
        {keys.map((k, i) => (
          <Rise key={`${k}-${hotkeyPressed ? "p" : "i"}`} delay={i * 90}>
            <Kbd
              className={cn(
                "tour-key-press h-14 min-w-14 rounded-[12px] px-3 text-[20px] font-semibold text-foreground",
                hotkeyPressed && "border-primary text-accent-text",
              )}
              style={{ animationDelay: `${350 + i * 110}ms` }}
            >
              {k}
            </Kbd>
          </Rise>
        ))}
      </div>
      {hotkeyPressed ? (
        <div className="tour-pop flex h-10 items-center gap-2 rounded-full bg-ok-tint px-4 text-[13px] font-semibold text-ok shadow-ledge">
          <Check className="size-4" strokeWidth={2.5} /> That’s the one
        </div>
      ) : (
        <Rise delay={700}>
          <div className="flex h-10 items-center gap-3 rounded-[18px] bg-card px-3 shadow-float">
            <span className="size-2 rounded-full bg-rec animate-rec-pulse" />
            <span className="text-[13px] font-semibold tabular">0:00</span>
            <span className="text-[12px] text-muted-foreground">Recording from any app</span>
          </div>
        </Rise>
      )}
    </div>
  );
}

const SAMPLE_FILES = ["Product walkthrough", "Bug repro, checkout", "Weekly update"];

function FilesScene({ engine }: StageProps) {
  const ext = engine.settings.container;
  const folder = engine.settings.outputDir.split(/[\\/]/).filter(Boolean).pop() ?? "Cortex";
  const tracks: Array<{ label: string; icon: React.ReactNode }> = [
    { label: "Mix", icon: <Radio className="size-3" /> },
    { label: "System audio", icon: <MonitorUp className="size-3" /> },
    { label: "Microphone", icon: <Mic className="size-3" /> },
  ];
  return (
    <div className="flex w-[320px] flex-col gap-3">
      <Rise className="overflow-hidden rounded-[12px] bg-card shadow-float">
        <div className="flex items-center gap-2 border-b px-3 py-2 text-[12px] font-semibold">
          <Folder className="size-3.5 text-accent-text" />
          {folder}
        </div>
        <div className="divide-y">
          <div className="flex items-center gap-2 px-3 py-2 text-[12px]">
            <Clapperboard className="size-3.5 text-rec" />
            <span className="flex-1 truncate font-medium">Recording.{ext}</span>
            <span className="flex items-center gap-1 text-[11px] text-muted-foreground">
              <span className="size-1.5 rounded-full bg-rec animate-rec-pulse" /> saved every 2 s
            </span>
          </div>
          {SAMPLE_FILES.map((name, i) => (
            <Rise key={name} delay={120 + i * 80} className="flex items-center gap-2 px-3 py-2 text-[12px]">
              <Clapperboard className="size-3.5 text-muted-foreground" />
              <span className="flex-1 truncate">
                {name}.{ext}
              </span>
            </Rise>
          ))}
        </div>
      </Rise>
      <Rise delay={320} className="space-y-1.5 rounded-[12px] bg-card p-2.5 shadow-ledge">
        <div className="text-[10px] font-semibold tracking-[0.08em] text-muted-foreground uppercase">Audio tracks</div>
        {tracks.map((t) => (
          <TrackLane key={t.label} icon={t.icon} label={t.label} tone="accent" />
        ))}
      </Rise>
    </div>
  );
}

/** Deterministic waveform heights (% of the lane). */
const WAVE = [30, 52, 40, 72, 58, 34, 66, 80, 44, 28, 62, 74, 50, 36, 58, 70, 42, 30, 54, 68, 46, 32, 60, 40];

function TrimScene() {
  return (
    <div className="flex w-[320px] flex-col gap-3">
      <Rise className="overflow-hidden rounded-[12px] bg-card shadow-float">
        <div className="aspect-[16/8] overflow-hidden">
          <MockScreen />
        </div>
        <div className="space-y-1.5 p-2.5">
          <div className="relative h-14 overflow-hidden rounded-[8px] bg-muted">
            <div className="grid h-8 grid-cols-8 gap-px">
              {Array.from({ length: 8 }, (_, i) => (
                <div key={i} className="bg-gradient-to-br from-accent-solid/45 via-accent-tint to-secondary" />
              ))}
            </div>
            <div className="flex h-6 items-center justify-between px-1.5">
              {WAVE.map((h, i) => (
                <span key={i} className="w-[3px] rounded-full bg-foreground/35" style={{ height: `${h}%` }} />
              ))}
            </div>
            {/* The cut parts shade in as the handles move. */}
            <div
              className="tour-trim-left absolute inset-y-0 left-0 w-[14%] bg-background/70"
              style={{ animationDelay: "350ms" }}
            />
            <div
              className="tour-trim-right absolute inset-y-0 right-0 w-[22%] bg-background/70"
              style={{ animationDelay: "350ms" }}
            />
            <div className="pointer-events-none absolute inset-y-0 right-[22%] left-[14%] rounded-[6px] border-2 border-primary transition-all" />
          </div>
          <div className="flex items-center justify-between text-[11px] text-muted-foreground">
            <span className="flex items-center gap-1">
              <Kbd>I</Kbd> in <Kbd>O</Kbd> out
            </span>
            <span className="tabular">0:04 – 0:31</span>
          </div>
        </div>
      </Rise>
      <Rise delay={260} className="flex justify-center">
        <div className="flex h-7 items-center gap-2 rounded-full bg-card px-3 text-[12px] font-medium shadow-ledge">
          <Scissors className="size-3.5 text-accent-text" />
          Fast · no re-encode
        </div>
      </Rise>
    </div>
  );
}

function ReplayLiveScene({ engine }: StageProps) {
  const s = engine.settings;
  const on = s.replayEnabled;
  const dests = s.streamDests.filter((d) => d.enabled).map((d) => d.service);
  const names: Record<string, string> = { youtube: "YouTube", twitch: "Twitch", kick: "Kick", custom: "Custom" };
  return (
    <div className="flex w-[320px] flex-col gap-3">
      <Rise
        className={cn("rounded-[12px] bg-card p-3 shadow-float transition-opacity duration-300", !on && "opacity-70")}
      >
        <div className="flex items-center justify-between text-[12px] font-semibold">
          <span>Replay buffer</span>
          <span className={cn("text-[11px] font-medium tabular", on ? "text-accent-text" : "text-muted-foreground")}>
            {on ? `Last ${s.replaySecs} s` : "Off"}
          </span>
        </div>
        <div className="mt-2 h-3 overflow-hidden rounded-full bg-muted">
          {on && <div className="tour-fill h-full rounded-full bg-primary/70" />}
        </div>
        {on && (
          <div
            className="tour-pop mt-2 flex items-center gap-2 rounded-[8px] bg-ok-tint px-2 py-1.5 text-[11.5px] font-medium text-ok"
            style={{ animationDelay: "700ms" }}
          >
            <Check className="size-3.5" /> Replay saved
          </div>
        )}
      </Rise>
      <Rise delay={200} className="rounded-[12px] bg-card p-3 shadow-float">
        <div className="flex items-center gap-2">
          <span className="flex h-5 items-center gap-1 rounded-full bg-rec px-2 text-[10.5px] font-bold tracking-wide text-rec-fg">
            <span className="size-1.5 rounded-full bg-rec-fg animate-rec-pulse" /> LIVE
          </span>
          <span className="text-[12px] font-semibold tabular">12:48</span>
          <span className="ml-auto text-[11px] text-muted-foreground tabular">{s.streamBitrateKbps} kb/s</span>
        </div>
        <div className="mt-2.5 flex flex-wrap gap-1.5">
          {(dests.length > 0 ? dests : ["youtube"]).map((d) => (
            <span key={d} className="rounded-full bg-muted px-2 py-0.5 text-[11px] font-medium">
              {names[d] ?? d}
            </span>
          ))}
        </div>
      </Rise>
    </div>
  );
}

/** Where Cortex lives: the menu bar on macOS, the taskbar tray elsewhere. */
function DoneScene({ platform }: StageProps) {
  const mac = platform === "macos";
  return (
    <Rise>
      <MiniScreen>
        <div
          className={cn(
            "absolute inset-x-0 flex h-[18px] items-center justify-end gap-2.5 bg-card/90 px-2 text-[9px] font-medium",
            mac ? "top-0" : "bottom-0",
          )}
        >
          <span
            className="tour-pop grid size-[14px] place-items-center rounded-[4px] bg-primary"
            style={{ animationDelay: "250ms" }}
          >
            <span className="size-1.5 rounded-full bg-primary-foreground" />
          </span>
          <Wifi className="size-2.5" />
          <span className="tabular">9:41</span>
        </div>
        <div
          className={cn(
            "tour-rise absolute right-[14px] w-[150px] space-y-1.5 rounded-[10px] bg-card p-2 shadow-float",
            mac ? "top-[22px]" : "bottom-[22px]",
          )}
          style={{ animationDelay: "420ms" }}
        >
          <div className="text-[9.5px] font-semibold">Cortex</div>
          <div className="flex h-6 items-center justify-center gap-1 rounded-[7px] bg-rec text-[9.5px] font-semibold text-rec-fg shadow-ledge-rec">
            <span className="size-1.5 rounded-full bg-rec-fg" /> Start recording
          </div>
          <div className="flex items-center gap-1 text-[9px] text-muted-foreground">
            <HelpCircle className="size-2.5" /> Help › Show the Tour
          </div>
        </div>
      </MiniScreen>
    </Rise>
  );
}

function SourceRow({
  icon,
  label,
  levels,
  live,
}: {
  icon: React.ReactNode;
  label: string;
  levels: LevelSource;
  live: boolean;
}) {
  return (
    <div className="flex w-[320px] items-center gap-3 rounded-[14px] bg-card px-3 py-2.5 shadow-ledge">
      <span
        className={cn(
          "grid size-7 place-items-center rounded-[8px] transition-colors",
          live ? "bg-accent-tint text-accent-text" : "bg-muted text-muted-foreground",
        )}
      >
        {icon}
      </span>
      <span className="w-24 text-[12.5px] font-medium">{label}</span>
      <LevelMeter className="flex-1" source={levels} muted={!live} segments={18} aria-label={`${label} level`} />
    </div>
  );
}
