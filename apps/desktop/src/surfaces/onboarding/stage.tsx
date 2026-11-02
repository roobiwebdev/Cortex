import type * as React from "react";
import { CameraOff, Lock, Mic, Speaker, TriangleAlert, Video, Wifi } from "lucide-react";

import { LevelMeter, type LevelSource } from "@starter/ui/components/level-meter";
import { cn } from "@starter/ui/lib/utils";

import { CameraBubble } from "../../components/camera-bubble";
import { micLevels, silentLevels, systemLevels } from "../../lib/engine";
import type { EngineState } from "../../lib/types";
import { MockScreen } from "../../prototype/mock-screen";
import type { ChipState } from "./permission-chip";
import type { StepId } from "./steps";

/**
 * SOURCE OF TRUTH KEYWORDS: OnboardingStage, permission illustration, live mock, teach by doing, onboarding left pane
 * WHAT:  The left half of onboarding: a small live mock of what the current step unlocks
 *        (screen capture, mic meter, camera bubble, menu bar home).
 * WHY:   Each permission is easier to grant when the user sees what it does. The mock
 *        reacts to the real permission state, so granting visibly "turns it on".
 * WHERE: OnboardingWindow only.
 */
export function OnboardingStage({
  step,
  engine,
  chips,
  cameraStream,
}: {
  step: StepId;
  engine: EngineState;
  /** Chip state per permission, including the prototype-local "waiting". */
  chips: Record<"screen" | "microphone" | "camera", ChipState>;
  cameraStream: MediaStream | null;
}) {
  return (
    <div className="relative size-full overflow-hidden bg-muted">
      {/* Soft accent wash so the stage reads as a "scene", not a panel. */}
      <div className="absolute inset-0 bg-gradient-to-br from-accent-tint via-transparent to-transparent opacity-80" />
      <div
        key={step}
        className="relative flex size-full items-center justify-center px-10 pt-11 pb-6 duration-300 ease-out-quick animate-in fade-in-0 slide-in-from-bottom-2"
      >
        {step === "welcome" && <WelcomeScene engine={engine} cameraStream={cameraStream} />}
        {step === "screen" && <ScreenScene state={chips.screen} />}
        {step === "microphone" && <MicScene engine={engine} state={chips.microphone} />}
        {step === "camera" && <CameraScene engine={engine} state={chips.camera} cameraStream={cameraStream} />}
        {step === "ready" && <ReadyScene />}
      </div>
    </div>
  );
}

/** A small framed display with optional overlays on top. */
function MiniScreen({ children, className }: { children?: React.ReactNode; className?: string }) {
  return (
    <div
      className={cn("relative aspect-[16/10] w-[300px] overflow-hidden rounded-[10px] bg-card shadow-float", className)}
    >
      <MockScreen />
      {children}
    </div>
  );
}

function RecPill({ secs }: { secs: number }) {
  return (
    <div className="absolute top-3 left-3 flex h-6 items-center gap-1.5 rounded-full bg-card/90 px-2 text-[11px] font-semibold shadow-ledge backdrop-blur">
      <span className="size-2 rounded-full bg-rec animate-rec-pulse" aria-hidden />
      <span className="tabular">0:{String(secs).padStart(2, "0")}</span>
    </div>
  );
}

function WelcomeScene({ engine, cameraStream }: { engine: EngineState; cameraStream: MediaStream | null }) {
  const s = engine.settings;
  return (
    <div className="relative">
      <MiniScreen>
        <RecPill secs={12} />
        <CameraBubble
          className="absolute right-3 bottom-3"
          shape={s.pipShape}
          roundness={s.pipRoundness}
          width={56}
          border={0}
          shadow
          mirror={s.mirror}
          stream={cameraStream}
        />
      </MiniScreen>
      {/* Mini HUD: the floating control a recording gets. */}
      <div className="absolute -bottom-7 left-1/2 flex h-10 -translate-x-1/2 items-center gap-3 rounded-[18px] bg-card px-3 shadow-float">
        <span className="grid size-6 place-items-center rounded-full bg-rec-tint">
          <span className="size-2.5 rounded-[3px] bg-rec" />
        </span>
        <span className="text-[13px] font-semibold tabular">0:12</span>
        <LevelMeter className="w-16" source={micLevels} segments={12} />
      </div>
    </div>
  );
}

