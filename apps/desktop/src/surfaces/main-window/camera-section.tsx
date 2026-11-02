import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@starter/ui/components/select";
import { Slider } from "@starter/ui/components/slider";
import { cn } from "@starter/ui/lib/utils";

import { engine } from "../../lib/engine";
import type { EngineState } from "../../lib/types";
import { FieldRow, MicroLabel, SIDEBAR_SLIDER, SwitchRow, Tip } from "./parts";
import { PIP_SHAPES, ShapeGlyph } from "./shape-glyph";

// Border colours offered as swatches (RRGGBB, the stored format). User data, not theme colours.
const BORDER_COLORS: ReadonlyArray<{ hex: string; label: string }> = [
  { hex: "ffffff", label: "White" },
  { hex: "18191b", label: "Black" },
  { hex: "8a8f98", label: "Grey" },
  { hex: "1f7a8c", label: "Teal" },
  { hex: "e8b53a", label: "Yellow" },
  { hex: "6d5bd0", label: "Violet" },
];

const NO_CAMERA = "none";

/**
 * SOURCE OF TRUTH KEYWORDS: CameraSection, camera device, overlay style, pip shape, pip size, border, mirror, camera track
 * WHAT:  Sidebar camera controls: device picker, overlay shape/size/roundness/border/shadow/
 *        mirror, and "Save camera as separate track".
 * WHY:   Overlay changes apply live to preview and file (CAM-21), so they stay editable while
 *        recording. Overlay styling only shows in the Screen + Camera scene (CAM-25).
 * WHERE: main window sidebar, CAMERA section.
 */
export function CameraSection({ e, locked }: { e: EngineState; locked: boolean }) {
  const s = e.settings;
  const overlay = s.scene === "screen-cam";
  const selected = e.cameras.find((c) => c.id === s.cameraId);
  const usesRoundness = s.pipShape === "rounded" || s.pipShape === "square" || s.pipShape === "squircle";

  if (s.scene === "screen") {
    return (
      <p className="text-[12px] leading-4 text-muted-foreground">
        The camera is off in the Screen scene. Pick Screen + Camera or Camera above to use it.
      </p>
    );
  }

  return (
    <>
      <Select
        value={s.cameraId ?? NO_CAMERA}
        onValueChange={(v) => engine.update({ cameraId: v === NO_CAMERA ? null : v })}
        disabled={locked}
      >
        <SelectTrigger size="sm" className="w-full bg-card" aria-label="Camera">
          <SelectValue placeholder="Choose a camera" />
        </SelectTrigger>
        <SelectContent position="popper">
          {e.cameras.map((c) => (
            <SelectItem key={c.id} value={c.id} disabled={!c.connected}>
              {c.connected ? c.name : `${c.name} (not connected)`}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {selected && !selected.connected && (
        <p className="text-[11.5px] leading-4 text-warn">
          {selected.name} is not connected. Plug it in or pick another camera.
        </p>
      )}

      {overlay && (
        <>
          <MicroLabel>Overlay</MicroLabel>
          <div
            role="radiogroup"
            aria-label="Overlay shape"
            className="grid grid-cols-6 gap-1 rounded-[10px] bg-foreground/[0.06] p-0.5"
          >
            {PIP_SHAPES.map((sh) => {
              const on = s.pipShape === sh.value;
              return (
                <Tip key={sh.value} label={sh.label}>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={on}
                    aria-label={sh.label}
                    onClick={() => engine.update({ pipShape: sh.value })}
                    className={cn(
                      "grid h-8 place-items-center rounded-[8px] transition-[background,color,box-shadow] duration-150 outline-none focus-visible:ring-3 focus-visible:ring-ring/50",
                      on ? "bg-card text-foreground shadow-ledge" : "text-muted-foreground hover:text-foreground",
                    )}
                  >
                    <ShapeGlyph shape={sh.value} />
                  </button>
                </Tip>
              );
            })}
          </div>

          <FieldRow label="Size" value={`${Math.round(s.pipSize * 100)}%`}>
            <Slider
              className={SIDEBAR_SLIDER}
              aria-label="Overlay size, percent of frame width"
              min={10}
              max={50}
              step={1}
              value={[Math.round(s.pipSize * 100)]}
              onValueChange={([v]) => v !== undefined && engine.update({ pipSize: v / 100 })}
            />
          </FieldRow>

          {usesRoundness && (
            <FieldRow label="Roundness" value={`${Math.round(s.pipRoundness * 100)}%`}>
              <Slider
                className={SIDEBAR_SLIDER}
                aria-label="Corner roundness"
                min={0}
                max={100}
                step={1}
                value={[Math.round(s.pipRoundness * 100)]}
                onValueChange={([v]) => v !== undefined && engine.update({ pipRoundness: v / 100 })}
              />
            </FieldRow>
          )}

          <FieldRow label="Border" value={s.pipBorder === 0 ? "None" : `${s.pipBorder} px`}>
            <Slider
              className={SIDEBAR_SLIDER}
              aria-label="Border width in pixels at 1080p"
              min={0}
              max={20}
              step={1}
              value={[s.pipBorder]}
              onValueChange={([v]) => v !== undefined && engine.update({ pipBorder: v })}
            />
          </FieldRow>
          {s.pipBorder > 0 && (
            <div role="radiogroup" aria-label="Border colour" className="flex gap-1.5">
              {BORDER_COLORS.map((c) => {
                const on = s.pipBorderColor.toLowerCase() === c.hex;
                return (
                  <button
                    key={c.hex}
                    type="button"
                    role="radio"
                    aria-checked={on}
                    aria-label={c.label}
                    title={c.label}
                    onClick={() => engine.update({ pipBorderColor: c.hex })}
                    className={cn(
                      "size-6 rounded-full ring-1 ring-foreground/15 ring-offset-sidebar transition-shadow outline-none focus-visible:ring-3 focus-visible:ring-ring/60",
                      on && "ring-2 ring-ring ring-offset-2",
                    )}
                    style={{ backgroundColor: `#${c.hex}` }}
                  />
                );
              })}
            </div>
          )}

          <SwitchRow label="Shadow" checked={s.pipShadow} onCheckedChange={(v) => engine.update({ pipShadow: v })} />
          <SwitchRow
            label="Mirror"
            hint="Flips the camera like a mirror, in the preview and the file."
            checked={s.mirror}
            onCheckedChange={(v) => engine.update({ mirror: v })}
          />
          <p className="text-[11.5px] leading-4 text-muted-foreground">
            Drag the camera in the preview to move it. It snaps to corners.
          </p>
        </>
      )}

      {!overlay && (
        <SwitchRow
          label="Mirror"
          hint="Flips the camera like a mirror, in the preview and the file."
          checked={s.mirror}
          onCheckedChange={(v) => engine.update({ mirror: v })}
        />
      )}

      <MicroLabel>Recording</MicroLabel>
      <SwitchRow
        label="Save camera as separate track"
        hint="Also writes the raw camera as its own video track, so you can reframe it later."
        checked={s.cameraTrack}
        onCheckedChange={(v) => engine.update({ cameraTrack: v })}
        disabled={locked}
      />
    </>
  );
}
