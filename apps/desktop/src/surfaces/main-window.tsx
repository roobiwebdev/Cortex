import * as React from "react";

import { TooltipProvider } from "@starter/ui/components/tooltip";

import { useEngine } from "../lib/mock-engine";
import type { SurfaceProps } from "../prototype/surfaces";
import { AudioMixer } from "./main-window/audio-mixer";
import { RecordBar } from "./main-window/record-bar";
import { SourcesSidebar } from "./main-window/sources-sidebar";
import { StageArea } from "./main-window/stage-area";
import { MainTitleBar } from "./main-window/title-bar";

/**
 * SOURCE OF TRUTH KEYWORDS: MainWindow, main window, recorder window, preview, sources, mixer, record bar
 * WHAT:  The recorder's main window: title bar status, sources sidebar, live preview with
 *        output chips, audio mixer and record bar.
 * WHY:   Mirrors Rec's main window (UI-02) re-laid out for calm progressive disclosure:
 *        choose sources on the left, watch the result in the centre, act at the bottom.
 * WHERE: prototype shell surface "main"; later the Tauri main window.
 */
export function MainWindow({ cameraStream, navigate }: SurfaceProps) {
  const e = useEngine();

  // ⌘, opens Settings while the main window is focused (KEY-04).
  React.useEffect(() => {
    const onKey = (ev: KeyboardEvent) => {
      if (ev.metaKey && ev.key === ",") {
        ev.preventDefault();
        navigate("settings");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate]);

  return (
    <TooltipProvider delayDuration={400}>
      <div className="relative flex size-full bg-background text-foreground">
        <MainTitleBar e={e} navigate={navigate} />
        <SourcesSidebar e={e} navigate={navigate} />
        <main className="flex min-w-0 flex-1 flex-col pt-11">
          <StageArea e={e} cameraStream={cameraStream} navigate={navigate} />
          <div className="mt-3 border-t bg-card">
            <AudioMixer e={e} />
            <RecordBar e={e} navigate={navigate} />
          </div>
        </main>
      </div>
    </TooltipProvider>
  );
}
