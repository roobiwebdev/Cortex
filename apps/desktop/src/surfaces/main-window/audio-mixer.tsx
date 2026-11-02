import type * as React from "react";
import { Mic, MicOff, Speaker, Volume2, VolumeX } from "lucide-react";

import { LevelMeter, type LevelSource } from "@starter/ui/components/level-meter";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@starter/ui/components/select";
import { Slider } from "@starter/ui/components/slider";
import { cn } from "@starter/ui/lib/utils";

import { engine, micLevels, systemLevels } from "../../lib/engine";
import type { EngineState } from "../../lib/types";
import { Tip } from "./parts";

const MIN_DB = -60;
const MAX_DB = 24;
// Mixing-desk taper: unity gain sits at 70% of the fader travel (AUD-17).
const UNITY_AT = 70;

function posToDb(pos: number) {
  if (pos >= UNITY_AT) return ((pos - UNITY_AT) / (100 - UNITY_AT)) * MAX_DB;
  return MIN_DB * Math.pow(1 - pos / UNITY_AT, 2);
}
function dbToPos(db: number) {
  if (db >= 0) return UNITY_AT + (db / MAX_DB) * (100 - UNITY_AT);
  return UNITY_AT * (1 - Math.sqrt(Math.min(1, db / MIN_DB)));
}
function formatDb(db: number) {
  if (db <= MIN_DB) return "−∞ dB";
  const v = Math.abs(db).toFixed(1);
  return db > 0.05 ? `+${v} dB` : db < -0.05 ? `−${v} dB` : "0.0 dB";
}

/**
 * SOURCE OF TRUTH KEYWORDS: AudioMixer, MixerStrip, gain fader, mute, level meter, system audio, microphone
 * WHAT:  The two-strip mixer (System audio, Microphone): meter with peak hold, a tapered gain
 *        fader (−∞…+24 dB, double-click for 0 dB), a dB readout and a mute toggle.
 * WHY:   Levels must be readable at a glance before and during a take (AUD-16…AUD-21);
 *        a muted or switched-off source dims so its state is obvious.
 * WHERE: main window, above the record bar.
 */
export function AudioMixer({ e }: { e: EngineState }) {
  const s = e.settings;
  return (
    <div className="space-y-1 px-4 py-2.5" role="group" aria-label="Audio mixer">
      <MixerStrip
        icon={<Speaker />}
        name="System audio"
        device={<span className="truncate text-[12px] text-muted-foreground">All sounds from this Mac</span>}
        enabled={s.systemAudio}
        levels={systemLevels}
        gainDb={s.systemGainDb}
        muted={s.systemMuted}
        onGain={(systemGainDb) => engine.update({ systemGainDb })}
        onMute={() => engine.update({ systemMuted: !s.systemMuted })}
        muteIcon={s.systemMuted ? <VolumeX /> : <Volume2 />}
      />
      <MixerStrip
        icon={<Mic />}
        name="Microphone"
        device={
          <Select value={s.micId ?? undefined} onValueChange={(micId) => engine.update({ micId })}>
            <SelectTrigger size="sm" className="h-7 w-full min-w-0 bg-card text-[12px]" aria-label="Microphone device">
              <SelectValue placeholder="Choose a microphone" />
            </SelectTrigger>
            <SelectContent position="popper">
              {e.mics.map((m) => (
                <SelectItem key={m.id} value={m.id} disabled={!m.connected}>
                  {m.connected ? m.name : `${m.name} (not connected)`}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        }
        enabled={s.micEnabled}
        levels={micLevels}
        gainDb={s.micGainDb}
        muted={s.micMuted}
        onGain={(micGainDb) => engine.update({ micGainDb })}
        onMute={() => engine.update({ micMuted: !s.micMuted })}
        muteIcon={s.micMuted ? <MicOff /> : <Mic />}
      />
    </div>
  );
}

function MixerStrip({
  icon,
  name,
  device,
  enabled,
  levels,
  gainDb,
  muted,
  onGain,
  onMute,
  muteIcon,
}: {
  icon: React.ReactNode;
  name: string;
  device: React.ReactNode;
  enabled: boolean;
  levels: LevelSource;
  gainDb: number;
  muted: boolean;
  onGain: (db: number) => void;
  onMute: () => void;
  muteIcon: React.ReactNode;
}) {
  const dim = muted || !enabled;
  return (
    <div className="flex h-10 items-center gap-3">
      <div className={cn("flex w-[340px] shrink-0 items-center gap-2.5 transition-opacity", dim && "opacity-45")}>
        <span className="grid size-7 shrink-0 place-items-center rounded-[8px] bg-muted text-muted-foreground [&_svg]:size-3.5">
          {icon}
        </span>
        <span className="w-[92px] shrink-0 text-[13px] font-medium">{name}</span>
        <span className="flex min-w-0 flex-1">{device}</span>
      </div>

      <LevelMeter source={levels} muted={dim} className="h-1.5 min-w-0 flex-1" aria-label={`${name} level`} />

      <div className={cn("flex w-[188px] shrink-0 items-center gap-2.5 transition-opacity", dim && "opacity-45")}>
        <Tip label="Drag to set gain. Double-click for 0 dB.">
          <span className="flex flex-1" onDoubleClick={() => onGain(0)}>
            <Slider
              aria-label={`${name} gain`}
              aria-valuetext={formatDb(gainDb)}
              min={0}
              max={100}
              step={0.5}
              value={[dbToPos(gainDb)]}
              onValueChange={([p]) => p !== undefined && onGain(Math.round(posToDb(p) * 2) / 2)}
              disabled={!enabled}
            />
          </span>
        </Tip>
        <span className="tabular w-[54px] text-right font-mono text-[11px] text-muted-foreground">
          {enabled ? formatDb(gainDb) : "Off"}
        </span>
      </div>

      <Tip label={muted ? `Unmute ${name.toLowerCase()}` : `Mute ${name.toLowerCase()}`}>
        <button
          type="button"
          aria-pressed={muted}
          aria-label={muted ? `Unmute ${name}` : `Mute ${name}`}
          onClick={onMute}
          disabled={!enabled}
          className={cn(
            "grid size-7 shrink-0 place-items-center rounded-[8px] transition-colors outline-none focus-visible:ring-3 focus-visible:ring-ring/50 disabled:opacity-40 [&_svg]:size-4",
            muted ? "bg-warn-tint text-warn" : "text-muted-foreground hover:bg-muted hover:text-foreground",
          )}
        >
          {muteIcon}
        </button>
      </Tip>
    </div>
  );
}
