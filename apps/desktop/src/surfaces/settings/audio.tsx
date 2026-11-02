import * as React from "react";
import { ChevronRight, Volume2, VolumeX } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { LevelMeter } from "@starter/ui/components/level-meter";
import { Segmented } from "@starter/ui/components/segmented";
import { Slider } from "@starter/ui/components/slider";
import { Switch } from "@starter/ui/components/switch";
import { cn } from "@starter/ui/lib/utils";

import { engine, useEngine } from "../../lib/engine";
import type { AudioTracks, InputDevice } from "../../lib/types";
import { ChoiceSelect, NumberField, Readout, SettingsGroup, SettingsPage, SettingsRow } from "./parts";

const DEFAULT_ID = "__default";
/** Output latency the engine would read from CoreAudio (mocked). */
const OUTPUT_LATENCY_MS = 28;
const MIN_DB = -60;

/** "-∞ dB", "0.0 dB", "+3.5 dB" as in Rec's mixer. */
export function formatDb(db: number) {
  if (db <= MIN_DB) return "−∞ dB";
  const v = db.toFixed(1);
  return db > 0 ? `+${v} dB` : `${v.replace("-", "−")} dB`;
}

/** Device options with the saved-but-unplugged device kept in the list (AUD-07). */
export function deviceOptions(devices: InputDevice[], defaultLabel: string) {
  return [
    { value: DEFAULT_ID, label: defaultLabel },
    ...devices.map((d) => ({ value: d.id, label: d.connected ? d.name : `${d.name} (not connected)` })),
  ];
}
export const toDeviceValue = (id: string | null) => id ?? DEFAULT_ID;
export const fromDeviceValue = (v: string) => (v === DEFAULT_ID ? null : v);

