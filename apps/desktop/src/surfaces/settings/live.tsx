import * as React from "react";
import { KeyRound, Lock, Plus, X } from "lucide-react";

import { Button } from "@starter/ui/components/button";
import { Kbd } from "@starter/ui/components/kbd";
import { Segmented } from "@starter/ui/components/segmented";
import { Slider } from "@starter/ui/components/slider";
import { Switch } from "@starter/ui/components/switch";
import { cn } from "@starter/ui/lib/utils";

import { engine, useEngine } from "../../lib/mock-engine";
import type { Resolution, StreamDest, StreamService } from "../../lib/types";
import { ChoiceSelect, Readout, SettingsGroup, SettingsPage, SettingsRow } from "./parts";

const SERVICE: Record<Exclude<StreamService, "custom">, { name: string; url: string; maxKbps: number }> = {
  youtube: { name: "YouTube", url: "rtmps://a.rtmps.youtube.com/live2", maxKbps: 45_000 },
  twitch: { name: "Twitch", url: "rtmps://live.twitch.tv/app", maxKbps: 6_000 },
  kick: { name: "Kick", url: "rtmps://…global-contribute.live-video.net/app", maxKbps: 8_000 },
};
const YT_BY_RES: Record<Resolution, number> = {
  "720p": 6_000,
  "1080p": 12_000,
  "1440p": 24_000,
  "4k": 45_000,
  native: 45_000,
};
const LIVE_RES: readonly Resolution[] = ["720p", "1080p", "1440p", "4k"];
const LIVE_SIZE: Record<Resolution, string> = {
  "720p": "1280×720",
  "1080p": "1920×1080",
  "1440p": "2560×1440",
  "4k": "3840×2160",
  native: "native",
};
const AUDIO_KBPS = [96, 128, 160, 192, 256, 320] as const;

const isWhip = (d: StreamDest) => d.service === "custom" && /^https?:\/\//.test(d.url ?? "");

/** Platform preset: the lowest limit across enabled destinations (STR-14). */
function presetKbps(dests: StreamDest[], res: Resolution) {
  const caps = dests
    .filter((d) => d.enabled)
    .map((d) =>
      d.service === "youtube" ? YT_BY_RES[res] : d.service === "custom" ? YT_BY_RES[res] : SERVICE[d.service].maxKbps,
    );
  return caps.length ? Math.min(...caps) : YT_BY_RES[res];
}

