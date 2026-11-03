import type * as React from "react";
import {
  AppWindow,
  Check,
  Download,
  ExternalLink,
  FolderOpen,
  HardDrive,
  Info,
  KeyRound,
  Layers,
  LayoutGrid,
  Mic,
  Monitor,
  Radio,
  RotateCcw,
  Scissors,
  ShieldCheck,
  SquareDashed,
  Video,
} from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Kbd } from "@starter/ui/components/kbd";
import { Segmented } from "@starter/ui/components/segmented";
import { Switch } from "@starter/ui/components/switch";
import { cn } from "@starter/ui/lib/utils";

import { engine } from "../../lib/engine";
import type { EngineState, Permission, PipCorner, PlatformOs } from "../../lib/types";
import { PIP_SHAPES, ShapeGlyph } from "../main-window/shape-glyph";
import { PermissionChip, type ChipState } from "./permission-chip";
import type { CaptureMode } from "./stage";
import { PLATFORM_FACTS, type PermissionCopy, type StepCopy } from "./steps";

const PERMISSION_ICON: Record<Permission, React.ReactNode> = {
  screen: <Monitor className="size-4" />,
  microphone: <Mic className="size-4" />,
  camera: <Video className="size-4" />,
};

/**
 * SOURCE OF TRUTH KEYWORDS: StepHeader, tour kicker, headline, lede, onboarding copy layout
 * WHAT:  The kicker, heavy headline and lede every tour step starts with.
 * WHY:   One rhythm for all nine steps; only the control under it changes.
 * WHERE: OnboardingWindow, every step.
 */
export function StepHeader({ copy, kicker, titleId }: { copy: StepCopy; kicker: string; titleId: string }) {
  return (
    <>
      <p className="text-[10.5px] font-semibold tracking-[0.08em] text-accent-text uppercase">{kicker}</p>
      <h1
        id={titleId}
        className="mt-2 font-display text-[29px] leading-[1.08] font-bold tracking-[-0.01em] text-balance"
      >
        {copy.title}
      </h1>
      <p className="mt-3 text-[13.5px] leading-relaxed text-pretty text-muted-foreground">{copy.lede}</p>
    </>
  );
}

export function WelcomeBody({ platform }: { platform: PlatformOs }) {
  const points: Array<{ icon: React.ReactNode; text: string }> = [
    { icon: <Monitor className="size-3.5" />, text: "A display, a window, an app or any area, up to 4K at 60 fps." },
    { icon: <Layers className="size-3.5" />, text: "Camera, microphone and system audio each keep their own track." },
    {
      icon: <HardDrive className="size-3.5" />,
      text: "Files save straight to this computer. No account, nothing uploaded.",
    },
  ];
  return (
    <>
      <ul className="mt-5 space-y-2.5">
        {points.map((p) => (
          <li key={p.text} className="flex items-start gap-3 text-[13px]">
            <span className="mt-px grid size-6 shrink-0 place-items-center rounded-[6px] bg-accent-tint text-accent-text">
              {p.icon}
            </span>
            <span className="pt-0.5">{p.text}</span>
          </li>
        ))}
      </ul>
      {platform === "macos" && (
        <Note icon={<Download className="size-3.5" />} className="mt-5">
          Opened Cortex from Downloads or a disk image? It will offer to move itself to Applications so updates can
          install.
        </Note>
      )}
    </>
  );
}

/**
 * SOURCE OF TRUTH KEYWORDS: PermissionsBody, permission rows, Allow, status chip, relaunch, privacy settings, portal
 * WHAT:  The permissions step's rows: one per permission the platform guards, with a live
 *        status chip and the real action (Allow, Reopen Cortex, Open privacy settings).
 * WHY:   Grants happen where they are explained, and the result shows at once. Only macOS's
 *        Screen Recording gates the tour; the rest can be skipped.
 * WHERE: OnboardingWindow, permissions step.
 */
