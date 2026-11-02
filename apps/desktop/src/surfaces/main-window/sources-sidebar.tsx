import { Segmented } from "@starter/ui/components/segmented";
import { Slider } from "@starter/ui/components/slider";

import { engine } from "../../lib/mock-engine";
import type { EngineState, Scene } from "../../lib/types";
import type { SurfaceId } from "../../prototype/surfaces";
import { CameraSection } from "./camera-section";
import { CaptureSection } from "./capture-section";
import { FieldRow, SIDEBAR_SLIDER, SidebarSection, SwitchRow } from "./parts";

// Rough in-memory cost of the replay buffer at the default live bitrate (~8 Mb/s).
const REPLAY_BYTES_PER_SEC = 1_000_000;

/**
 * SOURCE OF TRUTH KEYWORDS: SourcesSidebar, main window sidebar, capture, scene, camera, options, replay buffer
 * WHAT:  The main window's left sidebar: CAPTURE, SCENE, CAMERA and OPTIONS sections, each
 *        collapsible, scrolling as one column.
 * WHY:   Everything that decides *what* gets recorded lives in one place, next to the preview
 *        that shows the result (UI-02, UI-12). Source choices lock while recording (UI-15).
 * WHERE: main window, left column.
 */
export function SourcesSidebar({ e, navigate }: { e: EngineState; navigate: (id: SurfaceId) => void }) {
  const s = e.settings;
  const locked = e.status !== "idle";
  const lockNote = locked ? "Locked while recording" : undefined;

  return (
    <aside
      aria-label="Sources"
      className="flex w-[280px] shrink-0 flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground"
    >
      {/* Title-bar inset: the traffic lights sit here. */}
      <div className="h-11 shrink-0" />
      <div className="min-h-0 flex-1 overflow-y-auto pb-4 [scrollbar-width:thin]">
        <SidebarSection title="Capture" aside={lockNote}>
          <CaptureSection e={e} locked={locked} navigate={navigate} />
        </SidebarSection>

        <SidebarSection title="Scene">
          <Segmented<Scene>
            aria-label="Scene"
            size="sm"
            className="w-full bg-foreground/[0.06]"
            value={s.scene}
            onValueChange={(scene) => engine.update({ scene })}
            options={[
              { value: "screen", label: "Screen", title: "Screen only" },
              { value: "screen-cam", label: "Screen + Cam", title: "Screen with the camera as an overlay" },
              { value: "cam", label: "Camera", title: "Camera only" },
            ]}
          />
        </SidebarSection>

        <SidebarSection title="Camera">
          <CameraSection e={e} locked={locked} />
        </SidebarSection>

        <SidebarSection title="Options">
          <SwitchRow label="Show cursor" checked={s.cursor} onCheckedChange={(v) => engine.update({ cursor: v })} />
          <SwitchRow
            label="Highlight clicks"
            hint="Draws a ring where you click."
            checked={s.clicks}
            onCheckedChange={(v) => engine.update({ clicks: v })}
          />
          <div className="flex items-center justify-between gap-3">
            <span className="text-[13px]">Countdown</span>
            <Segmented<"0" | "3" | "5">
              aria-label="Countdown before recording"
              size="sm"
              className="bg-foreground/[0.06]"
              value={s.countdown === 5 ? "5" : s.countdown === 3 ? "3" : "0"}
              onValueChange={(v) => engine.update({ countdown: v === "5" ? 5 : v === "3" ? 3 : 0 })}
              options={[
                { value: "0", label: "Off" },
                { value: "3", label: "3 s" },
                { value: "5", label: "5 s" },
              ]}
            />
          </div>
          <SwitchRow
            label="Replay buffer"
            hint="Keeps recent footage in memory so you can save it after it happens (⇧⌘S)."
            checked={s.replayEnabled}
            onCheckedChange={(v) => engine.update({ replayEnabled: v })}
          />
          {s.replayEnabled && (
            <FieldRow
              label="Keep the last"
              value={`${s.replaySecs} s · ~${Math.round((s.replaySecs * REPLAY_BYTES_PER_SEC) / 1e6)} MB`}
            >
              <Slider
                className={SIDEBAR_SLIDER}
                aria-label="Replay buffer length in seconds"
                min={15}
                max={300}
                step={5}
                value={[s.replaySecs]}
                onValueChange={([v]) => v !== undefined && engine.update({ replaySecs: v })}
              />
            </FieldRow>
          )}
        </SidebarSection>
      </div>
    </aside>
  );
}
