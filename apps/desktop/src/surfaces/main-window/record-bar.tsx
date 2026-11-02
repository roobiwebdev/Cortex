import type * as React from "react";
import { Circle, History, Pause, Play, Square } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Kbd } from "@starter/ui/components/kbd";
import { cn } from "@starter/ui/lib/utils";

import { formatBytes, formatClock } from "../../lib/format";
import { engine } from "../../lib/mock-engine";
import type { EngineState } from "../../lib/types";
import type { SurfaceId } from "../../prototype/surfaces";
import { GoLiveButton } from "./go-live";
import { Tip } from "./parts";

// Space must never start or stop a take (KEY-10); ⇧⌘R and clicks still work.
function blockSpace(ev: React.KeyboardEvent) {
  if (ev.key === " ") ev.preventDefault();
}

/**
 * SOURCE OF TRUTH KEYWORDS: RecordBar, record button, stop, pause, resume, clock, save replay, go live
 * WHAT:  The main window's bottom bar: elapsed clock and bytes written, the Record/Stop and
 *        Pause/Resume buttons, Save replay and Go live.
 * WHY:   One big unmistakable action (REC-01) with the clock as the glanceable number
 *        (REC-08, REC-10). Red appears only when recording.
 * WHERE: main window, bottom.
 */
export function RecordBar({ e, navigate }: { e: EngineState; navigate: (id: SurfaceId) => void }) {
  const st = e.status;
  const active = st === "recording" || st === "paused";
  const s = e.settings;

  return (
    <div className="grid h-[72px] shrink-0 grid-cols-[1fr_auto_1fr] items-center gap-4 border-t px-5">
      {/* Clock: red while recording, grey otherwise. */}
      <div className="min-w-0" aria-live="off">
        <div
          className={cn(
            "tabular font-mono text-[26px] leading-none font-medium tracking-tight",
            st === "recording" ? "text-rec" : active ? "text-foreground" : "text-muted-foreground/70",
          )}
        >
          {formatClock(active || st === "finalizing" ? e.elapsedSecs : 0)}
        </div>
        <div className="tabular mt-1.5 truncate text-[11.5px] text-muted-foreground">
          {st === "recording" && `${formatBytes(e.bytesWritten)} written`}
          {st === "paused" && <span className="text-warn">Paused · {formatBytes(e.bytesWritten)} written</span>}
          {st === "finalizing" && "Saving to " + s.outputDir}
          {st === "countdown" && `Starting in ${e.countdownLeft} s`}
          {st === "idle" && `Saves to ${s.outputDir}`}
        </div>
      </div>

      <div className="flex items-center gap-2">
        <RecordButton e={e} />
        <Tip
          label={
            active ? (st === "paused" ? "Resume recording" : "Pause recording") : "Pause is available while recording"
          }
        >
          <span>
            <Button
              variant="soft"
              size="icon-lg"
              className="size-11 rounded-[14px]"
              disabled={!active}
              aria-label={st === "paused" ? "Resume" : "Pause"}
              onClick={() => engine.togglePause()}
            >
              {st === "paused" ? <Play /> : <Pause />}
            </Button>
          </span>
        </Tip>
      </div>

      <div className="flex items-center justify-end gap-2">
        {s.replayEnabled && (
          <Tip label="Save the buffer to an MP4 without stopping (⇧⌘S)">
            <Button variant="soft" size="lg" onClick={() => engine.saveReplay()}>
              <History />
              Save replay
              <Kbd className="ml-0.5 shadow-none">⇧⌘S</Kbd>
            </Button>
          </Tip>
        )}
        <GoLiveButton e={e} navigate={navigate} />
      </div>
    </div>
  );
}

function RecordButton({ e }: { e: EngineState }) {
  const st = e.status;
  if (st === "finalizing") {
    return (
      <Button variant="soft" size="xl" disabled className="w-[164px]">
        Saving…
      </Button>
    );
  }
  if (st === "countdown") {
    return (
      <Tip label="Cancel the countdown (Esc)">
        <Button
          variant="soft"
          size="xl"
          className="w-[164px]"
          onClick={() => engine.cancelCountdown()}
          onKeyDown={blockSpace}
          onKeyUp={blockSpace}
        >
          Cancel
          <Kbd className="shadow-none">Esc</Kbd>
        </Button>
      </Tip>
    );
  }
  const stop = st === "recording" || st === "paused";
  // Without Screen Recording permission only the camera scene can record (PRM-05).
  const blocked = !stop && e.settings.scene !== "cam" && e.permissions.screen !== "granted";
  return (
    <Tip
      label={
        blocked
          ? "Turn on Screen Recording to record your screen"
          : stop
            ? "Stop and save the recording (⇧⌘R)"
            : "Start recording (⇧⌘R)"
      }
    >
      <span>
        <Button
          variant="record"
          size="xl"
          className="w-[164px]"
          disabled={blocked}
          onClick={() => engine.toggleRecord()}
          onKeyDown={blockSpace}
          onKeyUp={blockSpace}
        >
          {stop ? <Square className="fill-current" /> : <Circle className="fill-current" />}
          {stop ? "Stop" : "Record"}
          <Kbd className="border-transparent bg-rec-fg/20 text-rec-fg shadow-none">⇧⌘R</Kbd>
        </Button>
      </span>
    </Tip>
  );
}