export function PermissionsBody({
  platform,
  chips,
  onRequest,
  onOpenSettings,
  onRelaunch,
}: {
  platform: PlatformOs;
  chips: Record<Permission, ChipState>;
  onRequest: (p: Permission) => void;
  onOpenSettings: (p: Permission) => void;
  onRelaunch: () => void;
}) {
  const facts = PLATFORM_FACTS[platform];
  const model = facts.permissionModel;
  const anyWaiting = Object.values(chips).includes("waiting");
  // "Optional" says something only next to a "Required".
  const tagNeeds = facts.permissions.some((r) => r.need === "required");
  return (
    <>
      <ul className="mt-4 divide-y rounded-[14px] border bg-card shadow-ledge">
        {facts.permissions.map((row) => (
          <li key={row.permission} className="flex items-center gap-3 px-3 py-2">
            <span className="grid size-8 shrink-0 place-items-center rounded-[9px] bg-accent-tint text-accent-text">
              {PERMISSION_ICON[row.permission]}
            </span>
            <div className="min-w-0 flex-1">
              <div className="truncate text-[13px] font-medium" title={row.name}>
                {row.name}
              </div>
              {model === "portal" ? (
                <div className="truncate text-[12px] text-muted-foreground">{row.hint}</div>
              ) : (
                <div className="mt-1 flex items-center gap-2">
                  <PermissionChip state={chips[row.permission]} settingsApp={facts.settingsApp} />
                  {tagNeeds && (
                    <span
                      className={cn(
                        "text-[10.5px] font-semibold tracking-[0.04em] uppercase",
                        row.need === "required" ? "text-accent-text" : "text-muted-foreground",
                      )}
                    >
                      {row.need === "required" ? "Required" : "Optional"}
                    </span>
                  )}
                </div>
              )}
            </div>
            {model === "portal" && (
              <span className="inline-flex h-6 shrink-0 items-center rounded-full bg-muted px-2.5 text-[11.5px] font-medium text-muted-foreground">
                {row.allowLabel}
              </span>
            )}
            {model !== "portal" && (
              <PermissionAction
                row={row}
                model={model}
                chip={chips[row.permission]}
                busy={anyWaiting}
                onRequest={() => onRequest(row.permission)}
                onOpenSettings={() => onOpenSettings(row.permission)}
                onRelaunch={onRelaunch}
              />
            )}
          </li>
        ))}
      </ul>
      <div className="mt-3 space-y-1.5">
        {facts.permissionNotes.map((n) => (
          <Note key={n} icon={<Info className="size-3.5" />}>
            {n}
          </Note>
        ))}
      </div>
    </>
  );
}

function PermissionAction({
  row,
  model,
  chip,
  busy,
  onRequest,
  onOpenSettings,
  onRelaunch,
}: {
  row: PermissionCopy;
  model: "tcc" | "privacy";
  chip: ChipState;
  busy: boolean;
  onRequest: () => void;
  onOpenSettings: () => void;
  onRelaunch: () => void;
}) {
  if (chip === "relaunch") {
    return (
      <Button variant="tactile" size="sm" onClick={onRelaunch}>
        <RotateCcw /> Reopen Cortex
      </Button>
    );
  }
  if (chip === "granted") {
    return (
      <Button variant="soft" size="sm" disabled>
        <Check /> Allowed
      </Button>
    );
  }
  if (model === "privacy") {
    return (
      <Button variant="soft" size="sm" disabled={busy} onClick={onOpenSettings}>
        Open settings <ExternalLink data-icon="inline-end" />
      </Button>
    );
  }
  // macOS asks only once: after a denial the switch is in System Settings.
  return chip === "denied" ? (
    <Button variant="soft" size="sm" disabled={busy} onClick={onOpenSettings}>
      Open Settings <ExternalLink data-icon="inline-end" />
    </Button>
  ) : (
    <Button variant="tactile" size="sm" disabled={busy} onClick={onRequest}>
      <ShieldCheck /> {row.allowLabel}
    </Button>
  );
}

const CAPTURE_HINT: Record<CaptureMode, string> = {
  display: "Everything on one display. Cortex’s own windows are left out.",
  window: "Follows one window, even when it moves or sits behind others.",
  area: "Drag a rectangle on any display, or click a window to snap to it.",
  app: "Every window of one app on that display, including ones it opens later.",
};

