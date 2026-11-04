import { Scissors, Settings } from "lucide-react";

import { Button } from "@starter/ui/components/button";

import { ElapsedClock } from "../../components/progress-text";
import { useShortcut } from "../../components/shortcut-kbd";
import type { EngineState } from "../../lib/types";
import type { SurfaceId } from "../../prototype/surfaces";
import { Tip } from "./parts";

/**
 * SOURCE OF TRUTH KEYWORDS: MainTitleBar, title bar, recording status pill, LIVE badge, settings button, quick trim
 * WHAT:  The 44px inset title-bar zone of the main window: centred "Cortex" with the
 *        recording status (REC clock / Paused / Saving… / LIVE), plus Trim and Settings
 *        buttons on the right.
 * WHY:   The title bar is always visible, so it carries status (REC-09, STR-09) without
 *        another toolbar. The traffic lights are drawn by the window, so the left is empty.
 * WHERE: main window, top.
 */
export function MainTitleBar({ e, navigate }: { e: EngineState; navigate: (id: SurfaceId) => void }) {
  const settingsKeys = useShortcut("settings");
  const st = e.status;
  const hasRecording = e.recordings.length > 0;
  return (
    <div className="pointer-events-none absolute inset-x-0 top-0 z-20 flex h-11 items-center">
      <div className="absolute inset-x-0 flex items-center justify-center gap-2.5 text-[13px]">
        <span className="font-semibold">Cortex</span>
        {st === "recording" && (
          <span className="tabular flex items-center gap-1.5 font-mono text-[12px] text-rec" role="status">
            <span className="size-2 animate-rec-pulse rounded-full bg-rec" aria-hidden />
            <ElapsedClock />
          </span>
        )}
        {st === "paused" && (
          <span className="tabular flex items-center gap-1.5 font-mono text-[12px] text-warn" role="status">
            <span className="size-2 rounded-full bg-warn" aria-hidden />
            Paused <ElapsedClock />
          </span>
        )}
        {st === "finalizing" && (
          <span className="text-[12px] text-muted-foreground" role="status">
            Saving…
          </span>
        )}
        {st === "countdown" && (
          <span className="text-[12px] text-muted-foreground" role="status">
            Starting…
          </span>
        )}
        {e.live && (
          <span className="rounded-[4px] bg-rec px-1.5 py-px text-[10.5px] font-bold tracking-wider text-rec-fg">
            LIVE
          </span>
        )}
      </div>

      <div className="pointer-events-auto ml-auto flex items-center gap-0.5 pr-3">
        {e.capabilities.quickTrim && (
          <Tip label={hasRecording ? "Quick trim the last recording" : "No recordings to trim yet"} side="bottom">
            <span>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Quick trim last recording"
                disabled={!hasRecording}
                onClick={() => navigate("trim")}
              >
                <Scissors />
              </Button>
            </span>
          </Tip>
        )}
        <Tip label={`Settings: output, codec, audio, shortcuts (${settingsKeys})`} side="bottom">
          <Button variant="ghost" size="icon-sm" aria-label="Settings" onClick={() => navigate("settings")}>
            <Settings />
          </Button>
        </Tip>
      </div>
    </div>
  );
}
