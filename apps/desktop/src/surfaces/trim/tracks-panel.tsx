import type * as React from "react";
import { AudioLines, Eye, Lock, Mic, Monitor, Video } from "lucide-react";

import { Switch } from "@starter/ui/components/switch";
import { cn } from "@starter/ui/lib/utils";

import type { PlayerView } from "./player";

/** Which optional tracks the export keeps. Prototype-local: no engine field exists yet. */
export interface KeptTracks {
  camera: boolean;
  system: boolean;
  mic: boolean;
}

/**
 * SOURCE OF TRUTH KEYWORDS: TrimTracksPanel, kept tracks, camera track toggle, stems, include camera track
 * WHAT:  Lists the tracks in the recording and which of them the trimmed file will keep.
 * WHY:   Passthrough trim keeps every track by default (NEW-TR-04); the separate camera
 *        track (NEW-CT-01) is large and invisible in players, so it gets an explicit switch.
 * WHERE: TrimWindow, right column.
 */
export function TrimTracksPanel({
  hasCameraTrack,
  micName,
  kept,
  onKeptChange,
  view,
  onViewChange,
  disabled,
}: {
  hasCameraTrack: boolean;
  micName: string;
  kept: KeptTracks;
  onKeptChange: (k: KeptTracks) => void;
  view: PlayerView;
  onViewChange: (v: PlayerView) => void;
  disabled: boolean;
}) {
  const total = hasCameraTrack ? 4 : 3;
  const keptCount = 1 + (hasCameraTrack && kept.camera ? 1 : 0) + (kept.system ? 1 : 0) + (kept.mic ? 1 : 0);

  return (
    <section aria-label="Tracks" className="flex h-full flex-col">
      <div className="flex items-baseline justify-between px-4 pt-3 pb-2">
        <h2 className="text-[10.5px] font-semibold tracking-[0.08em] text-muted-foreground uppercase">Tracks</h2>
        <span className="text-[11px] text-muted-foreground tabular">
          Keeping {keptCount} of {total}
        </span>
      </div>
      <div className="flex flex-col gap-1 px-2">
        <TrackRow
          icon={<Monitor />}
          name="Screen"
          detail="Camera burned in"
          kept
          control={
            <span
              title="The screen track is always kept"
              className="grid size-6 place-items-center text-muted-foreground"
            >
              <Lock className="size-3.5" aria-label="Always kept" />
            </span>
          }
          active={view === "composite"}
          onSelect={() => onViewChange("composite")}
          selectLabel="Preview the screen track"
        />
        {hasCameraTrack && (
          <TrackRow
            icon={<Video />}
            name="Camera track"
            detail="Raw, for editors"
            kept={kept.camera}
            control={
              <Switch
                size="sm"
                checked={kept.camera}
                disabled={disabled}
                onCheckedChange={(camera) => onKeptChange({ ...kept, camera })}
                aria-label="Include camera track"
              />
            }
            active={view === "camera"}
            onSelect={() => onViewChange("camera")}
            selectLabel="Preview the camera track"
          />
        )}
        <TrackRow
          icon={<AudioLines />}
          name="System audio"
          detail="Stereo · own track"
          kept={kept.system}
          control={
            <Switch
              size="sm"
              checked={kept.system}
              disabled={disabled}
              onCheckedChange={(system) => onKeptChange({ ...kept, system })}
              aria-label="Keep system audio"
            />
          }
        />
        <TrackRow
          icon={<Mic />}
          name="Mic"
          detail={micName}
          kept={kept.mic}
          control={
            <Switch
              size="sm"
              checked={kept.mic}
              disabled={disabled}
              onCheckedChange={(mic) => onKeptChange({ ...kept, mic })}
              aria-label="Keep microphone"
            />
          }
        />
      </div>
      {hasCameraTrack && (
        <p className="mt-auto px-4 pb-3 text-[11.5px] leading-snug text-muted-foreground">
          {kept.camera
            ? "Players show the screen; Final Cut and Resolve also see the camera on its own track."
            : "The camera stays in the picture, but editors won't get it as a separate track."}
        </p>
      )}
    </section>
  );
}

function TrackRow({
  icon,
  name,
  detail,
  kept,
  control,
  active,
  onSelect,
  selectLabel,
}: {
  icon: React.ReactNode;
  name: string;
  detail: string;
  kept: boolean;
  control: React.ReactNode;
  active?: boolean;
  onSelect?: () => void;
  selectLabel?: string;
}) {
  return (
    <div
      className={cn(
        "group/row flex items-center gap-2.5 rounded-[10px] px-2 py-2 transition-[background,opacity] duration-150",
        active && "bg-card shadow-ledge",
      )}
    >
      <span
        className={cn(
          "grid size-7 shrink-0 place-items-center rounded-[6px] [&_svg]:size-3.5",
          kept ? "bg-accent-tint text-accent-text" : "bg-muted text-muted-foreground",
        )}
      >
        {icon}
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span
            className={cn(
              "truncate text-[13px] font-medium",
              !kept && "text-muted-foreground line-through decoration-muted-foreground/50",
            )}
          >
            {name}
          </span>
          {onSelect && (
            <button
              type="button"
              onClick={onSelect}
              aria-label={selectLabel}
              aria-pressed={active}
              title={selectLabel}
              className={cn(
                "grid size-5 place-items-center rounded-[5px] text-muted-foreground outline-none hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50",
                active ? "text-accent-text" : "opacity-0 group-hover/row:opacity-100 focus-visible:opacity-100",
              )}
            >
              <Eye className="size-3.5" />
            </button>
          )}
        </div>
        <p className="truncate text-[11.5px] text-muted-foreground" title={detail}>
          {detail}
        </p>
      </div>
      {control}
    </div>
  );
}