export function CaptureBody({
  mode,
  onMode,
  onTryArea,
}: {
  mode: CaptureMode;
  onMode: (m: CaptureMode) => void;
  onTryArea: () => void;
}) {
  return (
    <>
      <Segmented<CaptureMode>
        aria-label="What to record"
        className="mt-5 w-full"
        value={mode}
        onValueChange={onMode}
        options={[
          {
            value: "display",
            label: (
              <>
                <Monitor /> Display
              </>
            ),
          },
          {
            value: "window",
            label: (
              <>
                <AppWindow /> Window
              </>
            ),
          },
          {
            value: "area",
            label: (
              <>
                <SquareDashed /> Area
              </>
            ),
          },
          {
            value: "app",
            label: (
              <>
                <LayoutGrid /> App
              </>
            ),
          },
        ]}
      />
      <p className="mt-2.5 min-h-[36px] text-[12.5px] leading-relaxed text-muted-foreground">{CAPTURE_HINT[mode]}</p>
      <div className="mt-2 flex items-center gap-3">
        <Button variant="soft" size="sm" onClick={onTryArea}>
          <SquareDashed /> Try the area picker
        </Button>
        <span className="text-[12px] text-muted-foreground">
          <Kbd>Esc</Kbd> to come back
        </span>
      </div>
    </>
  );
}

const CORNERS: ReadonlyArray<{ value: PipCorner; label: string; dot: string }> = [
  { value: "top-left", label: "Top left", dot: "top-1 left-1" },
  { value: "top-right", label: "Top right", dot: "top-1 right-1" },
  { value: "bottom-left", label: "Bottom left", dot: "bottom-1 left-1" },
  { value: "bottom-right", label: "Bottom right", dot: "right-1 bottom-1" },
];

/** The camera step: the real overlay shape, corner and camera-track settings. */
export function CameraBody({ e }: { e: EngineState }) {
  const s = e.settings;
  const corner = s.pipPlace.corner ?? null;
  return (
    <div className="mt-5 space-y-3">
      <div className="flex items-center gap-3">
        <span className="w-14 shrink-0 text-[12.5px] font-medium">Shape</span>
        <Segmented
          aria-label="Overlay shape"
          className="flex-1"
          value={s.pipShape}
          onValueChange={(pipShape) => engine.update({ pipShape })}
          options={PIP_SHAPES.map((sh) => ({
            value: sh.value,
            label: <ShapeGlyph shape={sh.value} />,
            title: sh.label,
          }))}
        />
      </div>
      <div className="flex items-center gap-3">
        <span className="w-14 shrink-0 text-[12.5px] font-medium">Corner</span>
        <div role="radiogroup" aria-label="Overlay corner" className="flex gap-1.5">
          {CORNERS.map((c) => {
            const on = corner === c.value;
            return (
              <button
                key={c.value}
                type="button"
                role="radio"
                aria-checked={on}
                aria-label={c.label}
                title={c.label}
                onClick={() => engine.update({ pipPlace: { corner: c.value } })}
                className={cn(
                  "relative h-7 w-10 rounded-[7px] border bg-card outline-none transition-[box-shadow,border-color] duration-150 focus-visible:ring-3 focus-visible:ring-ring/50",
                  on ? "border-primary shadow-ledge-accent" : "shadow-ledge hover:border-foreground/30",
                )}
              >
                <span
                  className={cn("absolute size-2 rounded-[3px]", c.dot, on ? "bg-primary" : "bg-muted-foreground/40")}
                />
              </button>
            );
          })}
        </div>
        <span className="text-[12px] text-muted-foreground">or drag it in the preview</span>
      </div>
      <label
        htmlFor="tour-camera-track"
        className="flex items-center gap-3 rounded-[12px] border bg-card px-3 py-2.5 shadow-ledge"
      >
        <span className="min-w-0 flex-1">
          <span className="block text-[13px] font-medium">Save camera as a separate track</span>
          <span className="block text-[12px] text-muted-foreground">
            The bubble stays in the video; the raw camera is kept too.
          </span>
        </span>
        <Switch
          id="tour-camera-track"
          checked={s.cameraTrack}
          onCheckedChange={(cameraTrack) => engine.update({ cameraTrack })}
        />
      </label>
    </div>
  );
}