/** Audio: sources, backend, track layout, gain, sync offsets. Matrix AUD-02/03/04/05/07/09/11/16/18/19/23/24, FLG-10..14/30..32/42..44, SET-K10..18. */
export function AudioSection() {
  const e = useEngine();
  const s = e.settings;
  const [advanced, setAdvanced] = React.useState(false);

  return (
    <SettingsPage title="Audio" subtitle="What you hear in the recording, and how it is laid out in the file.">
      <SettingsGroup title="Sources">
        <SettingsRow
          label="Record system audio"
          hint="Sound from apps on this Mac. Cortex's own sounds are left out."
          htmlFor="set-sys"
        >
          <Switch
            id="set-sys"
            checked={s.systemAudio}
            onCheckedChange={(systemAudio) => engine.update({ systemAudio })}
          />
        </SettingsRow>
        <SettingsRow
          label="Record microphone"
          hint="Your voice. Recording still starts if the mic is missing."
          htmlFor="set-mic"
        >
          <Switch id="set-mic" checked={s.micEnabled} onCheckedChange={(micEnabled) => engine.update({ micEnabled })} />
        </SettingsRow>
        <SettingsRow
          label="Microphone"
          hint="If it is unplugged, Cortex uses the default and tells you."
          disabled={!s.micEnabled}
        >
          <ChoiceSelect<string>
            aria-label="Microphone"
            disabled={!s.micEnabled}
            value={toDeviceValue(s.micId)}
            onChange={(v) => engine.update({ micId: fromDeviceValue(v) })}
            options={deviceOptions(e.mics, "Default microphone")}
            className="w-52"
          />
        </SettingsRow>
        <div>
          <button
            type="button"
            aria-expanded={advanced}
            onClick={() => setAdvanced((a) => !a)}
            className="flex w-full items-center gap-1.5 px-4 py-2.5 text-left text-xs font-medium text-muted-foreground outline-none hover:text-foreground focus-visible:text-foreground"
          >
            <ChevronRight className={cn("size-3.5 transition-transform duration-150", advanced && "rotate-90")} />
            Advanced
          </button>
          {advanced && (
            <div className="border-t">
              <SettingsRow
                label="Microphone capture"
                hint={
                  s.micBackend === "sck"
                    ? "ScreenCaptureKit: same clock as the screen. Best sync."
                    : "AVCapture: for devices ScreenCaptureKit cannot open."
                }
              >
                <Segmented<"sck" | "avcapture">
                  aria-label="Microphone capture backend"
                  size="sm"
                  value={s.micBackend}
                  onValueChange={(micBackend) => engine.update({ micBackend })}
                  options={[
                    { value: "sck", label: "ScreenCaptureKit" },
                    { value: "avcapture", label: "AVCapture" },
                  ]}
                />
              </SettingsRow>
            </div>
          )}
        </div>
      </SettingsGroup>

      <SettingsGroup
        title="Tracks"
        footer="QuickTime plays only the first track, the mix. Editors like Final Cut and DaVinci see every track."
      >
        <SettingsRow
          label="Audio tracks"
          stacked
          hint={
            s.audioTracks === "both"
              ? "A mixed track plus separate system and mic tracks for editing."
              : s.audioTracks === "mix"
                ? "One mixed track. Smallest file, plays everywhere."
                : "Separate system and mic tracks, no mix. Some players will sound silent."
          }
        >
          <Segmented<AudioTracks>
            aria-label="Audio tracks"
            className="w-full"
            value={s.audioTracks}
            onValueChange={(audioTracks) => engine.update({ audioTracks })}
            options={[
              { value: "both", label: "Mix + separate" },
              { value: "mix", label: "Mix only" },
              { value: "stems", label: "Separate only" },
            ]}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup
        title="Levels"
        footer="Double-click a slider to reset it to 0 dB. Changes apply immediately, even while recording."
      >
        <GainRow
          label="System audio"
          enabled={s.systemAudio}
          db={s.systemGainDb}
          muted={s.systemMuted}
          level={e.levels.system}
          peak={e.levels.systemPeak}
          onDb={(systemGainDb) => engine.update({ systemGainDb })}
          onMute={(systemMuted) => engine.update({ systemMuted })}
        />
        <GainRow
          label="Microphone"
          enabled={s.micEnabled}
          db={s.micGainDb}
          muted={s.micMuted}
          level={e.levels.mic}
          peak={e.levels.micPeak}
          onDb={(micGainDb) => engine.update({ micGainDb })}
          onMute={(micMuted) => engine.update({ micMuted })}
        />
      </SettingsGroup>

      <SettingsGroup
        title="Sync"
        footer="Use offsets only if lips and sound drift apart. Positive values delay the audio."
      >
        <SettingsRow
          label="System audio offset"
          hint={
            s.systemOffsetMs === null
              ? `Automatic: matches your output device's latency (${OUTPUT_LATENCY_MS} ms).`
              : "Fixed offset in milliseconds. Negative values are allowed."
          }
        >
          {s.systemOffsetMs !== null && (
            <NumberField
              aria-label="System audio offset in milliseconds"
              value={s.systemOffsetMs}
              min={-1000}
              max={1000}
              unit="ms"
              onCommit={(systemOffsetMs) => engine.update({ systemOffsetMs })}
            />
          )}
          <Segmented<"auto" | "manual">
            aria-label="System audio offset mode"
            value={s.systemOffsetMs === null ? "auto" : "manual"}
            onValueChange={(m) => engine.update({ systemOffsetMs: m === "auto" ? null : OUTPUT_LATENCY_MS })}
            options={[
              { value: "auto", label: "Auto" },
              { value: "manual", label: "Manual" },
            ]}
          />
        </SettingsRow>
        <SettingsRow label="Microphone offset" hint="Bluetooth headsets often lag by 150–250 ms.">
          <NumberField
            aria-label="Microphone offset in milliseconds"
            value={s.micOffsetMs}
            min={-1000}
            max={1000}
            unit="ms"
            onCommit={(micOffsetMs) => engine.update({ micOffsetMs })}
          />
        </SettingsRow>
      </SettingsGroup>
    </SettingsPage>
  );
}

function GainRow({
  label,
  enabled,
  db,
  muted,
  level,
  peak,
  onDb,
  onMute,
}: {
  label: string;
  enabled: boolean;
  db: number;
  muted: boolean;
  level: number;
  peak: number;
  onDb: (db: number) => void;
  onMute: (muted: boolean) => void;
}) {
  return (
    <div className={cn("flex items-center gap-3 px-4 py-3", !enabled && "opacity-45")}>
      <div className="w-28 shrink-0">
        <div className="text-[13px] font-medium">{label}</div>
        <div className="text-xs text-muted-foreground">{enabled ? (muted ? "Muted" : "Live") : "Off"}</div>
      </div>
      <div className="flex min-w-0 flex-1 flex-col gap-2" onDoubleClick={() => onDb(0)}>
        <Slider
          aria-label={`${label} gain`}
          disabled={!enabled}
          min={MIN_DB}
          max={24}
          step={0.5}
          value={[db]}
          onValueChange={([v]) => {
            if (v !== undefined) onDb(v);
          }}
        />
        <LevelMeter level={level} peak={peak} muted={muted || !enabled} className="h-1.5" segments={32} />
      </div>
      <Readout>{formatDb(db)}</Readout>
      <Button
        variant="ghost"
        size="icon-sm"
        disabled={!enabled}
        aria-pressed={muted}
        aria-label={muted ? `Unmute ${label}` : `Mute ${label}`}
        title={muted ? `Unmute ${label}` : `Mute ${label}`}
        onClick={() => onMute(!muted)}
        className={cn(muted && "text-warn")}
      >
        {muted ? <VolumeX /> : <Volume2 />}
      </Button>
    </div>
  );
}