/** Live & Replay: replay buffer, destinations with Keychain keys, live encode. Matrix SET-K30..39, STR-01..05/07/14/24/27..34, FLG-33..41, ENV-05. */
export function LiveSection() {
  const e = useEngine();
  const s = e.settings;
  const locked = e.live;
  const enabled = s.streamDests.filter((d) => d.enabled);
  const onlyYouTube = enabled.length > 0 && enabled.every((d) => d.service === "youtube");
  const codec = s.streamCodec === "hevc" || (s.streamCodec === "auto" && onlyYouTube) ? "HEVC" : "H.264";
  const kbps = s.streamBitrateAuto ? presetKbps(s.streamDests, s.liveResolution) : s.streamBitrateKbps;

  function setDest(i: number, patch: Partial<StreamDest>) {
    engine.update({ streamDests: s.streamDests.map((d, j) => (j === i ? { ...d, ...patch } : d)) });
  }

  return (
    <SettingsPage
      title="Live & Replay"
      subtitle="Keep the last moments ready to save, and stream to one or more platforms."
    >
      <SettingsGroup title="Instant replay">
        <SettingsRow
          label="Replay buffer"
          htmlFor="set-replay"
          hint={
            <>
              Keeps the last {s.replaySecs} s in memory. Press <Kbd>⇧⌘S</Kbd> to save it as an MP4.
            </>
          }
        >
          <Switch
            id="set-replay"
            checked={s.replayEnabled}
            onCheckedChange={(replayEnabled) => engine.update({ replayEnabled })}
          />
        </SettingsRow>
        <div className={cn("flex min-h-[48px] items-center gap-4 px-4 py-2.5", !s.replayEnabled && "opacity-45")}>
          <span className="w-24 shrink-0 text-[13px] font-medium">Length</span>
          <Slider
            aria-label="Replay length"
            disabled={!s.replayEnabled}
            min={15}
            max={300}
            step={5}
            value={[s.replaySecs]}
            onValueChange={([v]) => {
              if (v !== undefined) engine.update({ replaySecs: v });
            }}
            className="flex-1"
          />
          <Readout>
            {s.replaySecs >= 60
              ? `${Math.floor(s.replaySecs / 60)}:${String(s.replaySecs % 60).padStart(2, "0")} min`
              : `${s.replaySecs} s`}
          </Readout>
        </div>
      </SettingsGroup>

      <SettingsGroup
        title="Destinations"
        action={
          <Button
            variant="ghost"
            size="xs"
            disabled={locked}
            onClick={() =>
              engine.update({
                streamDests: [...s.streamDests, { service: "custom", url: "", enabled: true, keyStored: false }],
              })
            }
          >
            <Plus /> Add server
          </Button>
        }
        footer={
          locked
            ? "Stop streaming to change destinations."
            : "Stream keys are stored in your macOS Keychain, never in settings files or logs."
        }
      >
        {s.streamDests.map((d, i) => (
          <DestRow
            key={i}
            dest={d}
            locked={locked}
            onChange={(p) => setDest(i, p)}
            onRemove={() => engine.update({ streamDests: s.streamDests.filter((_, j) => j !== i) })}
          />
        ))}
      </SettingsGroup>

      <SettingsGroup
        title="Live encode"
        footer={
          locked ? (
            <span className="inline-flex items-center gap-1.5">
              <Lock className="size-3" /> Stop streaming to change the live encode.
            </span>
          ) : enabled.length > 1 ? (
            `One encode feeds every destination, within each one's limits.`
          ) : undefined
        }
      >
        <div className="bg-muted/50 px-4 py-2.5">
          <div className="tabular text-[13px] font-medium">
            Up to {LIVE_SIZE[s.liveResolution]} {codec} at {(kbps / 1000).toFixed(kbps % 1000 ? 1 : 0)} Mb/s
          </div>
          <div className="text-xs text-muted-foreground">
            The frame keeps its shape and never exceeds the recording size.
          </div>
        </div>
        <SettingsRow label="Resolution" hint="Largest size sent to viewers." disabled={locked}>
          <Segmented<string>
            aria-label="Live resolution"
            value={s.liveResolution}
            onValueChange={(v) => {
              const r = LIVE_RES.find((x) => x === v);
              if (r && !locked) engine.update({ liveResolution: r });
            }}
            options={LIVE_RES.map((r) => ({ value: r, label: r === "4k" ? "4K" : r }))}
          />
        </SettingsRow>
        <SettingsRow label="Codec" hint="Auto uses HEVC only when every destination is YouTube." disabled={locked}>
          <ChoiceSelect<"auto" | "h264" | "hevc">
            aria-label="Live codec"
            disabled={locked}
            value={s.streamCodec}
            onChange={(streamCodec) => engine.update({ streamCodec })}
            options={[
              { value: "auto", label: "Auto" },
              { value: "h264", label: "H.264" },
              { value: "hevc", label: "HEVC", detail: "falls back to H.264" },
            ]}
          />
        </SettingsRow>
        <SettingsRow
          label="Video bitrate"
          disabled={locked}
          hint={
            s.streamBitrateAuto
              ? "Platform preset: the highest rate every destination accepts."
              : "Fixed rate. Higher looks better but needs a faster upload."
          }
        >
          <Segmented<"auto" | "manual">
            aria-label="Video bitrate mode"
            value={s.streamBitrateAuto ? "auto" : "manual"}
            onValueChange={(m) => !locked && engine.update({ streamBitrateAuto: m === "auto" })}
            options={[
              { value: "auto", label: "Preset" },
              { value: "manual", label: "Manual" },
            ]}
          />
        </SettingsRow>
        {!s.streamBitrateAuto && (
          <div className={cn("flex min-h-[48px] items-center gap-4 px-4 py-2.5", locked && "opacity-45")}>
            <span className="w-24 shrink-0 text-[13px] font-medium">Rate</span>
            <Slider
              aria-label="Video bitrate"
              disabled={locked}
              min={1000}
              max={60000}
              step={500}
              value={[s.streamBitrateKbps]}
              onValueChange={([v]) => {
                if (v !== undefined) engine.update({ streamBitrateKbps: v });
              }}
              className="flex-1"
            />
            <Readout className="w-20">{s.streamBitrateKbps.toLocaleString("en-US")} kb/s</Readout>
          </div>
        )}
        <SettingsRow
          label="Constant bitrate (CBR)"
          hint="Platforms prefer CBR. Turn off for variable bitrate."
          htmlFor="set-cbr"
          disabled={locked}
        >
          <Switch
            id="set-cbr"
            disabled={locked}
            checked={s.streamCbr}
            onCheckedChange={(streamCbr) => engine.update({ streamCbr })}
          />
        </SettingsRow>
        <div className={cn("flex min-h-[48px] items-center gap-4 px-4 py-2.5", locked && "opacity-45")}>
          <span className="w-24 shrink-0 text-[13px] font-medium">Keyframes</span>
          <Slider
            aria-label="Keyframe interval"
            disabled={locked}
            min={1}
            max={4}
            step={0.5}
            value={[s.keyframeSecs]}
            onValueChange={([v]) => {
              if (v !== undefined) engine.update({ keyframeSecs: v });
            }}
            className="flex-1"
          />
          <Readout className="w-20">every {s.keyframeSecs} s</Readout>
        </div>
        <SettingsRow label="Audio bitrate" hint="AAC for RTMP, Opus for WHIP." disabled={locked}>
          <ChoiceSelect<string>
            aria-label="Audio bitrate"
            disabled={locked}
            className="min-w-28"
            value={String(s.streamAudioKbps)}
            onChange={(v) => {
              const k = AUDIO_KBPS.find((x) => String(x) === v);
              if (k) engine.update({ streamAudioKbps: k });
            }}
            options={AUDIO_KBPS.map((k) => ({ value: String(k), label: `${k} kb/s` }))}
          />
        </SettingsRow>
      </SettingsGroup>
    </SettingsPage>
  );
}