/** The shortcut step: the keys, and whether a real press arrived. */
export function ShortcutBody({ platform, pressed }: { platform: PlatformOs; pressed: boolean }) {
  const f = PLATFORM_FACTS[platform];
  return (
    <>
      <div className="mt-5 flex items-center gap-3 rounded-[12px] border bg-card px-3 py-3 shadow-ledge">
        <span className="flex gap-1" aria-label={f.recordKeysLabel}>
          {f.recordKeys.map((k) => (
            <Kbd key={k} className="h-7 min-w-7 text-[13px] text-foreground">
              {k}
            </Kbd>
          ))}
        </span>
        <span role="status" aria-live="polite" className="text-[13px]">
          {pressed ? (
            <span className="flex items-center gap-1.5 font-medium text-ok">
              <Check className="size-4" strokeWidth={2.5} /> Got it. That would start a recording.
            </span>
          ) : (
            <span className="text-muted-foreground">Try it now: press the keys.</span>
          )}
        </span>
      </div>
      <Note icon={<Info className="size-3.5" />} className="mt-3">
        While this step is open the shortcut won’t start a recording. It works in every app, even full screen.
      </Note>
    </>
  );
}

export function FilesBody({ platform, outputDir }: { platform: PlatformOs; outputDir: string }) {
  const f = PLATFORM_FACTS[platform];
  return (
    <>
      <div className="mt-5 flex items-center gap-3 rounded-[12px] border bg-card px-3 py-2.5 shadow-ledge">
        <FolderOpen className="size-4 shrink-0 text-accent-text" />
        <span className="min-w-0 flex-1 truncate font-mono text-[12px]" title={outputDir}>
          {outputDir || f.defaultFolder}
        </span>
        <Button variant="soft" size="sm" onClick={() => engine.openRecordingsFolder()}>
          {f.revealLabel}
        </Button>
      </div>
      <ul className="mt-4 space-y-2.5">
        <Fact icon={<Layers className="size-3.5" />}>
          <b className="font-semibold">Three audio tracks.</b> The mix, system audio and your mic each get their own
          track, so you can fix a level later.
        </Fact>
        <Fact icon={<ShieldCheck className="size-3.5" />}>
          <b className="font-semibold">Crash-safe.</b> The file is saved every 2 seconds. A crash or power cut keeps
          everything up to that point.
        </Fact>
      </ul>
      <Note icon={<Info className="size-3.5" />} className="mt-4">
        Change the folder, format and quality in Settings › Recording.
      </Note>
    </>
  );
}

export function TrimBody({ onOpenTrim }: { onOpenTrim: () => void }) {
  return (
    <>
      <ul className="mt-5 space-y-2.5">
        <Fact icon={<Scissors className="size-3.5" />}>
          Opens from the “saved” notification after each recording, or from the menu.
        </Fact>
        <Fact icon={<span className="text-[10px] font-bold">I O</span>}>
          Press <Kbd>I</Kbd> and <Kbd>O</Kbd> to set the in and out points while it plays.
        </Fact>
        <Fact icon={<Layers className="size-3.5" />}>
          <b className="font-semibold">Fast</b> cuts at keyframes with no quality loss.{" "}
          <b className="font-semibold">Exact</b> is frame-accurate. Keep a copy or replace the original (it goes to the
          Trash).
        </Fact>
      </ul>
      <Button variant="soft" size="sm" className="mt-4" onClick={onOpenTrim}>
        <Scissors /> Open Quick trim
      </Button>
    </>
  );
}

