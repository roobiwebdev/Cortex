import { Loader2, PanelBottom, Play, Square } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Kbd } from "@starter/ui/components/kbd";
import { Segmented } from "@starter/ui/components/segmented";

import { formatClock } from "../lib/format";
import { engine, useEngine } from "../lib/mock-engine";
import type { RecorderSettings } from "../lib/types";
import type { SurfaceProps } from "../prototype/surfaces";
import { CaptureExcludedHint, PrototypeNote } from "./floating/floating-card";
import { isCapturing, RecStatus } from "./floating/status";

type CountdownSecs = RecorderSettings["countdown"];
const COUNTDOWN_OPTIONS: ReadonlyArray<{ value: CountdownSecs; label: string }> = [
  { value: 0, label: "Off" },
  { value: 3, label: "3 s" },
  { value: 5, label: "5 s" },
];

/**
 * SOURCE OF TRUTH KEYWORDS: CountdownSurface, countdown, 3-2-1, pre-roll, Esc to cancel, settings.countdown
 * WHAT:  The centred 3-2-1 square shown between pressing Record and the first captured frame.
 * WHY:   Gives the user a beat to switch windows or start talking. It is left out of capture
 *        and removed before the first frame, and Esc cancels without creating a file.
 * WHERE: Prototype surface "countdown"; later an above-floating, click-through panel.
 */
export function CountdownSurface({ navigate }: SurfaceProps) {
  const e = useEngine();
  const n = e.settings.countdown;

  if (e.status === "countdown") {
    return (
      <div className="absolute inset-0 top-7 grid place-items-center">
        <div className="flex flex-col items-center gap-4">
          <div
            role="timer"
            aria-label={`Recording starts in ${e.countdownLeft}`}
            className="grid size-[170px] place-items-center rounded-[36px] bg-popover/80 shadow-float backdrop-blur-xl"
          >
            <span
              key={e.countdownLeft}
              className="font-display text-[104px] leading-none font-bold tabular text-foreground duration-500 ease-spring animate-in fade-in zoom-in-125"
            >
              {e.countdownLeft}
            </span>
          </div>
          <div className="flex items-center gap-2 rounded-full bg-popover/80 px-3 py-1.5 text-xs text-muted-foreground shadow-float backdrop-blur-xl">
            <Kbd>Esc</Kbd>
            to cancel
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="absolute inset-0 top-7 grid place-items-center p-6">
      {e.status === "idle" ? (
        <div className="flex flex-col items-center gap-3">
          <PrototypeNote
            title="Countdown before recording"
            actions={
              <Button
                variant="tactile"
                onClick={() => {
                  if (n === 0) engine.update({ countdown: 3 });
                  engine.start();
                }}
              >
                <Play className="fill-current" />
                Preview countdown
              </Button>
            }
          >
            After you press Record, a short count shows in the middle of the screen so you can get ready. Press Esc to
            cancel. The preview starts a real (mock) recording when it reaches zero.
            <div className="mt-3 flex items-center justify-between gap-3 text-foreground">
              <span className="text-[13px] font-medium">Count</span>
              <Segmented<string>
                aria-label="Countdown length"
                size="sm"
                value={String(n)}
                onValueChange={(v) => {
                  const pick = COUNTDOWN_OPTIONS.find((o) => String(o.value) === v);
                  if (pick) engine.update({ countdown: pick.value });
                }}
                options={COUNTDOWN_OPTIONS.map((o) => ({ value: String(o.value), label: o.label }))}
              />
            </div>
          </PrototypeNote>
          <CaptureExcludedHint>Never part of the recording</CaptureExcludedHint>
        </div>
      ) : (
        <PrototypeNote
          title={
            <span className="flex items-center gap-2">
              {e.status === "finalizing" ? "Saving recording…" : "Recording started"}
              <RecStatus status={e.status} />
            </span>
          }
          actions={
            e.status === "finalizing" ? (
              <span className="flex items-center gap-2 text-[13px] text-muted-foreground">
                <Loader2 className="size-4 animate-spin" aria-hidden />
                Writing the file
              </span>
            ) : (
              <>
                <Button variant="soft" onClick={() => navigate("hud")}>
                  <PanelBottom />
                  Show the HUD
                </Button>
                {isCapturing(e.status) && (
                  <Button variant="record" onClick={() => engine.stop()}>
                    <Square className="fill-current" />
                    Stop
                  </Button>
                )}
              </>
            )
          }
        >
          The countdown closed before the first frame, so it is not in the video. Elapsed{" "}
          <span className="font-medium text-foreground tabular">{formatClock(e.elapsedSecs)}</span>.
        </PrototypeNote>
      )}
    </div>
  );
}