function ScreenScene({ state }: { state: ChipState }) {
  const granted = state === "granted";
  return (
    <div className="flex flex-col items-center gap-4">
      <MiniScreen className={cn(granted && "ring-2 ring-primary")}>
        {granted ? (
          <RecPill secs={4} />
        ) : (
          <div className="absolute inset-0 grid place-items-center bg-background/70 backdrop-blur-sm">
            <div className="flex flex-col items-center gap-2 text-center">
              <span
                className={cn(
                  "grid size-9 place-items-center rounded-full bg-card shadow-ledge",
                  state === "denied" ? "text-warn" : "text-muted-foreground",
                )}
              >
                {state === "denied" ? <TriangleAlert className="size-4" /> : <Lock className="size-4" />}
              </span>
              <span className="max-w-[220px] text-[12px] font-medium text-muted-foreground">
                {state === "waiting" ? "Waiting for your answer…" : "Cortex can’t see your screen yet"}
              </span>
            </div>
          </div>
        )}
      </MiniScreen>
      <SourceRow
        icon={<Speaker className="size-3.5" />}
        label="System audio"
        levels={granted ? systemLevels : silentLevels}
        live={granted}
      />
    </div>
  );
}

function MicScene({ engine, state }: { engine: EngineState; state: ChipState }) {
  const granted = state === "granted";
  const mic = engine.mics.find((m) => m.id === engine.settings.micId) ?? engine.mics[0];
  return (
    <div className="w-[300px] rounded-[18px] bg-card p-4 shadow-float">
      <div className="flex items-center gap-3">
        <span
          className={cn(
            "grid size-9 place-items-center rounded-[10px] transition-colors",
            granted ? "bg-accent-tint text-accent-text" : "bg-muted text-muted-foreground",
          )}
        >
          <Mic className="size-4" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="truncate text-[13px] font-medium">{mic?.name ?? "Microphone"}</div>
          <div className="text-[11.5px] text-muted-foreground">
            {granted ? "Listening · try saying something" : "No access yet"}
          </div>
        </div>
      </div>
      <LevelMeter
        className="mt-4 h-3"
        source={granted ? micLevels : silentLevels}
        muted={!granted}
        segments={28}
        aria-label="Microphone level"
      />
      <div className="mt-2 flex justify-between text-[10.5px] text-muted-foreground tabular">
        <span>−60</span>
        <span>−24</span>
        <span>−12</span>
        <span>0 dB</span>
      </div>
    </div>
  );
}

function CameraScene({
  engine,
  state,
  cameraStream,
}: {
  engine: EngineState;
  state: ChipState;
  cameraStream: MediaStream | null;
}) {
  const granted = state === "granted";
  const cam = engine.cameras.find((c) => c.id === engine.settings.cameraId) ?? engine.cameras[0];
  return (
    <div className="flex flex-col items-center gap-5">
      <div className={cn("relative rounded-full transition-shadow duration-300", granted && "ring-2 ring-primary")}>
        <CameraBubble
          className={cn("transition-[opacity,filter] duration-300", !granted && "opacity-40 grayscale")}
          shape="circle"
          roundness={1}
          width={196}
          border={granted ? 4 : 0}
          shadow={granted}
          mirror={engine.settings.mirror}
          stream={granted ? cameraStream : null}
        />
        {!granted && (
          <span className="absolute inset-0 grid place-items-center">
            <span className="grid size-10 place-items-center rounded-full bg-card text-muted-foreground shadow-ledge">
              <CameraOff className="size-4" />
            </span>
          </span>
        )}
      </div>
      <div className="flex h-7 items-center gap-2 rounded-full bg-card px-3 text-[12px] font-medium shadow-ledge">
        <Video className={cn("size-3.5", granted ? "text-accent-text" : "text-muted-foreground")} />
        {cam?.name ?? "Camera"}
      </div>
    </div>
  );
}

function ReadyScene() {
  return (
    <div className="relative w-[300px]">
      <MiniScreen>
        {/* Mock macOS menu bar with Cortex's status item highlighted. */}
        <div className="absolute inset-x-0 top-0 flex h-[18px] items-center justify-end gap-2.5 bg-card/85 px-2 text-[9px] font-medium text-foreground backdrop-blur">
          <span className="grid size-[14px] place-items-center rounded-[4px] bg-primary">
            <span className="size-1.5 rounded-full bg-primary-foreground" />
          </span>
          <Wifi className="size-2.5" />
          <span className="tabular">Mon 9:41</span>
        </div>
        <div className="absolute top-[22px] right-[70px] w-[150px] space-y-1.5 rounded-[10px] bg-card p-2 shadow-float">
          <div className="text-[9.5px] font-semibold">Cortex</div>
          <div className="flex h-6 items-center justify-center gap-1 rounded-[7px] bg-rec text-[9.5px] font-semibold text-rec-fg shadow-ledge-rec">
            <span className="size-1.5 rounded-full bg-rec-fg" /> Start recording
          </div>
          <div className="h-1.5 w-3/4 rounded-full bg-muted" />
          <div className="h-1.5 w-1/2 rounded-full bg-muted" />
        </div>
      </MiniScreen>
    </div>
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
    <div className="flex w-[300px] items-center gap-3 rounded-[14px] bg-card px-3 py-2.5 shadow-ledge">
      <span
        className={cn(
          "grid size-7 place-items-center rounded-[8px]",
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
