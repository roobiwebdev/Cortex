import * as React from "react";

import { Segmented } from "@starter/ui/components/segmented";
import { Switch } from "@starter/ui/components/switch";

import { engine, useEngine } from "../../lib/engine";
import type { Codec, Container, Fps, Resolution } from "../../lib/types";
import { OutputFolderRow, NamingRow } from "./files";
import { ChoiceSelect, NumberField, SettingsGroup, SettingsPage, SettingsRow } from "./parts";

const RESOLUTIONS: ReadonlyArray<{ value: Resolution | "custom"; label: string; detail?: string }> = [
  { value: "720p", label: "720p" },
  { value: "1080p", label: "1080p" },
  { value: "1440p", label: "1440p" },
  { value: "4k", label: "4K" },
  { value: "native", label: "Native", detail: "display size" },
  { value: "custom", label: "Custom…" },
];
const FPS: readonly Fps[] = [24, 30, 60];
const LONG_EDGE: Record<Resolution, number> = { "720p": 1280, "1080p": 1920, "1440p": 2560, "4k": 3840, native: 3456 };

/** Estimated automatic bitrate (Mb/s), shown so "Auto" is not a black box. */
export function autoBitrate(res: Resolution, fps: number) {
  const px = LONG_EDGE[res] * Math.round((LONG_EDGE[res] * 9) / 16);
  return Math.round((px * fps * 0.1) / 1_000_000);
}

/** Recording: frame size, rate, codec, container, bitrate, cursor/clicks, countdown, output. Matrix SET-K01..06, K27/28, FLG-05/07/08/09/26/27. */
export function RecordingSection() {
  const s = useEngine().settings;
  // prototype-local: custom W×H is not in RecorderSettings yet (Resolution needs a { w, h } variant).
  const [custom, setCustom] = React.useState<{ on: boolean; w: number; h: number }>({ on: false, w: 1600, h: 1000 });
  const proRes = s.codec === "prores";

  return (
    <SettingsPage title="Recording" subtitle="Frame size, smoothness and file format for new recordings.">
      <SettingsGroup title="Video">
        <SettingsRow label="Resolution" hint="Scaled to fit inside this size. Cortex never upscales past your display.">
          <ChoiceSelect<Resolution | "custom">
            aria-label="Resolution"
            value={custom.on ? "custom" : s.resolution}
            onChange={(v) => {
              if (v === "custom") return setCustom((c) => ({ ...c, on: true }));
              setCustom((c) => ({ ...c, on: false }));
              engine.update({ resolution: v });
            }}
            options={RESOLUTIONS}
          />
        </SettingsRow>
        {custom.on && (
          <SettingsRow
            label="Custom size"
            hint="Width × height in pixels. The frame keeps the capture's shape."
            badge="prototype"
          >
            <NumberField
              aria-label="Width"
              value={custom.w}
              min={320}
              max={7680}
              step={2}
              unit="W"
              onCommit={(w) => setCustom((c) => ({ ...c, w }))}
            />
            <span className="text-muted-foreground">×</span>
            <NumberField
              aria-label="Height"
              value={custom.h}
              min={240}
              max={4320}
              step={2}
              unit="H"
              onCommit={(h) => setCustom((c) => ({ ...c, h }))}
            />
          </SettingsRow>
        )}
        <SettingsRow label="Frame rate" hint="60 fps for motion and scrolling. 30 fps halves the file size of slides.">
          <Segmented<string>
            aria-label="Frame rate"
            value={String(s.fps)}
            onValueChange={(v) => {
              const fps = FPS.find((f) => String(f) === v);
              if (fps) engine.update({ fps });
            }}
            options={FPS.map((f) => ({ value: String(f), label: <span className="tabular">{f} fps</span> }))}
          />
        </SettingsRow>
        <SettingsRow
          label="Codec"
          hint={
            proRes
              ? "ProRes 422: large files, smoothest editing. Saved as MOV."
              : s.codec === "auto"
                ? "H.264 up to 1080p, HEVC above. Hardware encoded."
                : s.codec === "hevc"
                  ? "About half the size of H.264. Plays on any recent Mac."
                  : "Plays everywhere. Larger files above 1080p."
          }
        >
          <ChoiceSelect<Codec>
            aria-label="Codec"
            value={s.codec}
            onChange={(codec) => engine.update(codec === "prores" ? { codec, container: "mov" } : { codec })}
            options={[
              { value: "auto", label: "Auto" },
              { value: "h264", label: "H.264" },
              { value: "hevc", label: "HEVC (H.265)" },
              { value: "prores", label: "ProRes 422" },
            ]}
          />
        </SettingsRow>
        <SettingsRow
          label="Container"
          hint={
            proRes
              ? "ProRes needs MOV."
              : s.container === "mov"
                ? "MOV keeps separate audio and camera tracks."
                : "MP4 for sharing. Extra tracks become a side file."
          }
        >
          <Segmented<Container>
            aria-label="Container"
            value={s.container}
            onValueChange={(container) => {
              if (!(proRes && container === "mp4")) engine.update({ container });
            }}
            options={[
              { value: "mov", label: "MOV" },
              { value: "mp4", label: "MP4", title: proRes ? "ProRes needs MOV" : undefined },
            ]}
          />
        </SettingsRow>
        <SettingsRow
          label="Bitrate"
          hint={
            s.bitrateMbps === null
              ? `Automatic, about ${autoBitrate(s.resolution, s.fps)} Mb/s at these settings.`
              : `About ${((s.bitrateMbps * 3600) / 8 / 1000).toFixed(1)} GB per hour.`
          }
        >
          {s.bitrateMbps !== null && (
            <NumberField
              aria-label="Bitrate in megabits per second"
              value={s.bitrateMbps}
              min={1}
              max={400}
              unit="Mb/s"
              width="w-[104px]"
              onCommit={(bitrateMbps) => engine.update({ bitrateMbps })}
            />
          )}
          <Segmented<"auto" | "manual">
            aria-label="Bitrate mode"
            value={s.bitrateMbps === null ? "auto" : "manual"}
            onValueChange={(m) =>
              engine.update({ bitrateMbps: m === "auto" ? null : autoBitrate(s.resolution, s.fps) })
            }
            options={[
              { value: "auto", label: "Auto" },
              { value: "manual", label: "Manual" },
            ]}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup title="Capture">
        <SettingsRow label="Show cursor" hint="Draw the pointer into the video." htmlFor="set-cursor">
          <Switch id="set-cursor" checked={s.cursor} onCheckedChange={(cursor) => engine.update({ cursor })} />
        </SettingsRow>
        <SettingsRow label="Highlight clicks" hint="A soft ring appears wherever you click." htmlFor="set-clicks">
          <Switch id="set-clicks" checked={s.clicks} onCheckedChange={(clicks) => engine.update({ clicks })} />
        </SettingsRow>
        <SettingsRow label="Countdown" hint="Time to get ready after you press Record. Esc cancels.">
          <Segmented<string>
            aria-label="Countdown"
            value={String(s.countdown)}
            onValueChange={(v) => engine.update({ countdown: v === "5" ? 5 : v === "3" ? 3 : 0 })}
            options={[
              { value: "0", label: "Off" },
              { value: "3", label: "3 s" },
              { value: "5", label: "5 s" },
            ]}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup title="Output">
        <OutputFolderRow />
        <NamingRow />
      </SettingsGroup>
    </SettingsPage>
  );
}