export function ReplayLiveBody({
  e,
  platform,
  onOpenLiveSettings,
}: {
  e: EngineState;
  platform: PlatformOs;
  onOpenLiveSettings: () => void;
}) {
  const f = PLATFORM_FACTS[platform];
  const s = e.settings;
  return (
    <div className="mt-5 space-y-3">
      <label
        htmlFor="tour-replay"
        className="flex items-center gap-3 rounded-[12px] border bg-card px-3 py-2.5 shadow-ledge"
      >
        <span className="min-w-0 flex-1">
          <span className="flex items-center gap-2 text-[13px] font-medium">
            Keep a replay buffer
            <span className="flex gap-0.5" aria-label={f.replayKeysLabel}>
              {f.replayKeys.map((k) => (
                <Kbd key={k}>{k}</Kbd>
              ))}
            </span>
          </span>
          <span className="block text-[12px] text-muted-foreground">
            Holds the last {s.replaySecs} seconds in memory, nothing on disk until you save.
          </span>
        </span>
        <Switch
          id="tour-replay"
          checked={s.replayEnabled}
          onCheckedChange={(replayEnabled) => engine.update({ replayEnabled })}
        />
      </label>
      <div className="flex items-center gap-3 rounded-[12px] border bg-card px-3 py-2.5 shadow-ledge">
        <span className="grid size-8 shrink-0 place-items-center rounded-[9px] bg-rec-tint text-rec">
          <Radio className="size-4" />
        </span>
        <span className="min-w-0 flex-1">
          <span className="block text-[13px] font-medium">Go live</span>
          <span className="flex items-center gap-1 text-[12px] text-muted-foreground">
            <KeyRound className="size-3" /> Stream keys stay in {f.keyStore}.
          </span>
        </span>
        <Button variant="soft" size="sm" onClick={onOpenLiveSettings}>
          Set up
        </Button>
      </div>
    </div>
  );
}

export function DoneBody({ platform, chips }: { platform: PlatformOs; chips: Record<Permission, ChipState> }) {
  const f = PLATFORM_FACTS[platform];
  return (
    <>
      {f.permissionModel !== "portal" && (
        <ul className="mt-4 flex flex-wrap gap-1.5">
          {f.permissions.map((row) => {
            const ok = chips[row.permission] === "granted";
            return (
              <li
                key={row.permission}
                className={cn(
                  "inline-flex h-7 items-center gap-1.5 rounded-full px-2.5 text-[12px] font-medium",
                  ok ? "bg-ok-tint text-ok" : "bg-muted text-muted-foreground",
                )}
                title={ok ? undefined : row.skippedNote}
              >
                {ok ? <Check className="size-3.5" strokeWidth={2.5} /> : PERMISSION_ICON[row.permission]}
                {row.name.split(" ")[0]}
                {!ok && <span className="font-normal">· skipped</span>}
              </li>
            );
          })}
        </ul>
      )}
      <div className="mt-4 space-y-2">
        <div className="flex items-center gap-2 text-[13px]">
          <span className="text-muted-foreground">Start or stop from anywhere</span>
          <span className="flex gap-0.5" aria-label={f.recordKeysLabel}>
            {f.recordKeys.map((k) => (
              <Kbd key={k}>{k}</Kbd>
            ))}
          </span>
        </div>
        <Note icon={<Info className="size-3.5" />}>{f.reopenHint}</Note>
        <Note icon={<Monitor className="size-3.5" />}>{f.homeNote}</Note>
      </div>
    </>
  );
}

function Fact({ icon, children }: { icon: React.ReactNode; children: React.ReactNode }) {
  return (
    <li className="flex items-start gap-3 text-[13px] leading-relaxed">
      <span className="mt-0.5 grid size-6 shrink-0 place-items-center rounded-[6px] bg-accent-tint text-accent-text">
        {icon}
      </span>
      <span>{children}</span>
    </li>
  );
}

function Note({ icon, children, className }: { icon: React.ReactNode; children: React.ReactNode; className?: string }) {
  return (
    <p className={cn("flex items-start gap-2 text-[12px] leading-relaxed text-muted-foreground", className)}>
      <span className="mt-[3px] shrink-0">{icon}</span>
      <span>{children}</span>
    </p>
  );
}
