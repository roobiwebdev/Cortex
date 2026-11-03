import type * as React from "react";
import { HardDrive, History, Loader2, MonitorX, SquareUser } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { cn } from "@starter/ui/lib/utils";

import { FreeDisk } from "../../components/progress-text";
import { PreviewStage } from "../../components/preview-stage";
import { engine, useEngine } from "../../lib/engine";
import type { Codec, EngineState, Resolution } from "../../lib/types";
import { formatSize, recordedOutput, sameOutput, sourcePixels } from "../../lib/output-size";
import { MockScreen } from "../../prototype/mock-screen";
import type { SurfaceId } from "../../prototype/surfaces";
import { PermissionAsk } from "../../components/permission-ask";
import { Tip } from "./parts";

const RES_LABEL: Record<Resolution, string> = {
  "720p": "720p",
  "1080p": "1080p",
  "1440p": "1440p",
  "4k": "4K",
  native: "Native",
};
const CODEC_LABEL: Record<Codec, string> = { auto: "Auto", h264: "H.264", hevc: "HEVC", prores: "ProRes" };
const LOW_DISK = 5e9;

/** The recorded frame's shape (width / height), so the preview stage matches the output. */
function targetAspect(e: EngineState): number {
  const px = sourcePixels(e.settings.target, e);
  return px && px.w > 0 && px.h > 0 ? px.w / px.h : 16 / 10;
}

/**
 * SOURCE OF TRUTH KEYWORDS: StageArea, preview, countdown overlay, permission denied, saving overlay, output chips
 * WHAT:  The main window centre: the live preview stage with its state overlays (countdown,
 *        saving, Screen Recording off) and the output chip row underneath.
 * WHY:   The preview is the one thing to look at, so states show *on* it rather than in
 *        banners (PRV-07, PRM-05), and the format readout sits right under it (OUT-20).
 * WHERE: main window, centre column.
 */
export function StageArea({
  e,
  cameraStream,
  navigate,
}: {
  e: EngineState;
  cameraStream: MediaStream | null;
  navigate: (id: SurfaceId) => void;
}) {
  const s = e.settings;
  const recording = e.status === "recording" || e.status === "paused";
  const needsScreen = s.scene !== "cam";
  const denied = needsScreen && e.permissions.screen !== "granted";
  const aspect = targetAspect(e);

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2.5 px-4 pt-3">
      <div className="relative flex min-h-0 flex-1 flex-col">
        {denied ? (
          <PermissionCard onFix={() => navigate("onboarding")} />
        ) : (
          <PreviewStage
            settings={s}
            aspect={aspect}
            recording={recording}
            paused={e.status === "paused"}
            cameraStream={cameraStream}
            onPipPlace={(pipPlace) => engine.update({ pipPlace })}
            cameraBlocked={
              e.permissions.camera === "granted" ? undefined : (
                <PermissionAsk permission="camera" state={e.permissions.camera} />
              )
            }
          >
            <MockScreen />
          </PreviewStage>
        )}

        {e.status === "countdown" && (
          <StageOverlay>
            <div
              key={e.countdownLeft}
              className="grid size-40 animate-in place-items-center rounded-[28px] bg-card/90 text-foreground shadow-float backdrop-blur duration-200 ease-out-quick zoom-in-90 fade-in-0"
            >
              <span className="tabular font-display text-[88px] leading-none font-bold" aria-live="assertive">
                {e.countdownLeft}
              </span>
            </div>
            <p className="mt-4 rounded-full bg-card/80 px-3 py-1 text-[12px] text-muted-foreground backdrop-blur">
              Recording starts soon · <span className="text-foreground">Esc</span> to cancel
            </p>
          </StageOverlay>
        )}

        {e.status === "finalizing" && (
          <StageOverlay>
            <div
              className="flex items-center gap-2 rounded-full bg-card/90 px-4 py-2 text-[13px] shadow-float backdrop-blur"
              role="status"
            >
              <Loader2 className="size-4 animate-spin text-muted-foreground" aria-hidden />
              Saving…
            </div>
          </StageOverlay>
        )}
      </div>

      <ChipRow e={e} navigate={navigate} />
    </div>
  );
}

function StageOverlay({ children }: { children: React.ReactNode }) {
  return (
    <div className="absolute inset-0 z-10 flex flex-col items-center justify-center rounded-[18px] bg-stage/55">
      {children}
    </div>
  );
}

function PermissionCard({ onFix }: { onFix: () => void }) {
  return (
    <div className="flex min-h-0 flex-1 items-center justify-center rounded-[18px] bg-stage p-6">
      <div className="w-full max-w-sm rounded-[18px] bg-card p-6 shadow-float">
        <span className="grid size-10 place-items-center rounded-[12px] bg-warn-tint text-warn">
          <MonitorX className="size-5" aria-hidden />
        </span>
        <h2 className="mt-4 font-display text-[20px] leading-tight font-bold tracking-tight">
          Screen Recording is off
        </h2>
        <p className="mt-1.5 text-[13px] leading-5 text-muted-foreground">
          macOS needs your permission before Cortex can see your screen. Turn it on in System Settings, then come back
          here. Camera-only recording still works.
        </p>
        <Button variant="tactile" className="mt-5 w-full" onClick={onFix}>
          Turn on Screen Recording
        </Button>
      </div>
    </div>
  );
}

const isLowDisk = (s: EngineState) => s.freeDiskBytes < LOW_DISK;

function ChipRow({ e, navigate }: { e: EngineState; navigate: (id: SurfaceId) => void }) {
  const s = e.settings;
  const out = useEngine(recordedOutput, sameOutput);
  // What gets written: the real frame size and the codec "Auto" picks for it.
  const format = [
    RES_LABEL[s.resolution],
    ...(out ? [formatSize(out.size)] : []),
    `${s.fps} fps`,
    CODEC_LABEL[out?.codec ?? s.codec],
    s.container.toUpperCase(),
  ].join(" · ");
  const lowDisk = useEngine(isLowDisk);
  return (
    <div className="flex h-7 shrink-0 items-center gap-1.5">
      <Tip
        label={
          s.codec === "auto"
            ? "Output format (codec set to Auto). Change in Settings"
            : "Output format. Change in Settings"
        }
      >
        <button
          type="button"
          onClick={() => navigate("settings")}
          className={cn(chip, "hover:bg-muted hover:text-foreground")}
        >
          {format}
        </button>
      </Tip>
      {s.scene !== "screen" && s.cameraTrack && (
        <span className={chip}>
          <SquareUser aria-hidden />
          Camera track
        </span>
      )}
      {s.replayEnabled && (
        <span className={chip}>
          <History aria-hidden />
          Replay {s.replaySecs}s
        </span>
      )}
      <span className={cn(chip, "ml-auto", lowDisk && "border-warn/40 bg-warn-tint text-warn")}>
        <HardDrive aria-hidden />
        <FreeDisk /> free
      </span>
    </div>
  );
}

const chip =
  "tabular inline-flex h-6 items-center gap-1.5 rounded-[6px] border bg-card px-2 font-mono text-[11px] text-muted-foreground transition-colors outline-none focus-visible:ring-3 focus-visible:ring-ring/50 [&_svg]:size-3";
