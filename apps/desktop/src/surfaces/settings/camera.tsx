import type * as React from "react";

import { Segmented } from "@starter/ui/components/segmented";
import { Slider } from "@starter/ui/components/slider";
import { Switch } from "@starter/ui/components/switch";
import { cn } from "@starter/ui/lib/utils";

import { CameraBubble } from "../../components/camera-bubble";
import { ALLOW_ACCESS, allowAccessItem, askPermission } from "../../components/permission-ask";
import { engine, useEngine } from "../../lib/engine";
import type { PipShape, RecorderSettings } from "../../lib/types";
import { deviceOptions, fromDeviceValue, toDeviceValue } from "./audio";
import { ChoiceSelect, Readout, SettingsGroup, SettingsPage, SettingsRow } from "./parts";

const SHAPES: ReadonlyArray<{ value: PipShape; label: string }> = [
  { value: "squircle", label: "Squircle" },
  { value: "rounded", label: "Rounded" },
  { value: "circle", label: "Circle" },
  { value: "square", label: "Square" },
  { value: "pill", label: "Pill" },
  { value: "rect", label: "16:9" },
];
/** Border colour presets, stored as RRGGBB like Rec's pip_border_color. */
const BORDER_COLORS = [
  { hex: "ffffff", label: "White" },
  { hex: "16181b", label: "Graphite" },
] as const;

const FRAME_W = 300;
const FRAME_H = Math.round((FRAME_W * 9) / 16);

/** Camera: device, mirror, resolution, separate track, overlay defaults with a live preview. Matrix SET-K09/19..26, FLG-15/17..25, NEW camera track + 1080p. */
export function CameraSection({ cameraStream }: { cameraStream: MediaStream | null }) {
  const e = useEngine();
  const s = e.settings;
  const roundable = s.pipShape === "rounded" || s.pipShape === "square" || s.pipShape === "squircle";

  return (
    <SettingsPage title="Camera" subtitle="Your webcam overlay: which camera, and how it looks by default.">
      <SettingsGroup title="Device">
        <SettingsRow label="Camera" hint="Used when the scene includes the camera.">
          <ChoiceSelect<string>
            aria-label="Camera"
            value={toDeviceValue(s.cameraId)}
            onChange={(v) =>
              v === ALLOW_ACCESS
                ? void askPermission("camera", e.permissions.camera)
                : engine.update({ cameraId: fromDeviceValue(v) })
            }
            options={deviceOptions(
              e.cameras,
              "Default camera",
              allowAccessItem("camera", e.cameras, e.permissions.camera),
            )}
            className="w-52"
          />
        </SettingsRow>
        <SettingsRow label="Camera resolution" badge="new" hint="1080p looks sharper when the camera fills the frame.">
          <Segmented<"720p" | "1080p">
            aria-label="Camera resolution"
            value={s.cameraResolution}
            onValueChange={(cameraResolution) => engine.update({ cameraResolution })}
            options={[
              { value: "720p", label: "720p" },
              { value: "1080p", label: "1080p" },
            ]}
          />
        </SettingsRow>
        <SettingsRow
          label="Mirror"
          hint="Flip horizontally so it looks like a mirror. Text reads backwards."
          htmlFor="set-mirror"
        >
          <Switch id="set-mirror" checked={s.mirror} onCheckedChange={(mirror) => engine.update({ mirror })} />
        </SettingsRow>
        <SettingsRow
          label="Save camera as separate track"
          badge="new"
          htmlFor="set-camtrack"
          hint={
            s.container === "mov"
              ? "Adds a clean second video track inside the .mov, so you can re-frame it later."
              : "MP4 can't hold it: Cortex saves a side file next to the recording."
          }
        >
          <Switch
            id="set-camtrack"
            checked={s.cameraTrack}
            onCheckedChange={(cameraTrack) => engine.update({ cameraTrack })}
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup
        title="Overlay defaults"
        footer="New recordings start with these. Drag the overlay in the main window to place it."
      >
        <OverlayPreview s={s} stream={cameraStream} />
        <SettingsRow label="Shape" stacked>
          <Segmented<PipShape>
            aria-label="Overlay shape"
            className="w-full"
            value={s.pipShape}
            onValueChange={(pipShape) => engine.update({ pipShape })}
            options={SHAPES}
          />
        </SettingsRow>
        <SliderRow
          label="Size"
          value={s.pipSize}
          min={0.1}
          max={0.5}
          step={0.01}
          readout={`${Math.round(s.pipSize * 100)}%`}
          onChange={(pipSize) => engine.update({ pipSize })}
        />
        <SliderRow
          label="Roundness"
          disabled={!roundable}
          value={s.pipRoundness}
          min={0}
          max={1}
          step={0.01}
          readout={roundable ? `${Math.round(s.pipRoundness * 100)}%` : "—"}
          onChange={(pipRoundness) => engine.update({ pipRoundness })}
        />
        <SliderRow
          label="Border"
          value={s.pipBorder}
          min={0}
          max={24}
          step={1}
          readout={s.pipBorder === 0 ? "None" : `${s.pipBorder} px`}
          onChange={(pipBorder) => engine.update({ pipBorder })}
        >
          <div role="radiogroup" aria-label="Border colour" className="flex items-center gap-1.5">
            {BORDER_COLORS.map((c) => (
              <button
                key={c.hex}
                type="button"
                role="radio"
                aria-checked={s.pipBorderColor === c.hex}
                aria-label={c.label}
                title={c.label}
                disabled={s.pipBorder === 0}
                onClick={() => engine.update({ pipBorderColor: c.hex })}
                className={cn(
                  "size-5 rounded-full border border-foreground/25 ring-offset-2 ring-offset-card outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-40",
                  s.pipBorderColor === c.hex && "ring-2 ring-ring",
                )}
                // The user's chosen border colour is data, not a theme colour.
                style={{ background: `#${c.hex}` }}
              />
            ))}
            <label
              title="Custom colour"
              className={cn(
                "relative size-5 cursor-pointer overflow-hidden rounded-full border bg-[conic-gradient(var(--color-meter-high),var(--color-meter-mid),var(--color-meter-low),var(--color-accent-solid),var(--color-meter-high))] ring-offset-2 ring-offset-card focus-within:ring-2 focus-within:ring-ring",
                !BORDER_COLORS.some((c) => c.hex === s.pipBorderColor) && "ring-2 ring-ring",
                s.pipBorder === 0 && "pointer-events-none opacity-40",
              )}
            >
              <input
                type="color"
                aria-label="Custom border colour"
                disabled={s.pipBorder === 0}
                value={`#${s.pipBorderColor}`}
                onChange={(ev) => engine.update({ pipBorderColor: ev.target.value.slice(1) })}
                className="absolute inset-0 cursor-pointer opacity-0"
              />
            </label>
          </div>
        </SliderRow>
        <SettingsRow label="Shadow" hint="A soft drop shadow lifts the overlay off busy screens." htmlFor="set-shadow">
          <Switch id="set-shadow" checked={s.pipShadow} onCheckedChange={(pipShadow) => engine.update({ pipShadow })} />
        </SettingsRow>
      </SettingsGroup>
    </SettingsPage>
  );
}