function DestRow({
  dest,
  locked,
  onChange,
  onRemove,
}: {
  dest: StreamDest;
  locked: boolean;
  onChange: (p: Partial<StreamDest>) => void;
  onRemove: () => void;
}) {
  const [editing, setEditing] = React.useState(false);
  const [draft, setDraft] = React.useState("");
  const preset = dest.service === "custom" ? null : SERVICE[dest.service];
  const custom = preset === null;
  const whip = isWhip(dest);
  const name = preset ? preset.name : whip ? "WHIP server" : "Custom server";

  function cancel() {
    setDraft("");
    setEditing(false);
  }
  function saveKey() {
    if (!draft.trim()) return;
    // prototype: the key would go straight to Keychain; the UI keeps only "stored: yes".
    setDraft("");
    setEditing(false);
    onChange({ keyStored: true });
  }

  return (
    <div className="px-4 py-3">
      <div className="flex items-center gap-3">
        <span className="grid size-7 shrink-0 place-items-center rounded-[6px] bg-muted text-xs font-semibold text-muted-foreground">
          {name.slice(0, 1)}
        </span>
        <div className="min-w-0 flex-1">
          <div className="text-[13px] font-medium">{name}</div>
          {!preset ? (
            <input
              aria-label="Server URL"
              disabled={locked}
              value={dest.url ?? ""}
              placeholder="rtmp://… or https://…/whip"
              onChange={(e) => onChange({ url: e.target.value })}
              className="mt-1 h-7 w-full rounded-[8px] border bg-card px-2 font-mono text-xs outline-none placeholder:text-muted-foreground/70 focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
            />
          ) : (
            <div className="truncate font-mono text-[11px] text-muted-foreground">{preset.url}</div>
          )}
        </div>
        {custom && (
          <Button variant="ghost" size="icon-xs" aria-label={`Remove ${name}`} disabled={locked} onClick={onRemove}>
            <X />
          </Button>
        )}
        <Switch
          aria-label={`Stream to ${name}`}
          disabled={locked}
          checked={dest.enabled}
          onCheckedChange={(enabled) => onChange({ enabled })}
        />
      </div>

      <div className="mt-2 ml-10 flex min-h-7 items-center gap-2 text-xs">
        {editing ? (
          <form
            className="flex flex-1 items-center gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              saveKey();
            }}
          >
            <input
              type="password"
              autoComplete="off"
              autoFocus
              aria-label={whip ? "Token" : "Stream key"}
              placeholder={whip ? "Token (if the server needs one)" : "Stream key"}
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") cancel();
              }}
              className="h-7 flex-1 rounded-[8px] border bg-card px-2 text-xs outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
            />
            <Button type="submit" size="xs" disabled={!draft.trim()}>
              Save to Keychain
            </Button>
            <Button type="button" variant="ghost" size="xs" onClick={cancel}>
              Cancel
            </Button>
          </form>
        ) : dest.keyStored ? (
          <>
            <span className="inline-flex items-center gap-1.5 text-ok">
              <KeyRound className="size-3.5" /> {whip ? "Token" : "Stream key"} in Keychain
            </span>
            <span className="flex-1" />
            <Button variant="ghost" size="xs" disabled={locked} onClick={() => setEditing(true)}>
              Replace
            </Button>
            <Button
              variant="ghost"
              size="xs"
              disabled={locked}
              className="text-muted-foreground"
              onClick={() => onChange({ keyStored: false })}
            >
              Remove
            </Button>
          </>
        ) : (
          <>
            <span className={cn("text-muted-foreground", dest.enabled && !whip && "text-warn")}>
              {whip ? "No token (optional)" : dest.enabled ? "Needs a stream key to go live" : "No stream key"}
            </span>
            <span className="flex-1" />
            <Button variant="soft" size="xs" disabled={locked} onClick={() => setEditing(true)}>
              {whip ? "Set token…" : "Set key…"}
            </Button>
          </>
        )}
      </div>
    </div>
  );
}