function SliderRow({
  label,
  value,
  min,
  max,
  step,
  readout,
  disabled,
  onChange,
  children,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  readout: string;
  disabled?: boolean;
  onChange: (v: number) => void;
  children?: React.ReactNode;
}) {
  return (
    <div className={cn("flex min-h-[48px] items-center gap-4 px-4 py-2.5", disabled && "opacity-45")}>
      <span className="w-24 shrink-0 text-[13px] font-medium">{label}</span>
      <Slider
        aria-label={label}
        disabled={disabled}
        min={min}
        max={max}
        step={step}
        value={[value]}
        onValueChange={([v]) => {
          if (v !== undefined) onChange(v);
        }}
        className="flex-1"
      />
      <Readout>{readout}</Readout>
      {children}
    </div>
  );
}

/** A miniature of the recording frame with the overlay where it will land. */
function OverlayPreview({ s, stream }: { s: RecorderSettings; stream: MediaStream | null }) {
  const scale = FRAME_W / 1920;
  const width = Math.round(s.pipSize * FRAME_W);
  const corner = s.pipPlace.corner ?? "bottom-right";
  const inset = Math.round(FRAME_W * 0.03);
  return (
    <div className="grid place-items-center bg-stage py-5">
      <div
        aria-label="Overlay preview"
        role="img"
        className="relative overflow-hidden rounded-[6px] bg-gradient-to-br from-muted-foreground/30 via-muted-foreground/15 to-muted-foreground/5 ring-1 ring-background/10"
        style={{ width: FRAME_W, height: FRAME_H }}
      >
        {/* Stand-in screen content: a window with a few lines of text. */}
        <div className="absolute top-4 left-4 h-[62%] w-[58%] rounded-[4px] bg-background/80 p-2.5">
          <div className="mb-2 h-1.5 w-1/3 rounded-full bg-muted-foreground/40" />
          <div className="mb-1.5 h-1 w-5/6 rounded-full bg-muted-foreground/25" />
          <div className="mb-1.5 h-1 w-3/4 rounded-full bg-muted-foreground/25" />
          <div className="h-1 w-2/3 rounded-full bg-muted-foreground/25" />
        </div>
        <div
          className="absolute transition-[top,left,right,bottom] duration-200 ease-out-quick"
          style={{
            [corner.startsWith("top") ? "top" : "bottom"]: inset,
            [corner.endsWith("left") ? "left" : "right"]: inset,
          }}
        >
          <CameraBubble
            shape={s.pipShape}
            roundness={s.pipRoundness}
            width={width}
            border={Math.max(s.pipBorder > 0 ? 1 : 0, Math.round(s.pipBorder * scale * 2))}
            borderColor={s.pipBorderColor}
            shadow={s.pipShadow}
            mirror={s.mirror}
            stream={stream}
          />
        </div>
      </div>
    </div>
  );
}
